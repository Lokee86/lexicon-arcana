package adapters

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/Lokee86/lexicon/internal/config"
	languageRegistry "github.com/Lokee86/lexicon/internal/languages"
)

const SchemaVersion = 1

type Definition = languageRegistry.Definition

var ignoredFingerprintDirectories = map[string]struct{}{
	".arcana": {}, ".bundle": {}, ".cantrip": {}, ".ddocs": {}, ".git": {},
	".grimoire": {}, ".homunculus": {}, ".import": {}, ".incubus": {},
	".lexicon": {}, ".next": {}, ".pitlord": {}, ".pytest_cache": {},
	".ritual": {}, ".venv": {}, ".warlock": {}, ".worktrees": {},
	".workingtrees": {}, "__pycache__": {}, "bin": {}, "build": {},
	"coverage": {}, "dist": {}, "log": {}, "node_modules": {}, "obj": {},
	"target": {}, "test": {}, "testdata": {}, "tests": {}, "tmp": {}, "vendor": {}, "venv": {},
}

func Definitions() []Definition {
	return languageRegistry.Definitions()
}

func Lookup(language string) (Definition, bool) {
	return languageRegistry.Lookup(language)
}

func Fingerprint(root, language string) (string, error) {
	return FingerprintWithVersions(root, language, SchemaVersion, config.Version)
}

func FingerprintWithVersions(root, language string, schemaVersion, configVersion int) (string, error) {
	definition, ok := Lookup(language)
	if !ok {
		return "", fmt.Errorf("unsupported language %q", language)
	}
	adapterRoot := filepath.Join(root, definition.Directory)
	paths, err := adapterFiles(adapterRoot)
	if err != nil {
		return "", fmt.Errorf("list %s adapter files: %w", language, err)
	}

	hash := sha256.New()
	writeFingerprintField(hash, "lexicon:adapter-fingerprint:v1")
	writeFingerprintField(hash, definition.Language)
	writeFingerprintField(hash, definition.Directory)
	writeFingerprintField(hash, strconv.Itoa(schemaVersion))
	writeFingerprintField(hash, strconv.Itoa(configVersion))
	for _, path := range paths {
		data, err := os.ReadFile(filepath.Join(adapterRoot, filepath.FromSlash(path)))
		if err != nil {
			return "", fmt.Errorf("read %s adapter file %s: %w", language, path, err)
		}
		writeFingerprintField(hash, path)
		writeFingerprintField(hash, string(data))
	}
	return "sha256:" + hex.EncodeToString(hash.Sum(nil)), nil
}

func adapterFiles(root string) ([]string, error) {
	paths := []string{}
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			if _, ignored := ignoredFingerprintDirectories[strings.ToLower(entry.Name())]; ignored {
				return filepath.SkipDir
			}
			return nil
		}
		if ignoredFingerprintFile(entry.Name()) {
			return nil
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		paths = append(paths, filepath.ToSlash(relative))
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Strings(paths)
	return paths, nil
}

func ignoredFingerprintFile(name string) bool {
	lower := strings.ToLower(name)
	return strings.HasSuffix(lower, "_test.go") ||
		(strings.HasSuffix(lower, ".py") && (strings.HasPrefix(lower, "test_") || strings.HasSuffix(lower, "_test.py"))) ||
		strings.HasSuffix(lower, ".test.ts") || strings.HasSuffix(lower, ".test.tsx") ||
		strings.HasSuffix(lower, ".spec.ts") || strings.HasSuffix(lower, ".spec.tsx")
}

func writeFingerprintField(hash interface{ Write([]byte) (int, error) }, value string) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	_, _ = hash.Write(length[:])
	_, _ = hash.Write([]byte(value))
}
