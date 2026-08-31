package state

import (
	"runtime"
	"sort"
	"sync"
)

const maxMirrorCopyWorkers = 8

func (m Mirror) copyAll(desired map[string]string) error {
	paths := make([]string, 0, len(desired))
	for relative := range desired {
		paths = append(paths, relative)
	}
	sort.Strings(paths)
	if len(paths) == 0 {
		return nil
	}

	workers := runtime.GOMAXPROCS(0)
	if workers > maxMirrorCopyWorkers {
		workers = maxMirrorCopyWorkers
	}
	if workers > len(paths) {
		workers = len(paths)
	}
	if workers < 2 {
		for _, relative := range paths {
			if err := m.copy(relative, desired[relative]); err != nil {
				return err
			}
		}
		return nil
	}

	jobs := make(chan int)
	errorsByPath := make([]error, len(paths))
	var group sync.WaitGroup
	group.Add(workers)
	for range workers {
		go func() {
			defer group.Done()
			for index := range jobs {
				relative := paths[index]
				errorsByPath[index] = m.copy(relative, desired[relative])
			}
		}()
	}
	for index := range paths {
		jobs <- index
	}
	close(jobs)
	group.Wait()
	for _, err := range errorsByPath {
		if err != nil {
			return err
		}
	}
	return nil
}
