package objectstore

import (
	"runtime"
	"sync"
)

const maxObjectWriteWorkers = 16

func (s Store) writeFileObjects(
	entry LanguageEntry,
	paths []string,
	files map[string][]byte,
	groups map[string]typedRecords,
) ([]FileEntry, error) {
	result := make([]FileEntry, len(paths))
	if len(paths) == 0 {
		return result, nil
	}

	workers := runtime.GOMAXPROCS(0)
	if workers > maxObjectWriteWorkers {
		workers = maxObjectWriteWorkers
	}
	if workers > len(paths) {
		workers = len(paths)
	}
	if workers < 2 {
		for index, path := range paths {
			file, err := s.writeFileObject(entry, path, files[path], groups[path])
			if err != nil {
				return nil, err
			}
			result[index] = file
		}
		return result, nil
	}

	jobs := make(chan int)
	errorsByPath := make([]error, len(paths))
	var group sync.WaitGroup
	group.Add(workers)
	for range workers {
		go func() {
			defer group.Done()
			for index := range jobs {
				path := paths[index]
				result[index], errorsByPath[index] = s.writeFileObject(entry, path, files[path], groups[path])
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
			return nil, err
		}
	}
	return result, nil
}
