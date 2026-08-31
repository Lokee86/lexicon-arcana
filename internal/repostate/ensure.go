package repostate

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/Lokee86/grimoire/internal/repostatefs"
)

var repositoryLocks sync.Map

func now() time.Time { return time.Now() }

func elapsedMS(start time.Time) int64 { return time.Since(start).Milliseconds() }

func Ensure(ctx context.Context, options Options) (Status, error) {
	started := now()
	location, err := normalize(options)
	if err != nil {
		return Status{}, err
	}
	mode := options.Mode
	if mode == "" {
		mode = CurrentOnly
	}
	if mode != CurrentOnly && mode != RefreshIfNeeded && mode != ForceRefresh {
		return Status{}, fmt.Errorf("unsupported repository state mode %q", mode)
	}
	if mode != CurrentOnly {
		for _, state := range []string{location.lexicon, location.arcana, location.grimoire} {
			if err := repostatefs.Prepare(location.root, state); err != nil {
				return Status{}, fmt.Errorf("prepare repository state directory: %w", err)
			}
		}
	}
	inspectionStarted := now()
	status, err := inspect(ctx, location)
	if err != nil {
		return Status{}, err
	}
	status.Timings.InitialInspectionMS = elapsedMS(inspectionStarted)
	status.Mode = mode
	status.ElapsedMS = elapsedMS(started)
	status.Timings.TotalMS = status.ElapsedMS
	if mode == CurrentOnly {
		return status, nil
	}
	if mode == RefreshIfNeeded && repositoryStateCurrent(status) {
		quickFingerprint := ""
		if !status.Repository.GitAvailable {
			quickFingerprint, err = quickSourceFingerprint(location.root)
			if err != nil {
				status.Warnings = append(status.Warnings, "inspect repository freshness metadata: "+err.Error())
			}
		}
		if !preparedMarkersTrackRepository(location, status.Repository, quickFingerprint) {
			if markerErr := writeMarkersTimed(&status, location, status.Repository.SourceFingerprint, currentLexiconSnapshot(status), status.Repository.GitHead); markerErr != nil {
				status.Warnings = append(status.Warnings, "update repository freshness metadata: "+markerErr.Error())
			}
		}
		status.ElapsedMS = elapsedMS(started)
		status.Timings.TotalMS = status.ElapsedMS
		return status, nil
	}

	lockStarted := now()
	guard := lockFor(location.grimoire)
	guard.Lock()
	defer guard.Unlock()
	fileGuard, err := acquireFileLock(ctx, filepath.Join(location.grimoire, "repostate.lock"))
	if err != nil {
		return failStatus(status, started, fmt.Errorf("acquire repository refresh lock: %w", err))
	}
	defer fileGuard.Close()
	priorTimings := status.Timings
	priorTimings.LockWaitMS += elapsedMS(lockStarted)

	// Another caller may have completed the work while this caller waited.
	reinspectionStarted := now()
	status, err = inspect(ctx, location)
	if err != nil {
		return Status{}, err
	}
	priorTimings.ReinspectionMS += elapsedMS(reinspectionStarted)
	status.Timings = priorTimings
	status.Mode = mode
	lexiconChanged := false
	arcanaChanged := false
	if mode == ForceRefresh || status.Lexicon.Status != "current" {
		command := "scan"
		if mode == ForceRefresh {
			command = "rebuild"
		}
		if status.Lexicon.Status == "absent" || !fileExists(filepath.Join(location.lexicon, "config.json")) {
			command = "init"
		}
		refreshErr := perform(ctx, &status, "refresh-lexicon", options.Run, ProcessCommand{
			Executable:  commandFor(options.LexiconCommand, "lexicon"),
			Arguments:   []string{command, "--repo", location.root},
			Environment: commandEnvironment("LEXICON_STATE_DIR", location.lexicon),
		})
		if refreshErr != nil {
			status.Warnings = append(status.Warnings, "Lexicon refresh unavailable; continuing with source analysis: "+refreshErr.Error())
		} else if markerErr := markLexiconPrepared(location, status.Repository.SourceFingerprint, status.Repository.GitHead); markerErr != nil {
			status.Warnings = append(status.Warnings, "Lexicon preparation metadata unavailable; continuing with source analysis: "+markerErr.Error())
		} else {
			lexiconChanged = true
		}
		status, err = reinspect(ctx, location, status, mode)
		if err != nil {
			return Status{}, err
		}
		status.Mode = mode
		if refreshErr == nil && status.Lexicon.Status != "current" {
			status.Warnings = append(status.Warnings, "Lexicon refresh did not produce a current snapshot; continuing with source analysis: "+strings.Join(status.Lexicon.StaleReasons, "; "))
			lexiconChanged = false
		}
	}
	if status.Lexicon.Status == "current" && status.Lexicon.Snapshot != "" && (mode == ForceRefresh || status.Arcana.Status != "current" || lexiconChanged) {
		syncErr := perform(ctx, &status, "synchronize-arcana", options.Run, ProcessCommand{
			Executable: commandFor(options.ArcanaCommand, "arcana"),
			Arguments:  []string{"sync", "--lexicon", location.lexicon, "--state", location.arcana},
		})
		if syncErr != nil {
			status.Warnings = append(status.Warnings, "Arcana synchronization unavailable; continuing without graph traversal: "+syncErr.Error())
		} else {
			arcanaChanged = true
		}
		status, err = reinspect(ctx, location, status, mode)
		if err != nil {
			return Status{}, err
		}
		status.Mode = mode
		if syncErr == nil && status.Arcana.Status != "current" {
			status.Warnings = append(status.Warnings, "Arcana synchronization did not produce a current graph; continuing without graph traversal: "+strings.Join(status.Arcana.StaleReasons, "; "))
			arcanaChanged = false
		}
	}
	if mode == ForceRefresh || status.Grimoire.Status != "current" || lexiconChanged || arcanaChanged {
		arguments := []string{"index", "--root", location.root, "--state", location.grimoire}
		if status.Lexicon.Status == "current" && status.Lexicon.Snapshot != "" {
			arguments = append(arguments, "--lexicon-state", location.lexicon, "--lexicon-command", commandFor(options.LexiconCommand, "lexicon"))
		}
		if err := perform(ctx, &status, "prepare-grimoire", options.Run, ProcessCommand{
			Executable: commandFor(options.GrimoireCommand, "grimoire"),
			Arguments:  arguments,
		}); err != nil {
			return failStatus(status, started, err)
		}
		if err := writeMarkersTimed(&status, location, status.Repository.SourceFingerprint, currentLexiconSnapshot(status), status.Repository.GitHead); err != nil {
			return failStatus(status, started, err)
		}
	}
	status, err = reinspect(ctx, location, status, mode)
	if err != nil {
		return Status{}, err
	}
	status.Mode = mode
	if mode == ForceRefresh || status.Knowledge.Status != "current" {
		arguments := []string{"knowledge", "index", "--root", location.root, "--state", location.knowledge}
		if err := perform(ctx, &status, "prepare-knowledge", options.Run, ProcessCommand{
			Executable: commandFor(options.GrimoireCommand, "grimoire"),
			Arguments:  arguments,
		}); err != nil {
			status.Warnings = append(status.Warnings, "knowledge indexing unavailable; continuing with code-only retrieval: "+err.Error())
		}
		status, err = reinspect(ctx, location, status, mode)
		if err != nil {
			return Status{}, err
		}
		status.Mode = mode
	}
	verificationStarted := now()
	finalFingerprint, fingerprintErr := fingerprintRepository(location.root)
	status.Timings.FinalVerificationMS += elapsedMS(verificationStarted)
	if fingerprintErr != nil {
		return failStatus(status, started, fmt.Errorf("verify source after repository preparation: %w", fingerprintErr))
	}
	if finalFingerprint != status.Repository.SourceFingerprint {
		return failStatus(status, started, errors.New("repository source changed during preparation; retry the request"))
	}
	if status.Grimoire.Status == "current" {
		if err := writeMarkersTimed(&status, location, finalFingerprint, currentLexiconSnapshot(status), status.Repository.GitHead); err != nil {
			return failStatus(status, started, err)
		}
		// Marker writes are deliberately excluded from the source fingerprint.
	}
	status.DeterministicQueryReady = status.Grimoire.Status == "current"
	status.ElapsedMS = elapsedMS(started)
	status.Timings.TotalMS = status.ElapsedMS
	return status, nil
}

func perform(ctx context.Context, status *Status, name string, runner CommandRunner, command ProcessCommand) error {
	started := now()
	action := Action{Name: name, Status: "running"}
	status.Actions = append(status.Actions, action)
	if runner == nil {
		runner = runCommand
	}
	err := runner(ctx, command)
	last := &status.Actions[len(status.Actions)-1]
	last.ElapsedMS = elapsedMS(started)
	status.Timings.addAction(name, last.ElapsedMS)
	if err != nil {
		last.Status = "failed"
		last.Error = err.Error()
		return fmt.Errorf("%s: %w", name, err)
	}
	last.Status = "completed"
	return nil
}

func failStatus(status Status, started time.Time, err error) (Status, error) {
	status.ElapsedMS = elapsedMS(started)
	status.Timings.TotalMS = status.ElapsedMS
	status.Error = err.Error()
	status.Warnings = append(status.Warnings, err.Error())
	status.DeterministicQueryReady = false
	return status, err
}

func writeMarkersTimed(status *Status, location paths, fingerprint, lexiconID, gitHead string) error {
	started := now()
	err := writeMarkers(location, fingerprint, lexiconID, gitHead)
	status.Timings.MarkerWriteMS += elapsedMS(started)
	return err
}

func writeMarkers(location paths, fingerprint, lexiconID, gitHead string) error {
	quickFingerprint := ""
	var err error
	if gitHead == "" {
		quickFingerprint, err = quickSourceFingerprint(location.root)
		if err != nil {
			return fmt.Errorf("fingerprint repository metadata: %w", err)
		}
	}
	marker := stateMarker{
		SourceFingerprint: fingerprint,
		QuickFingerprint:  quickFingerprint,
		LexiconSnapshot:   lexiconID,
		GitHead:           gitHead,
	}
	data, err := marshalJSON(marker)
	if err != nil {
		return err
	}
	if lexiconID != "" {
		if err := atomicWrite(filepath.Join(location.lexicon, ".repostate.json"), data); err != nil {
			return fmt.Errorf("write Lexicon preparation metadata: %w", err)
		}
	}
	if err := atomicWrite(filepath.Join(location.grimoire, ".repostate.json"), data); err != nil {
		return fmt.Errorf("write Grimoire preparation metadata: %w", err)
	}
	return nil
}

func markLexiconPrepared(location paths, fingerprint, gitHead string) error {
	id, err := readCurrent(filepath.Join(location.lexicon, "CURRENT"))
	if err != nil {
		return fmt.Errorf("read Lexicon CURRENT after refresh: %w", err)
	}
	quickFingerprint := ""
	if gitHead == "" {
		quickFingerprint, err = quickSourceFingerprint(location.root)
		if err != nil {
			return fmt.Errorf("fingerprint repository metadata: %w", err)
		}
	}
	data, err := marshalJSON(stateMarker{
		SourceFingerprint: fingerprint,
		QuickFingerprint:  quickFingerprint,
		LexiconSnapshot:   id,
		GitHead:           gitHead,
	})
	if err != nil {
		return err
	}
	if err := atomicWrite(filepath.Join(location.lexicon, ".repostate.json"), data); err != nil {
		return fmt.Errorf("write Lexicon preparation metadata: %w", err)
	}
	return nil
}

func reinspect(ctx context.Context, location paths, previous Status, mode Mode) (Status, error) {
	started := now()
	status, err := inspectWithFingerprint(ctx, location, previous.Repository.SourceFingerprint)
	if err != nil {
		return Status{}, err
	}
	status.Mode = mode
	status.Actions = previous.Actions
	status.Timings = previous.Timings
	status.Timings.ReinspectionMS += elapsedMS(started)
	status.Warnings = mergeWarnings(previous.Warnings, status.Warnings)
	return status, nil
}

func marshalJSON(value any) ([]byte, error) {
	data, err := jsonMarshalIndent(value)
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}

func atomicWrite(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	temporary := fmt.Sprintf("%s.tmp-%d", path, os.Getpid())
	if err := os.WriteFile(temporary, data, 0o644); err != nil {
		return err
	}
	if err := os.Rename(temporary, path); err != nil {
		_ = os.Remove(temporary)
		return err
	}
	return nil
}

func lockFor(path string) *sync.Mutex {
	value, _ := repositoryLocks.LoadOrStore(path, &sync.Mutex{})
	return value.(*sync.Mutex)
}
