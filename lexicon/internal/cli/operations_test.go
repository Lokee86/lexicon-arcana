package cli

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/Lokee86/lexicon/internal/config"
	"github.com/Lokee86/lexicon/internal/consumer"
)

func TestParseLanguageSelection(t *testing.T) {
	for _, value := range []string{"", "all", "ALL"} {
		got, err := parseLanguageSelection(value)
		if err != nil || got != nil {
			t.Fatalf("parseLanguageSelection(%q) = %v, %v", value, got, err)
		}
	}
	got, err := parseLanguageSelection("python, go,python")
	if err != nil {
		t.Fatal(err)
	}
	if want := []string{"go", "python"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("selection = %v, want %v", got, want)
	}
	if _, err := parseLanguageSelection("klingon"); err == nil {
		t.Fatal("unsupported language was accepted")
	}
}

func TestConsumerNameValidation(t *testing.T) {
	if got, err := consumerFileName("arcana"); err != nil || got != "arcana.json" {
		t.Fatalf("consumerFileName = %q, %v", got, err)
	}
	for _, name := range []string{"../arcana", `dir\\arcana`, ""} {
		if _, err := consumerFileName(name); err == nil {
			t.Fatalf("consumerFileName(%q) succeeded", name)
		}
	}
}

func TestConsumerAddListAndRemoveCommands(t *testing.T) {
	repository := t.TempDir()
	adapterRoot := filepath.Join(repository, "adapters")
	if err := os.MkdirAll(filepath.Join(adapterRoot, "python"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := config.Save(repository, adapterRoot); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	code := Run([]string{
		"consumer", "add", "--repo", repository,
		"--name", "arcana", "--command", "arcana",
		"--arg", "sync", "--timeout", "5s",
	}, &stdout, &stderr)
	if code != 0 {
		t.Fatalf("consumer add = %d, stderr = %s", code, stderr.String())
	}
	definition, err := consumer.Validate(filepath.Join(config.StateRoot(repository), "consumers", "arcana.json"))
	if err != nil {
		t.Fatal(err)
	}
	if definition.Command != "arcana" || !reflect.DeepEqual(definition.Args, []string{"sync"}) || definition.Timeout.String() != "5s" {
		t.Fatalf("definition = %#v", definition)
	}

	stdout.Reset()
	stderr.Reset()
	if code := Run([]string{"consumer", "list", "--repo", repository}, &stdout, &stderr); code != 0 {
		t.Fatalf("consumer list = %d, stderr = %s", code, stderr.String())
	}
	if stdout.String() != "arcana\n" {
		t.Fatalf("consumer list output = %q", stdout.String())
	}

	stdout.Reset()
	stderr.Reset()
	if code := Run([]string{"consumer", "remove", "--repo", repository, "--name", "arcana"}, &stdout, &stderr); code != 0 {
		t.Fatalf("consumer remove = %d, stderr = %s", code, stderr.String())
	}
	if _, err := os.Stat(filepath.Join(config.StateRoot(repository), "consumers", "arcana.json")); !os.IsNotExist(err) {
		t.Fatalf("consumer definition still exists: %v", err)
	}
}

func TestInitWithLanguagesPropagatesInitializationFailure(t *testing.T) {
	repository := t.TempDir()
	if err := os.WriteFile(filepath.Join(repository, "main.py"), []byte("print('hello')\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	adapterRoot := t.TempDir()

	var stdout, stderr bytes.Buffer
	code := Run([]string{
		"init", "--repo", repository,
		"--adapters", adapterRoot,
		"--languages", "python",
	}, &stdout, &stderr)
	if code == 0 {
		t.Fatalf("init unexpectedly succeeded: stdout=%q stderr=%q", stdout.String(), stderr.String())
	}
	if strings.Contains(stdout.String(), "snapshot:") {
		t.Fatalf("failed init reported a snapshot: %q", stdout.String())
	}
}

func TestLanguagesListReportsConfiguredSelection(t *testing.T) {
	repository := t.TempDir()
	adapterRoot := filepath.Join(repository, "adapters")
	if err := os.MkdirAll(filepath.Join(adapterRoot, "python"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := config.SaveWithEnabledLanguages(repository, adapterRoot, []string{"python"}); err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	if err := runLanguages(context.Background(), []string{"--repo", repository}, &stdout, &stderr); err != nil {
		t.Fatalf("languages list: %v, stderr = %s", err, stderr.String())
	}
	if !strings.Contains(stdout.String(), "enabled languages: python") {
		t.Fatalf("languages output = %q", stdout.String())
	}
}

func TestDemonReplacesDaemonCommand(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Run([]string{"help"}, &stdout, &stderr); code != 0 {
		t.Fatalf("help returned %d", code)
	}
	if !strings.Contains(stdout.String(), "demon") || strings.Contains(stdout.String(), "daemon") {
		t.Fatalf("unexpected help output: %q", stdout.String())
	}

	stdout.Reset()
	stderr.Reset()
	if code := Run([]string{"daemon"}, &stdout, &stderr); code != 2 {
		t.Fatalf("daemon returned %d, want 2", code)
	}
	if !strings.Contains(stderr.String(), `unknown command "daemon"`) {
		t.Fatalf("unexpected daemon error: %q", stderr.String())
	}
}
