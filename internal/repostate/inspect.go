package repostate

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/Lokee86/grimoire/internal/embedding"
	"github.com/Lokee86/grimoire/internal/index"
	"github.com/Lokee86/grimoire/internal/knowledge"
	"github.com/Lokee86/grimoire/internal/knowledgevector"
)

type paths struct {
	root, lexicon, arcana, grimoire, knowledge string
}

type stateMarker struct {
	SourceFingerprint string `json:"source_fingerprint"`
	QuickFingerprint  string `json:"quick_fingerprint,omitempty"`
	LexiconSnapshot   string `json:"lexicon_snapshot,omitempty"`
	GitHead           string `json:"git_head,omitempty"`
}

var fingerprintRepository = sourceFingerprint

func normalize(options Options) (paths, error) {
	root := options.Root
	if root == "" {
		root = "."
	}
	absolute, err := filepath.Abs(root)
	if err != nil {
		return paths{}, fmt.Errorf("resolve repository root: %w", err)
	}
	resolve := func(value, fallback string) string {
		if value == "" {
			return filepath.Join(absolute, fallback)
		}
		if filepath.IsAbs(value) {
			return filepath.Clean(value)
		}
		return filepath.Join(absolute, value)
	}
	grimoire := resolve(options.GrimoireState, ".grimoire")
	return paths{
		root: absolute, lexicon: resolve(options.LexiconState, ".lexicon"),
		arcana: resolve(options.ArcanaState, ".arcana"), grimoire: grimoire,
		knowledge: filepath.Join(grimoire, "knowledge"),
	}, nil
}

func inspect(ctx context.Context, location paths) (Status, error) {
	return inspectWithFingerprint(ctx, location, "")
}

func inspectWithFingerprint(ctx context.Context, location paths, fingerprint string) (Status, error) {
	started := now()
	repository := RepositoryStatus{Root: location.root}
	repository.GitHead, repository.GitDirty, repository.GitAvailable = gitRepositoryStatus(ctx, location.root)
	quickFingerprint := ""
	if !repository.GitAvailable {
		quickFingerprint, _ = quickSourceFingerprint(location.root)
	}
	var err error
	if fingerprint == "" {
		fingerprint = reusablePreparedFingerprint(location, repository, quickFingerprint)
	}
	if fingerprint == "" {
		fingerprint, err = fingerprintRepository(location.root)
		if err != nil {
			return Status{}, fmt.Errorf("fingerprint source: %w", err)
		}
	}
	if !repository.GitAvailable {
		repository.GitDirty = preparedSourceChanged(location, repository, quickFingerprint, fingerprint)
	}
	repository.SourceFingerprint = fingerprint
	status := Status{Version: 2, Repository: repository}
	status.Lexicon = inspectLexicon(location, fingerprint, repository.GitDirty)
	status.Arcana = inspectArcana(location, currentLexiconSnapshot(status))
	for _, warning := range status.Arcana.Warnings {
		status.Warnings = append(status.Warnings, "Arcana compatibility warning: "+warning)
	}
	status.Grimoire = inspectGrimoire(location, fingerprint, status.Lexicon.Snapshot)
	var currentKnowledge knowledge.Index
	var knowledgeAvailable bool
	status.Knowledge, currentKnowledge, knowledgeAvailable = inspectKnowledge(location, fingerprint)
	status.ArcanaVectors = inspectArcanaVectors(location, status.Arcana.Snapshot)
	if status.Arcana.Status != "current" && status.ArcanaVectors.Status == "current" {
		status.ArcanaVectors.Status = "stale"
		status.ArcanaVectors.Reason = "Arcana graph state is not current"
	}
	status.KnowledgeVectors = inspectKnowledgeVectors(location, currentKnowledge, knowledgeAvailable)
	if status.Knowledge.Status != "current" && status.KnowledgeVectors.Status == "current" {
		status.KnowledgeVectors.Status = "stale"
		status.KnowledgeVectors.Reason = "knowledge index is stale; refresh it before trusting documentation vector freshness"
	}
	status.DeterministicQueryReady = status.Grimoire.Status == "current"
	status.ElapsedMS = elapsedMS(started)
	return status, nil
}

func inspectLexicon(location paths, fingerprint string, dirty bool) ComponentStatus {
	id, err := readCurrent(filepath.Join(location.lexicon, "CURRENT"))
	if errors.Is(err, os.ErrNotExist) {
		return ComponentStatus{Status: "absent", StaleReasons: []string{"CURRENT is missing"}}
	}
	if err != nil {
		return ComponentStatus{Status: "failed", StaleReasons: []string{err.Error()}}
	}
	if !validSnapshotID(id) {
		return ComponentStatus{Status: "failed", Snapshot: id, StaleReasons: []string{"CURRENT has an invalid snapshot ID"}}
	}
	if _, err := os.Stat(filepath.Join(location.lexicon, "snapshots", strings.TrimPrefix(id, "sha256:")+".json")); err != nil {
		return ComponentStatus{Status: "failed", Snapshot: id, StaleReasons: []string{"current snapshot manifest is missing"}}
	}
	status := ComponentStatus{Status: "current", Snapshot: id, Prepared: true}
	var marker stateMarker
	if err := readJSON(filepath.Join(location.lexicon, ".repostate.json"), &marker); err == nil {
		if marker.SourceFingerprint != "" && marker.SourceFingerprint != fingerprint {
			status.Status = "stale"
			status.StaleReasons = append(status.StaleReasons, "source fingerprint changed since Lexicon preparation")
		}
	} else if dirty {
		status.Status = "stale"
		status.StaleReasons = append(status.StaleReasons, "Git working tree has source changes")
	}
	return status
}

func inspectArcana(location paths, expected string) ComponentStatus {
	id, err := readCurrent(filepath.Join(location.arcana, "CURRENT"))
	if errors.Is(err, os.ErrNotExist) {
		return ComponentStatus{Status: "absent", Expected: expected, StaleReasons: []string{"CURRENT is missing"}}
	}
	if err != nil {
		return ComponentStatus{Status: "failed", Expected: expected, StaleReasons: []string{err.Error()}}
	}
	status := ComponentStatus{Status: "current", Snapshot: id, Expected: expected}
	if !validSnapshotID(id) {
		status.Status = "failed"
		status.StaleReasons = append(status.StaleReasons, "CURRENT has an invalid snapshot ID")
		return status
	}
	if expected == "" {
		status.Status = "stale"
		status.StaleReasons = append(status.StaleReasons, "Lexicon snapshot is unavailable for alignment")
	} else if id != expected {
		status.Status = "stale"
		status.StaleReasons = append(status.StaleReasons, "Arcana snapshot does not match Lexicon CURRENT")
	}
	directory := filepath.Join(location.arcana, "snapshots", strings.TrimPrefix(id, "sha256:"))
	if warningBytes, warningErr := os.ReadFile(filepath.Join(directory, "compatibility.warnings")); warningErr == nil {
		for _, warning := range strings.Split(strings.TrimSpace(string(warningBytes)), "\n") {
			if warning != "" {
				status.Warnings = append(status.Warnings, warning)
			}
		}
	} else if !errors.Is(warningErr, os.ErrNotExist) {
		status.Warnings = append(status.Warnings, "read Arcana compatibility warnings: "+warningErr.Error())
	}
	if !fileExists(filepath.Join(directory, "repository.manifest")) || !fileExists(filepath.Join(directory, "lexicon.snapshot")) {
		status.Status = "stale"
		status.StaleReasons = append(status.StaleReasons, "Arcana snapshot is incomplete")
	} else if value, readErr := readCurrent(filepath.Join(directory, "lexicon.snapshot")); readErr != nil || value != id {
		status.Status = "stale"
		status.StaleReasons = append(status.StaleReasons, "Arcana lexicon.snapshot does not match CURRENT")
	}
	status.Prepared = status.Status == "current"
	return status
}

func inspectGrimoire(location paths, fingerprint, lexiconID string) ComponentStatus {
	status := ComponentStatus{Status: "absent"}
	snapshot, err := index.Load(location.grimoire)
	if err != nil {
		if !errors.Is(err, os.ErrNotExist) {
			status.Status = "failed"
			status.StaleReasons = []string{err.Error()}
			return status
		}
		status.StaleReasons = []string{"prepared index is missing"}
		return status
	}
	status.Status = "current"
	status.Snapshot = snapshot.Identity()
	status.Prepared = true
	var marker stateMarker
	if err := readJSON(filepath.Join(location.grimoire, ".repostate.json"), &marker); err != nil {
		status.Status = "stale"
		status.Prepared = false
		status.StaleReasons = append(status.StaleReasons, "preparation metadata is missing")
		return status
	}
	if marker.SourceFingerprint != fingerprint {
		status.Status = "stale"
		status.Prepared = false
		status.StaleReasons = append(status.StaleReasons, "source fingerprint changed since Grimoire preparation")
	}
	if marker.LexiconSnapshot != lexiconID {
		status.Status = "stale"
		status.Prepared = false
		status.StaleReasons = append(status.StaleReasons, "prepared index Lexicon snapshot is not current")
	}
	return status
}

func inspectKnowledge(location paths, repositoryFingerprint string) (ComponentStatus, knowledge.Index, bool) {
	stored, err := knowledge.Load(location.knowledge)
	if errors.Is(err, os.ErrNotExist) {
		return ComponentStatus{Status: "absent", StaleReasons: []string{"knowledge index is missing"}}, knowledge.Index{}, false
	}
	if err != nil {
		return ComponentStatus{Status: "failed", StaleReasons: []string{err.Error()}}, knowledge.Index{}, false
	}
	actual := knowledge.Identity(stored)
	status := ComponentStatus{Status: "current", Snapshot: actual, Expected: actual, Prepared: true}
	if stored.SourceFingerprint == "" {
		status.Status = "stale"
		status.Prepared = false
		status.StaleReasons = append(status.StaleReasons, "knowledge freshness metadata is missing")
	} else if stored.SourceFingerprint != repositoryFingerprint {
		status.Status = "stale"
		status.Prepared = false
		status.StaleReasons = append(status.StaleReasons, "repository content changed since knowledge indexing")
	}
	return status, stored, true
}

func inspectKnowledgeVectors(location paths, current knowledge.Index, knowledgeAvailable bool) VectorStatus {
	if !knowledgeAvailable || len(current.Documents) == 0 {
		return VectorStatus{Status: "missing", Reason: "no knowledge index is available"}
	}
	info := knowledgevector.Inspect(location.knowledge, current, "")
	status := VectorStatus{
		Snapshot: info.SnapshotIdentity, Expected: info.ExpectedIdentity,
		Model: info.Model, Count: info.Count, Bytes: info.SnapshotBytes,
	}
	if !info.Available {
		status.Status = "missing"
		status.Reason = info.Error
		if status.Reason == "" {
			status.Reason = "documentation vector snapshot is not prepared"
		}
		return status
	}
	if info.Current {
		status.Status = "current"
		return status
	}
	status.Status = "stale"
	status.Reason = info.Error
	if status.Reason == "" {
		status.Reason = "documentation vector snapshot is not current"
	}
	return status
}

func inspectArcanaVectors(location paths, arcanaID string) VectorStatus {
	if arcanaID == "" || !validSnapshotID(arcanaID) {
		return VectorStatus{Status: "missing", Reason: "no current Arcana snapshot"}
	}
	digest := strings.TrimPrefix(arcanaID, "sha256:")
	base := filepath.Join(location.arcana, "vectors", digest, embedding.Identity())
	manifestPath := filepath.Join(base, "manifest.json")
	status := VectorStatus{Snapshot: arcanaID, Expected: arcanaID}
	if !fileExists(manifestPath) {
		status.Status = "missing"
		status.Reason = "matching vector index is not prepared"
		return status
	}

	repository, err := readArcanaRepositoryIdentity(filepath.Join(location.arcana, "snapshots", digest, "repository.manifest"))
	if err != nil {
		status.Status = "stale"
		status.Reason = "cannot validate current Arcana graph identity: " + err.Error()
		return status
	}
	var manifest arcanaVectorManifest
	if err := readJSON(manifestPath, &manifest); err != nil {
		status.Status = "stale"
		status.Reason = "invalid Arcana vector manifest: " + err.Error()
		return status
	}
	status.Model = manifest.Model
	status.Count = manifest.ItemCount
	if reason := validateArcanaVectorManifest(manifest, repository); reason != "" {
		status.Status = "stale"
		status.Reason = reason
		return status
	}
	vectors := filepath.Join(base, manifest.VectorsFile)
	info, err := os.Stat(vectors)
	if err != nil || info.IsDir() {
		status.Status = "stale"
		status.Reason = "Arcana vector data file is missing"
		return status
	}
	status.Bytes = info.Size()
	expectedBytes := int64(manifest.ItemCount) * int64(manifest.Dimensions) * 4
	if info.Size() != expectedBytes {
		status.Status = "stale"
		status.Reason = fmt.Sprintf("Arcana vector data is %d bytes; expected %d", info.Size(), expectedBytes)
		return status
	}
	if count, err := countArcanaVectorRecords(filepath.Join(base, manifest.RecordsFile)); err != nil || count != manifest.ItemCount {
		status.Status = "stale"
		if err != nil {
			status.Reason = "invalid Arcana vector node records: " + err.Error()
		} else {
			status.Reason = fmt.Sprintf("Arcana vector index has %d node records; expected %d", count, manifest.ItemCount)
		}
		return status
	}
	for _, data := range []struct {
		file, expected, label string
	}{
		{manifest.RecordsFile, manifest.RecordsSHA256, "node records"},
		{manifest.VectorsFile, manifest.VectorsSHA256, "vectors"},
	} {
		actual, err := fileSHA256(filepath.Join(base, data.file))
		if err != nil || actual != data.expected {
			status.Status = "stale"
			status.Reason = "Arcana vector " + data.label + " checksum does not match its manifest"
			return status
		}
	}
	status.Status = "current"
	return status
}

type arcanaVectorManifest struct {
	Version                  int    `json:"version"`
	RepositorySnapshotID     string `json:"repository_snapshot_id"`
	GraphSnapshotID          string `json:"graph_snapshot_id"`
	Model                    string `json:"model"`
	Identity                 string `json:"identity"`
	EligibilityPolicyVersion int    `json:"eligibility_policy_version"`
	Dimensions               int    `json:"dimensions"`
	ItemCount                int    `json:"item_count"`
	RecordsFile              string `json:"records_file"`
	RecordsSHA256            string `json:"records_sha256"`
	VectorsFile              string `json:"vectors_file"`
	VectorsSHA256            string `json:"vectors_sha256"`
}

type arcanaRepositoryIdentity struct {
	SnapshotID      string
	GraphSnapshotID string
}

func readArcanaRepositoryIdentity(path string) (arcanaRepositoryIdentity, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return arcanaRepositoryIdentity{}, err
	}
	values := make(map[string]string)
	for _, line := range strings.Split(strings.TrimSpace(string(data)), "\n") {
		name, value, found := strings.Cut(strings.TrimSpace(line), "=")
		if found {
			values[name] = value
		}
	}
	nodeCount, err := strconv.Atoi(values["node_count"])
	if err != nil || nodeCount < 0 || values["version"] != "1" || !validArcanaHexID(values["snapshot_id"]) || !validArcanaHexID(values["graph_snapshot_id"]) {
		return arcanaRepositoryIdentity{}, errors.New("repository.manifest has invalid vector identity fields")
	}
	return arcanaRepositoryIdentity{
		SnapshotID: values["snapshot_id"], GraphSnapshotID: values["graph_snapshot_id"],
	}, nil
}

// Mirrors Arcana's manifest contract for status inspection; eligibility decisions remain in Arcana.
const arcanaSemanticEligibilityPolicyVersion = 1

func arcanaSemanticIndexIdentity() string {
	return fmt.Sprintf("%s-arcana-semantic-v%d", embedding.Identity(), arcanaSemanticEligibilityPolicyVersion)
}

func validateArcanaVectorManifest(manifest arcanaVectorManifest, repository arcanaRepositoryIdentity) string {
	const maxVectorItems = int64(^uint64(0)>>1) / int64(embedding.Dimensions*4)
	switch {
	case manifest.Version != 3:
		return fmt.Sprintf("unsupported Arcana vector index version %d", manifest.Version)
	case manifest.RepositorySnapshotID != repository.SnapshotID || manifest.GraphSnapshotID != repository.GraphSnapshotID:
		return "Arcana vector index does not match the current graph identity"
	case manifest.Model != embedding.ModelReference || manifest.Identity != arcanaSemanticIndexIdentity() || manifest.Dimensions != embedding.Dimensions:
		return "Arcana vector index does not match Grimoire's embedding contract"
	case manifest.EligibilityPolicyVersion != arcanaSemanticEligibilityPolicyVersion:
		return "Arcana vector index does not match the semantic eligibility policy"
	case manifest.ItemCount < 0:
		return "Arcana vector index has a negative document count"
	case int64(manifest.ItemCount) > maxVectorItems:
		return "Arcana vector index byte length overflows the supported size"
	case manifest.RecordsFile != "nodes.jsonl" || manifest.VectorsFile != "vectors.f32":
		return "Arcana vector index uses unsupported data filenames"
	case !validSHA256(manifest.RecordsSHA256) || !validSHA256(manifest.VectorsSHA256):
		return "Arcana vector index has invalid data checksums"
	default:
		return ""
	}
}

func countArcanaVectorRecords(path string) (int, error) {
	file, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	count := 0
	for scanner.Scan() {
		var record struct {
			NodeKey string `json:"node_key"`
			Kind    string `json:"kind"`
			Name    string `json:"name"`
		}
		if err := json.Unmarshal(scanner.Bytes(), &record); err != nil || !validArcanaHexID(record.NodeKey) || strings.TrimSpace(record.Kind) == "" || strings.TrimSpace(record.Name) == "" {
			if err != nil {
				return 0, err
			}
			return 0, errors.New("node record has invalid required fields")
		}
		count++
	}
	return count, scanner.Err()
}

func validArcanaHexID(value string) bool {
	if len(value) != 16 {
		return false
	}
	for _, character := range value {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}

func validSHA256(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}

func fileSHA256(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return "", err
	}
	return fmt.Sprintf("%x", hash.Sum(nil)), nil
}

func readCurrent(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return "", fmt.Errorf("%w: %s", os.ErrNotExist, path)
		}
		return "", err
	}
	value := strings.TrimSpace(string(data))
	if value == "" {
		return "", fmt.Errorf("%s is empty", path)
	}
	return value, nil
}

func validSnapshotID(value string) bool {
	digest, ok := strings.CutPrefix(value, "sha256:")
	if !ok || len(digest) != 64 {
		return false
	}
	for _, character := range digest {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func readJSON(path string, target any) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	return json.Unmarshal(data, target)
}
