package state

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

type Repository struct {
	Root string
}

type Change struct {
	Status string
	Old    string
	New    string
}

func Ensure(root string) (*Repository, error) {
	if err := os.MkdirAll(root, 0o755); err != nil {
		return nil, fmt.Errorf("create Lexicon state repository: %w", err)
	}
	repository := &Repository{Root: root}
	if err := repository.run("init", "--quiet", "--initial-branch=state"); err != nil {
		return nil, err
	}
	for key, value := range map[string]string{
		"user.name": "Lexicon", "user.email": "lexicon@local", "core.autocrlf": "false", "core.filemode": "false",
	} {
		if err := repository.run("config", key, value); err != nil {
			return nil, err
		}
	}
	return repository, nil
}

func Open(root string) (*Repository, error) {
	repository := &Repository{Root: root}
	if _, err := repository.output("rev-parse", "--git-dir"); err != nil {
		return nil, fmt.Errorf("open Lexicon state repository: %w", err)
	}
	return repository, nil
}

func (r *Repository) ResetIndex() error {
	if !r.HasHead() {
		return nil
	}
	return r.run("reset", "--quiet", "--mixed", "HEAD")
}

func (r *Repository) StageSource() error {
	return r.run("add", "-A", "--", "source")
}

func (r *Repository) StageAll() error {
	return r.run("add", "-A")
}

func (r *Repository) HasHead() bool {
	_, err := r.output("rev-parse", "--verify", "HEAD")
	return err == nil
}

func (r *Repository) Head() (string, error) {
	if !r.HasHead() {
		return "", fmt.Errorf("Lexicon state repository has no commit")
	}
	return r.output("rev-parse", "HEAD")
}

func (r *Repository) HeadSource(path string) ([]byte, error) {
	if filepath.IsAbs(path) {
		return nil, fmt.Errorf("source path must be relative: %s", path)
	}
	normalized := filepath.ToSlash(filepath.Clean(filepath.FromSlash(path)))
	if normalized == "." || normalized == "" || normalized == ".." || strings.HasPrefix(normalized, "../") {
		return nil, fmt.Errorf("invalid source path: %s", path)
	}
	return r.outputBytes("show", "HEAD:source/"+normalized)
}

func (r *Repository) HasStagedChanges() bool {
	command := exec.Command("git", "diff", "--cached", "--quiet")
	command.Dir = r.Root
	err := command.Run()
	return err != nil
}

func (r *Repository) CommitState() error {
	if !r.HasHead() {
		return r.run("commit", "--quiet", "--allow-empty", "-m", "Lexicon state")
	}
	if !r.HasStagedChanges() {
		return nil
	}
	if err := r.run("commit", "--quiet", "--amend", "--no-edit"); err != nil {
		return err
	}
	_ = r.run("reflog", "expire", "--expire=now", "--all")
	return nil
}

func (r *Repository) SourceChanges() ([]Change, error) {
	if !r.HasHead() {
		return nil, nil
	}
	data, err := r.outputBytes("diff", "--cached", "--name-status", "-z", "-M", "HEAD", "--", "source")
	if err != nil {
		return nil, err
	}
	return parseChanges(data), nil
}

func (r *Repository) run(arguments ...string) error {
	_, err := r.output(arguments...)
	return err
}

func (r *Repository) output(arguments ...string) (string, error) {
	data, err := r.outputBytes(arguments...)
	return strings.TrimSpace(string(data)), err
}

func (r *Repository) outputBytes(arguments ...string) ([]byte, error) {
	command := exec.Command("git", arguments...)
	command.Dir = r.Root
	var stderr bytes.Buffer
	command.Stderr = &stderr
	data, err := command.Output()
	if err != nil {
		detail := strings.TrimSpace(stderr.String())
		if detail == "" {
			detail = err.Error()
		}
		return nil, fmt.Errorf("git %s: %s", strings.Join(arguments, " "), detail)
	}
	return data, nil
}

func parseChanges(data []byte) []Change {
	parts := bytes.Split(data, []byte{0})
	changes := make([]Change, 0)
	for index := 0; index < len(parts)-1; {
		status := string(parts[index])
		index++
		if status == "" || index >= len(parts)-1 {
			break
		}
		first := filepath.ToSlash(strings.TrimPrefix(string(parts[index]), "source/"))
		index++
		change := Change{Status: status, New: first}
		if strings.HasPrefix(status, "R") || strings.HasPrefix(status, "C") {
			change.Old = first
			if index < len(parts)-1 {
				change.New = filepath.ToSlash(strings.TrimPrefix(string(parts[index]), "source/"))
				index++
			}
		}
		changes = append(changes, change)
	}
	return changes
}
