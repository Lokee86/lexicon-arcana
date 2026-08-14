package repostate

import (
	"crypto/sha256"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	ignorepolicy "github.com/Lokee86/grimoire/internal/ignore"
)

// quickSourceFingerprint hashes source-file metadata rather than file contents.
// It is persisted beside the full content fingerprint and is used only to prove
// that a previously prepared fingerprint can be reused. Any metadata change
// falls back to the full content-aware repository fingerprint.
func quickSourceFingerprint(root string) (string, error) {
	policy, err := ignorepolicy.Load(root, "")
	if err != nil {
		return "", err
	}
	hash := sha256.New()
	if err := hashGitMetadata(hash, root); err != nil {
		return "", err
	}
	err = filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == root {
			return nil
		}
		if entry.IsDir() {
			if excludedDirectory(entry.Name()) {
				return filepath.SkipDir
			}
			ignored, err := policy.Ignored(path, true)
			if err != nil {
				return err
			}
			if ignored {
				return filepath.SkipDir
			}
			if err := policy.LoadDirectory(path); err != nil {
				return err
			}
			return nil
		}
		if entry.Type()&os.ModeSymlink != 0 || !entry.Type().IsRegular() {
			return nil
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		controlFile := strings.EqualFold(entry.Name(), ".gitignore")
		if !controlFile && !fingerprintPath(relative) {
			return nil
		}
		ignored, err := policy.Ignored(path, false)
		if err != nil {
			return err
		}
		if ignored && !controlFile {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		_, _ = fmt.Fprintf(hash, "file\x00%s\x00%d\x00%d\x00%d\x00", relative, info.Size(), info.ModTime().UnixNano(), info.Mode())
		return nil
	})
	if err != nil {
		return "", err
	}
	return fmt.Sprintf("sha256:%x", hash.Sum(nil)), nil
}

func hashGitMetadata(hash io.Writer, root string) error {
	gitPath := filepath.Join(root, ".git")
	gitDir := gitPath
	if info, err := os.Stat(gitPath); err == nil && !info.IsDir() {
		data, readErr := os.ReadFile(gitPath)
		if readErr != nil {
			return readErr
		}
		_, _ = fmt.Fprintf(hash, "gitfile\x00%s\x00", strings.TrimSpace(string(data)))
		value := strings.TrimSpace(string(data))
		if path, ok := strings.CutPrefix(value, "gitdir:"); ok {
			gitDir = strings.TrimSpace(path)
			if !filepath.IsAbs(gitDir) {
				gitDir = filepath.Join(root, gitDir)
			}
			gitDir = filepath.Clean(gitDir)
		}
	} else if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	commonDir := gitDir
	if data, err := os.ReadFile(filepath.Join(gitDir, "commondir")); err == nil {
		commonDir = strings.TrimSpace(string(data))
		if !filepath.IsAbs(commonDir) {
			commonDir = filepath.Join(gitDir, commonDir)
		}
		commonDir = filepath.Clean(commonDir)
	}
	hashMetadataFile(hash, filepath.Join(gitDir, "HEAD"))
	hashMetadataStat(hash, filepath.Join(gitDir, "index"))
	hashMetadataFile(hash, filepath.Join(commonDir, "packed-refs"))
	head, err := os.ReadFile(filepath.Join(gitDir, "HEAD"))
	if err == nil {
		if ref, ok := strings.CutPrefix(strings.TrimSpace(string(head)), "ref:"); ok {
			ref = strings.TrimSpace(ref)
			hashMetadataFile(hash, filepath.Join(gitDir, filepath.FromSlash(ref)))
			if commonDir != gitDir {
				hashMetadataFile(hash, filepath.Join(commonDir, filepath.FromSlash(ref)))
			}
		}
	}
	return nil
}

func hashMetadataFile(hash io.Writer, path string) {
	info, err := os.Stat(path)
	if err != nil || info.IsDir() {
		return
	}
	data, _ := os.ReadFile(path)
	_, _ = fmt.Fprintf(hash, "gitmeta\x00%s\x00%d\x00%d\x00", filepath.Base(path), info.Size(), info.ModTime().UnixNano())
	_, _ = hash.Write(data)
	_, _ = io.WriteString(hash, "\x00")
}

func hashMetadataStat(hash io.Writer, path string) {
	info, err := os.Stat(path)
	if err != nil || info.IsDir() {
		return
	}
	_, _ = fmt.Fprintf(hash, "gitstat\x00%s\x00%d\x00%d\x00", filepath.Base(path), info.Size(), info.ModTime().UnixNano())
}
