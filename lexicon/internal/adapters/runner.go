package adapters

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	languageRegistry "github.com/Lokee86/lexicon/internal/languages"
)

type Request struct {
	Language     string
	Repository   string
	Output       string
	ChangedFiles []string
	RemovedFiles []string
	Workers      int
	Shards       int
	MergeFanIn   int
}

type Analyzer interface {
	Run(context.Context, Request) error
}

type StreamingAnalyzer interface {
	RunStream(context.Context, Request, func(io.Reader) error) error
}

type Runner struct {
	Root string
}

func (r Runner) Run(ctx context.Context, request Request) error {
	if err := os.MkdirAll(filepath.Dir(request.Output), 0o755); err != nil {
		return err
	}
	command, err := r.command(ctx, request)
	if err != nil {
		return err
	}
	data, err := command.CombinedOutput()
	if err != nil {
		return fmt.Errorf("%s adapter failed: %w\n%s", request.Language, err, strings.TrimSpace(string(data)))
	}
	if info, err := os.Stat(request.Output); err != nil || info.Size() == 0 {
		return fmt.Errorf("%s adapter produced no output", request.Language)
	}
	return nil
}

func (r Runner) RunStream(ctx context.Context, request Request, consume func(io.Reader) error) error {
	if !languageRegistry.SupportsStreamingOutput(request.Language) {
		if err := r.Run(ctx, request); err != nil {
			return err
		}
		file, err := os.Open(request.Output)
		if err != nil {
			return fmt.Errorf("open %s adapter output: %w", request.Language, err)
		}
		defer file.Close()
		return consume(file)
	}
	request.Output = "-"
	command, err := r.command(ctx, request)
	if err != nil {
		return err
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		return fmt.Errorf("open %s adapter output: %w", request.Language, err)
	}
	var stderr bytes.Buffer
	command.Stderr = &stderr
	if err := command.Start(); err != nil {
		return fmt.Errorf("start %s adapter: %w", request.Language, err)
	}
	consumeErr := consume(stdout)
	if consumeErr != nil && command.Process != nil {
		_ = command.Process.Kill()
	}
	waitErr := command.Wait()
	if consumeErr != nil {
		return fmt.Errorf("read %s adapter output: %w", request.Language, consumeErr)
	}
	if waitErr != nil {
		return fmt.Errorf("%s adapter failed: %w\n%s", request.Language, waitErr, strings.TrimSpace(stderr.String()))
	}
	return nil
}

func (r Runner) command(ctx context.Context, request Request) (*exec.Cmd, error) {
	arguments := adapterArguments(request)
	switch request.Language {
	case "generic":
		return nil, fmt.Errorf("generic adapter requires an extension-qualified language")
	case "c-family", "go", "gdscript", "java", "kotlin", "lotusscript":
		if executable, ok := packagedExecutable(r.Root, request.Language); ok {
			return exec.CommandContext(ctx, executable, arguments...), nil
		}
		executable, err := findExecutable("go")
		if err != nil {
			return nil, err
		}
		directory := filepath.Join(r.Root, request.Language)
		command := exec.CommandContext(ctx, executable, append([]string{"run", "."}, arguments...)...)
		command.Dir = directory
		return command, nil
	case "csharp":
		if executable, ok := packagedExecutable(r.Root, request.Language); ok {
			return exec.CommandContext(ctx, executable, arguments...), nil
		}
		executable, err := findExecutable("dotnet")
		if err != nil {
			return nil, err
		}
		project := filepath.Join(r.Root, "csharp", "Lexicon.CSharp.csproj")
		prefix := []string{"run", "--project", project, "--"}
		return exec.CommandContext(ctx, executable, append(prefix, arguments...)...), nil
	case "python":
		executable, err := findExecutable("python", "python3")
		if err != nil {
			return nil, err
		}
		command := exec.CommandContext(ctx, executable, append([]string{"-m", "lexicon_python"}, arguments...)...)
		pythonRoot := filepath.Join(r.Root, "python")
		command.Env = append(os.Environ(), "PYTHONPATH="+joinPathList(pythonRoot, os.Getenv("PYTHONPATH")))
		return command, nil
	case "ruby":
		executable, err := findExecutable("ruby")
		if err != nil {
			return nil, err
		}
		return exec.CommandContext(ctx, executable, append([]string{filepath.Join(r.Root, "ruby", "lexicon_ruby.rb")}, arguments...)...), nil
	case "rust":
		if executable, ok := packagedExecutable(r.Root, request.Language); ok {
			return exec.CommandContext(ctx, executable, arguments...), nil
		}
		executable, err := findExecutable("cargo")
		if err != nil {
			return nil, err
		}
		manifest := filepath.Join(r.Root, "rust", "Cargo.toml")
		prefix := []string{"run", "--quiet", "--manifest-path", manifest, "--"}
		return exec.CommandContext(ctx, executable, append(prefix, arguments...)...), nil
	case "typescript":
		distribution := filepath.Join(r.Root, "typescript", "dist", "cli.js")
		if fileExists(distribution) {
			executable, err := findExecutable("node")
			if err != nil {
				return nil, err
			}
			return exec.CommandContext(ctx, executable, append([]string{distribution}, arguments...)...), nil
		}
		if err := r.prepareTypeScript(ctx); err != nil {
			return nil, err
		}
		executable, err := findExecutable("node")
		if err != nil {
			return nil, err
		}
		return exec.CommandContext(ctx, executable, append([]string{filepath.Join(r.Root, "typescript", "dist", "cli.js")}, arguments...)...), nil
	default:
		if languageRegistry.IsGeneric(request.Language) {
			if executable, ok := packagedExecutable(r.Root, "generic"); ok {
				return exec.CommandContext(ctx, executable, arguments...), nil
			}
			executable, err := findExecutable("go")
			if err != nil {
				return nil, err
			}
			directory := filepath.Join(r.Root, "generic")
			command := exec.CommandContext(ctx, executable, append([]string{"run", "."}, arguments...)...)
			command.Dir = directory
			return command, nil
		}
		return nil, fmt.Errorf("unsupported language %q", request.Language)
	}
}

func adapterArguments(request Request) []string {
	arguments := []string{"--repo", request.Repository, "--output", request.Output}
	if languageRegistry.IsGeneric(request.Language) {
		arguments = append(arguments, "--language", request.Language)
	}
	for _, path := range request.ChangedFiles {
		arguments = append(arguments, "--changed-file", filepath.ToSlash(path))
	}
	for _, path := range request.RemovedFiles {
		arguments = append(arguments, "--removed-file", filepath.ToSlash(path))
	}
	if languageRegistry.SupportsPartitionedExecution(request.Language) {
		if request.Workers > 0 {
			arguments = append(arguments, "--workers", fmt.Sprint(request.Workers))
		}
		if request.Shards > 0 {
			arguments = append(arguments, "--shards", fmt.Sprint(request.Shards))
		}
		if request.MergeFanIn > 0 {
			arguments = append(arguments, "--merge-fan-in", fmt.Sprint(request.MergeFanIn))
		}
	}
	return arguments
}

func (r Runner) prepareTypeScript(ctx context.Context) error {
	directory := filepath.Join(r.Root, "typescript")
	distribution := filepath.Join(directory, "dist", "cli.js")
	if _, err := os.Stat(distribution); err == nil {
		return nil
	}
	npm, err := findExecutable(npmExecutable())
	if err != nil {
		return err
	}
	if _, err := os.Stat(filepath.Join(directory, "node_modules")); os.IsNotExist(err) {
		command := exec.CommandContext(ctx, npm, "ci", "--silent")
		command.Dir = directory
		if data, runErr := command.CombinedOutput(); runErr != nil {
			return fmt.Errorf("prepare TypeScript dependencies: %w\n%s", runErr, strings.TrimSpace(string(data)))
		}
	}
	command := exec.CommandContext(ctx, npm, "run", "build", "--silent")
	command.Dir = directory
	if data, err := command.CombinedOutput(); err != nil {
		return fmt.Errorf("build TypeScript adapter: %w\n%s", err, strings.TrimSpace(string(data)))
	}
	return nil
}

func findExecutable(names ...string) (string, error) {
	for _, name := range names {
		if path, err := exec.LookPath(name); err == nil {
			return path, nil
		}
	}
	home, _ := os.UserHomeDir()
	for _, name := range names {
		for _, candidate := range commonExecutablePaths(home, name) {
			if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
				return candidate, nil
			}
		}
	}
	return "", fmt.Errorf("required executable not found: %s", strings.Join(names, " or "))
}

func commonExecutablePaths(home, name string) []string {
	if home == "" {
		return nil
	}
	executable := name
	if runtime.GOOS == "windows" && filepath.Ext(executable) == "" {
		executable += ".exe"
	}
	return []string{
		filepath.Join(home, ".cargo", "bin", executable),
		filepath.Join(home, "go", "bin", executable),
	}
}

func joinPathList(first, remainder string) string {
	if remainder == "" {
		return first
	}
	return first + string(os.PathListSeparator) + remainder
}

func npmExecutable() string {
	if runtime.GOOS == "windows" {
		return "npm.cmd"
	}
	return "npm"
}
