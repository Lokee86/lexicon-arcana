package scan

import (
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
)

const (
	standardShardUnits       = 64
	enterpriseShardUnits     = 32
	enterpriseFileThreshold  = 2_000
	enterpriseBytesThreshold = 128 * 1024 * 1024
	maxLogicalShards         = 4096
)

// ExecutionPlan describes logical partitioning separately from physical
// concurrency. Logical shards may greatly exceed the number of active workers.
type ExecutionPlan struct {
	Language       string
	SourceFiles    int
	SourceBytes    int64
	LogicalShards  int
	ActiveWorkers  int
	MergeFanIn     int
	ReservedWeight int
}

func (s *Scanner) executionPlan(plan analysisPlan) (ExecutionPlan, error) {
	result := ExecutionPlan{
		Language:      plan.Language,
		LogicalShards: 1, ActiveWorkers: 1, MergeFanIn: 2, ReservedWeight: 1,
	}
	if plan.Language != "go" {
		return result, nil
	}
	paths, bytes, err := s.analysisInventory(plan)
	if err != nil {
		return ExecutionPlan{}, err
	}
	result.SourceFiles = len(paths)
	result.SourceBytes = bytes
	if len(paths) < 2 {
		return result, nil
	}

	result.LogicalShards = logicalShardCount(len(paths), bytes)

	workers := runtime.GOMAXPROCS(0)
	if configured := configuredWorkerLimit(); configured > 0 && configured < workers {
		workers = configured
	}
	if workers < 1 {
		workers = 1
	}
	workerCeiling := result.LogicalShards
	if len(paths) < enterpriseFileThreshold && bytes < enterpriseBytesThreshold {
		workerCeiling = (result.LogicalShards + 1) / 2
	}
	if workers > workerCeiling {
		workers = workerCeiling
	}
	result.ActiveWorkers = workers
	result.ReservedWeight = workers
	switch {
	case result.LogicalShards > 64:
		result.MergeFanIn = 8
	case result.LogicalShards > 8:
		result.MergeFanIn = 4
	default:
		result.MergeFanIn = 2
	}
	return result, nil
}

func (s *Scanner) analysisInventory(plan analysisPlan) ([]string, int64, error) {
	root := filepath.Join(s.StateRoot, "source")
	if !plan.Full {
		paths := uniqueSorted(append(append([]string(nil), plan.ChangedFiles...), plan.ContextFiles...))
		var bytes int64
		for _, path := range paths {
			info, err := os.Stat(filepath.Join(root, filepath.FromSlash(path)))
			if err != nil {
				if os.IsNotExist(err) {
					continue
				}
				return nil, 0, err
			}
			bytes += info.Size()
		}
		return paths, bytes, nil
	}

	var paths []string
	var bytes int64
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		if !languageOwnsSource(plan.Language, relative) {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		paths = append(paths, relative)
		bytes += info.Size()
		return nil
	})
	sort.Strings(paths)
	return paths, bytes, err
}

func logicalShardCount(fileCount int, sourceBytes int64) int {
	if fileCount < 2 {
		return 1
	}
	units := fileCount
	if byteUnits := int((sourceBytes + 64*1024 - 1) / (64 * 1024)); byteUnits > units {
		units = byteUnits
	}
	shardUnits := standardShardUnits
	if fileCount >= enterpriseFileThreshold || sourceBytes >= enterpriseBytesThreshold {
		shardUnits = enterpriseShardUnits
	}
	target := (units + shardUnits - 1) / shardUnits
	shards := nextPowerOfTwo(target)
	if shards > fileCount {
		shards = fileCount
	}
	if shards > maxLogicalShards {
		shards = maxLogicalShards
	}
	return shards
}

func configuredWorkerLimit() int {
	value, err := strconv.Atoi(os.Getenv("LEXICON_MAX_WORKERS"))
	if err != nil || value < 1 {
		return 0
	}
	return value
}

func nextPowerOfTwo(value int) int {
	if value <= 1 {
		return 1
	}
	result := 1
	for result < value && result < maxLogicalShards {
		result <<= 1
	}
	return result
}
