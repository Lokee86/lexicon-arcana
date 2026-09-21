package cli

import (
	"context"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/Lokee86/lexicon/internal/config"
	"github.com/Lokee86/lexicon/internal/scan"
	lexwatch "github.com/Lokee86/lexicon/internal/watch"
)

func Run(arguments []string, stdout, stderr io.Writer) int {
	if len(arguments) == 0 {
		usage(stderr)
		return 2
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	var err error
	switch arguments[0] {
	case "init":
		err = runInit(ctx, arguments[1:], stdout, stderr)
	case "scan":
		err = runScan(ctx, arguments[1:], stdout, stderr)
	case "demon":
		err = runDemon(ctx, arguments[1:], stdout, stderr)
	case "rebuild":
		err = runRebuild(ctx, arguments[1:], stdout, stderr)
	case "export":
		err = runExport(arguments[1:], stdout, stderr)
	case "gc":
		err = runGC(arguments[1:], stdout, stderr)
	case "languages":
		err = runLanguages(ctx, arguments[1:], stdout, stderr)
	case "consumer":
		err = runConsumer(ctx, arguments[1:], stdout, stderr)
	case "status":
		err = runStatus(arguments[1:], stdout, stderr)
	case "doctor":
		err = runDoctor(arguments[1:], stdout, stderr)
	case "version":
		err = runVersion(stdout)
	case "help", "-h", "--help":
		usage(stdout)
		return 0
	default:
		fmt.Fprintf(stderr, "unknown command %q\n", arguments[0])
		usage(stderr)
		return 2
	}
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	return 0
}

func runInit(ctx context.Context, arguments []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("init", flag.ContinueOnError)
	flags.SetOutput(stderr)
	repository := flags.String("repo", "", "repository to initialize")
	adapterRoot := flags.String("adapters", "", "Lexicon adapters directory")
	languageText := flags.String("languages", "", "comma-separated languages or all")
	if err := flags.Parse(arguments); err != nil {
		return err
	}
	root, err := initRepository(*repository)
	if err != nil {
		return err
	}
	adapters, err := config.FindAdapterRoot(root, *adapterRoot)
	if err != nil {
		return err
	}
	var report scan.Report
	if flagWasSet(flags, "languages") {
		selection, selectionErr := parseLanguageSelection(*languageText)
		if selectionErr != nil {
			return selectionErr
		}
		_, report, err = scan.InitializeWithLanguages(ctx, root, adapters, selection, stdout)
	} else {
		_, report, err = scan.Initialize(ctx, root, adapters, stdout)
	}
	if err != nil {
		return err
	}
	fmt.Fprintf(stdout, "initialized Lexicon: %s\n", root)
	if len(report.Languages) > 0 {
		fmt.Fprintf(stdout, "libraries: %s\n", strings.Join(report.Languages, ", "))
	}
	fmt.Fprintf(stdout, "snapshot: %s\n", report.SnapshotID)
	return nil
}

func runScan(ctx context.Context, arguments []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("scan", flag.ContinueOnError)
	flags.SetOutput(stderr)
	repository := flags.String("repo", "", "repository to scan")
	if err := flags.Parse(arguments); err != nil {
		return err
	}
	root, err := resolveRepository(*repository)
	if err != nil {
		return err
	}
	scanner, err := scan.Open(root, stdout)
	if err != nil {
		return err
	}
	report, err := scanner.Scan(ctx)
	if err != nil {
		return err
	}
	if len(report.Changed) == 0 {
		if len(report.Languages) > 0 {
			fmt.Fprintf(stdout, "rebuilt libraries: %s\n", strings.Join(report.Languages, ", "))
			fmt.Fprintf(stdout, "snapshot: %s\n", report.SnapshotID)
			return nil
		}
		fmt.Fprintf(stdout, "Lexicon is current: %s\n", report.SnapshotID)
		return nil
	}
	fmt.Fprintf(stdout, "updated %d files: %s\n", len(report.Changed), strings.Join(report.Languages, ", "))
	fmt.Fprintf(stdout, "snapshot: %s\n", report.SnapshotID)
	return nil
}

func runDemon(ctx context.Context, arguments []string, stdout, stderr io.Writer) error {
	flags := flag.NewFlagSet("demon", flag.ContinueOnError)
	flags.SetOutput(stderr)
	repository := flags.String("repo", "", "repository to watch")
	debounce := flags.Duration("debounce", 150*time.Millisecond, "quiet period before scanning changes")
	reconcile := flags.Duration("reconcile", 30*time.Second, "full reconciliation interval")
	if err := flags.Parse(arguments); err != nil {
		return err
	}
	root, err := resolveRepository(*repository)
	if err != nil {
		return err
	}
	scanner, err := scan.Open(root, stdout)
	if err != nil {
		return err
	}
	fmt.Fprintf(stdout, "Lexicon demon watching %s\n", scanner.Repository)
	return lexwatch.Run(ctx, scanner, lexwatch.Options{Debounce: *debounce, Reconcile: *reconcile, Output: stderr})
}

func usage(output io.Writer) {
	fmt.Fprintln(output, "Usage: lexicon <init|scan|demon|rebuild|export|gc|languages|consumer|status|doctor|version> [options]")
}
