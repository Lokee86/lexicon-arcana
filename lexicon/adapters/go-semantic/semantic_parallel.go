package main

import (
	"fmt"
	"go/ast"
	"runtime"
	"sort"
	"sync"

	"golang.org/x/tools/go/packages"
)

type semanticFileJob struct {
	pkg    *packages.Package
	file   *ast.File
	owner  string
	weight int
}

type semanticShardResult struct {
	calls        directCallAccumulator
	dataflow     dataflowAccumulator
	callsiteKeys map[string]string
}

type semanticCollection struct {
	calls    directCallAccumulator
	dataflow dataflowAccumulator
}

func (index *semanticIndex) collectParallelSemantics(
	settings execution,
) (semanticCollection, error) {
	jobs := index.semanticFileJobs()
	if len(jobs) == 0 {
		return semanticCollection{}, nil
	}
	settings = normalizedExecution(settings, len(jobs))
	shards := partitionSemanticJobs(jobs, settings.Shards)
	results := make([]*semanticShardResult, len(shards))

	tasks := make(chan int)
	var group sync.WaitGroup
	for worker := 0; worker < settings.Workers; worker++ {
		group.Add(1)
		go func() {
			defer group.Done()
			for shardIndex := range tasks {
				local := *index
				local.callsiteKeys = make(map[string]string)
				result := &semanticShardResult{callsiteKeys: local.callsiteKeys}
				for _, job := range shards[shardIndex] {
					result.calls.addRecords(
						local.collectDirectCallsForFile(job.pkg, job.file, job.owner),
					)
					result.dataflow.addRecords(
						local.collectDataflowForFile(job.pkg, job.file, job.owner),
					)
				}
				results[shardIndex] = result
			}
		}()
	}
	for shardIndex := range shards {
		tasks <- shardIndex
	}
	close(tasks)
	group.Wait()

	root, err := reduceSemanticShards(results, settings.MergeFanIn)
	if err != nil {
		return semanticCollection{}, err
	}
	index.callsiteKeys = root.callsiteKeys
	return semanticCollection{
		calls:    root.calls,
		dataflow: root.dataflow,
	}, nil
}

func (index *semanticIndex) semanticFileJobs() []semanticFileJob {
	var jobs []semanticFileJob
	for _, pkg := range index.packages {
		if pkg.TypesInfo == nil || pkg.Fset == nil {
			continue
		}
		for _, file := range pkg.Syntax {
			owner, ok := index.ownerForPosition(pkg.Fset.PositionFor(file.Pos(), false).Filename)
			if !ok {
				continue
			}
			jobs = append(jobs, semanticFileJob{
				pkg: pkg, file: file, owner: owner, weight: len(file.Decls) + 1,
			})
		}
	}
	sort.SliceStable(jobs, func(left, right int) bool {
		if jobs[left].pkg.ID != jobs[right].pkg.ID {
			return jobs[left].pkg.ID < jobs[right].pkg.ID
		}
		return jobs[left].owner < jobs[right].owner
	})
	return jobs
}

func normalizedExecution(settings execution, jobCount int) execution {
	if settings.Workers < 1 {
		settings.Workers = 1
	}
	if settings.Shards < 1 {
		settings.Shards = settings.Workers
	}
	if jobCount > 0 && settings.Shards > jobCount {
		settings.Shards = jobCount
	}
	if settings.Shards < 1 {
		settings.Shards = 1
	}
	if settings.Workers > settings.Shards {
		settings.Workers = settings.Shards
	}
	if maximum := runtime.GOMAXPROCS(0); settings.Workers > maximum {
		settings.Workers = maximum
	}
	if settings.MergeFanIn < 2 {
		settings.MergeFanIn = 2
	}
	return settings
}

func partitionSemanticJobs(jobs []semanticFileJob, count int) [][]semanticFileJob {
	if count < 1 {
		count = 1
	}
	if count > len(jobs) {
		count = len(jobs)
	}
	ordered := append([]semanticFileJob(nil), jobs...)
	sort.SliceStable(ordered, func(left, right int) bool {
		if ordered[left].weight != ordered[right].weight {
			return ordered[left].weight > ordered[right].weight
		}
		if ordered[left].pkg.ID != ordered[right].pkg.ID {
			return ordered[left].pkg.ID < ordered[right].pkg.ID
		}
		return ordered[left].owner < ordered[right].owner
	})
	shards := make([][]semanticFileJob, count)
	weights := make([]int, count)
	for _, job := range ordered {
		selected := 0
		for index := 1; index < count; index++ {
			if weights[index] < weights[selected] {
				selected = index
			}
		}
		shards[selected] = append(shards[selected], job)
		weights[selected] += job.weight
	}
	return shards
}

func reduceSemanticShards(
	shards []*semanticShardResult,
	fanIn int,
) (*semanticShardResult, error) {
	if fanIn < 2 {
		fanIn = 2
	}
	current := append([]*semanticShardResult(nil), shards...)
	for len(current) > 1 {
		next := make([]*semanticShardResult, 0, (len(current)+fanIn-1)/fanIn)
		for start := 0; start < len(current); start += fanIn {
			end := min(start+fanIn, len(current))
			merged := current[start]
			for index := start + 1; index < end; index++ {
				if err := mergeSemanticShard(merged, current[index]); err != nil {
					return nil, err
				}
			}
			next = append(next, merged)
		}
		current = next
	}
	return current[0], nil
}

func mergeSemanticShard(destination, source *semanticShardResult) error {
	if source == nil {
		return nil
	}
	destination.calls.merge(source.calls)
	destination.dataflow.merge(source.dataflow)
	for key, incoming := range source.callsiteKeys {
		if existing, exists := destination.callsiteKeys[key]; exists && existing != incoming {
			return fmt.Errorf("semantic shard callsite conflict for %s", key)
		}
		destination.callsiteKeys[key] = incoming
	}
	return nil
}
