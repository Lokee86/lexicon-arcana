package state

import (
	"bytes"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	lexfiles "github.com/Lokee86/lexicon/internal/files"
)

type Mirror struct {
	Root string
}

func (m Mirror) SyncAll(source string) error {
	policy, err := lexfiles.LoadIgnorePolicy(source)
	if err != nil {
		return err
	}
	desired := make(map[string]string)
	err = filepath.WalkDir(source, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == source {
			return nil
		}
		if policy.Ignored(path, entry.IsDir()) {
			if entry.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.IsDir() || !lexfiles.Relevant(path) || entry.Type()&os.ModeSymlink != 0 {
			return nil
		}
		relative, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		desired[filepath.Clean(relative)] = path
		return nil
	})
	if err != nil {
		return err
	}
	if err := m.copyAll(desired); err != nil {
		return err
	}
	return m.removeMissing(desired)
}

func (m Mirror) SyncPaths(source string, paths []string) error {
	policy, err := lexfiles.LoadIgnorePolicy(source)
	if err != nil {
		return err
	}
	sort.Strings(paths)
	for _, path := range paths {
		absolute := path
		if !filepath.IsAbs(absolute) {
			absolute = filepath.Join(source, path)
		}
		relative, err := filepath.Rel(source, absolute)
		if err != nil || relative == "." || strings.HasPrefix(relative, "..") {
			continue
		}
		if policy.Ignored(absolute, false) {
			continue
		}
		info, statErr := os.Lstat(absolute)
		if statErr != nil {
			if os.IsNotExist(statErr) {
				_ = os.RemoveAll(filepath.Join(m.Root, relative))
				continue
			}
			return statErr
		}
		if policy.Ignored(absolute, info.IsDir()) {
			continue
		}
		if info.IsDir() {
			if err := m.syncDirectory(source, relative, policy); err != nil {
				return err
			}
			continue
		}
		if info.Mode()&os.ModeSymlink != 0 || !lexfiles.Relevant(relative) {
			_ = os.Remove(filepath.Join(m.Root, relative))
			continue
		}
		if err := m.copy(relative, absolute); err != nil {
			return err
		}
	}
	return nil
}

func (m Mirror) syncDirectory(source, relative string, policy lexfiles.IgnorePolicy) error {
	directory := filepath.Join(source, relative)
	desired := make(map[string]string)
	err := filepath.WalkDir(directory, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if policy.Ignored(path, entry.IsDir()) {
			if entry.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.IsDir() || entry.Type()&os.ModeSymlink != 0 || !lexfiles.Relevant(path) {
			return nil
		}
		rel, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		desired[filepath.Clean(rel)] = path
		return nil
	})
	if err != nil {
		return err
	}
	for rel, path := range desired {
		if err := m.copy(rel, path); err != nil {
			return err
		}
	}
	mirrorDirectory := filepath.Join(m.Root, relative)
	return filepath.WalkDir(mirrorDirectory, func(path string, entry fs.DirEntry, walkErr error) error {
		if os.IsNotExist(walkErr) {
			return nil
		}
		if walkErr != nil || entry.IsDir() {
			return walkErr
		}
		rel, _ := filepath.Rel(m.Root, path)
		if _, ok := desired[filepath.Clean(rel)]; !ok {
			return os.Remove(path)
		}
		return nil
	})
}

func (m Mirror) copy(relative, source string) error {
	data, err := os.ReadFile(source)
	if err != nil {
		return err
	}
	destination := filepath.Join(m.Root, relative)
	if existing, err := os.ReadFile(destination); err == nil && bytes.Equal(existing, data) {
		return nil
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		return err
	}
	temporary := destination + ".lexicon-tmp"
	if err := os.WriteFile(temporary, data, 0o644); err != nil {
		return err
	}
	if err := os.Rename(temporary, destination); err != nil {
		_ = os.Remove(destination)
		if retry := os.Rename(temporary, destination); retry != nil {
			return fmt.Errorf("replace mirror file %s: %w", destination, retry)
		}
	}
	return nil
}

func (m Mirror) removeMissing(desired map[string]string) error {
	directories := make([]string, 0)
	if err := filepath.WalkDir(m.Root, func(path string, entry fs.DirEntry, walkErr error) error {
		if os.IsNotExist(walkErr) {
			return nil
		}
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			if path != m.Root {
				directories = append(directories, path)
			}
			return nil
		}
		relative, _ := filepath.Rel(m.Root, path)
		if _, ok := desired[filepath.Clean(relative)]; !ok {
			return os.Remove(path)
		}
		return nil
	}); err != nil {
		return err
	}

	sort.Slice(directories, func(left, right int) bool {
		return len(directories[left]) > len(directories[right])
	})
	for _, directory := range directories {
		entries, err := os.ReadDir(directory)
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			return err
		}
		if len(entries) == 0 {
			if err := os.Remove(directory); err != nil && !os.IsNotExist(err) {
				return err
			}
		}
	}
	return nil
}
