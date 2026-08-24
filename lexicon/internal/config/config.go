package config

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	languageRegistry "github.com/Lokee86/lexicon/internal/languages"
	"github.com/Lokee86/lexicon/internal/repostatefs"
)

const Version = 1

type Config struct {
	Version          int      `json:"version"`
	AdapterRoot      string   `json:"adapter_root"`
	EnabledLanguages []string `json:"enabled_languages,omitempty"`
}

func StateRoot(repository string) string {
	if configured := os.Getenv("LEXICON_STATE_DIR"); configured != "" {
		if filepath.IsAbs(configured) {
			return filepath.Clean(configured)
		}
		return filepath.Join(repository, configured)
	}
	return filepath.Join(repository, ".lexicon")
}

func AnalysisID() string {
	sum := sha256.Sum256([]byte("lexicon:analysis-config:v1"))
	return "sha256:" + hex.EncodeToString(sum[:])
}

func Path(repository string) string {
	return filepath.Join(StateRoot(repository), "config.json")
}

func Save(repository, adapterRoot string) error {
	absolute, err := filepath.Abs(adapterRoot)
	if err != nil {
		return err
	}
	value := Config{Version: Version, AdapterRoot: absolute}
	if existing, loadErr := Load(repository); loadErr == nil {
		value.EnabledLanguages = existing.EnabledLanguages
	}
	return save(repository, value)
}

func SaveWithEnabledLanguages(repository, adapterRoot string, enabledLanguages []string) error {
	absolute, err := filepath.Abs(adapterRoot)
	if err != nil {
		return err
	}
	normalized, err := NormalizeEnabledLanguages(enabledLanguages)
	if err != nil {
		return err
	}
	return save(repository, Config{Version: Version, AdapterRoot: absolute, EnabledLanguages: normalized})
}

func UpdateEnabledLanguages(repository string, enabledLanguages []string) error {
	value, err := Load(repository)
	if err != nil {
		return err
	}
	value.EnabledLanguages, err = NormalizeEnabledLanguages(enabledLanguages)
	if err != nil {
		return err
	}
	return save(repository, value)
}

func NormalizeEnabledLanguages(enabledLanguages []string) ([]string, error) {
	supported := make(map[string]struct{}, len(languageRegistry.Supported()))
	for _, language := range languageRegistry.Supported() {
		supported[language] = struct{}{}
	}
	set := make(map[string]struct{}, len(enabledLanguages))
	for _, language := range enabledLanguages {
		if _, ok := supported[language]; !ok {
			return nil, fmt.Errorf("unsupported Lexicon language %q", language)
		}
		set[language] = struct{}{}
	}
	normalized := make([]string, 0, len(set))
	for language := range set {
		normalized = append(normalized, language)
	}
	sort.Strings(normalized)
	return normalized, nil
}

func (value Config) LanguageEnabled(language string) bool {
	if _, supported := languageRegistry.Lookup(language); !supported {
		return false
	}
	if len(value.EnabledLanguages) == 0 {
		return true
	}
	for _, enabled := range value.EnabledLanguages {
		if enabled == language || (enabled == "generic" && languageRegistry.IsGeneric(language)) {
			return true
		}
	}
	return false
}

func save(repository string, value Config) error {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	if err := repostatefs.Prepare(repository, StateRoot(repository)); err != nil {
		return err
	}
	return os.WriteFile(Path(repository), append(data, '\n'), 0o644)
}

func Load(repository string) (Config, error) {
	data, err := os.ReadFile(Path(repository))
	if err != nil {
		return Config{}, fmt.Errorf("read Lexicon configuration: %w", err)
	}
	var value Config
	if err := json.Unmarshal(data, &value); err != nil {
		return Config{}, fmt.Errorf("decode Lexicon configuration: %w", err)
	}
	if value.Version != Version {
		return Config{}, fmt.Errorf("unsupported Lexicon configuration version %d", value.Version)
	}
	value.EnabledLanguages, err = NormalizeEnabledLanguages(value.EnabledLanguages)
	if err != nil {
		return Config{}, fmt.Errorf("validate Lexicon enabled languages: %w", err)
	}
	return value, nil
}

func FindAdapterRoot(repository, explicit string) (string, error) {
	executable, _ := os.Executable()
	current, _ := os.Getwd()
	return findAdapterRoot(repository, explicit, executable, current)
}

func findAdapterRoot(repository, explicit, executable, current string) (string, error) {
	candidates := []string{explicit, os.Getenv("LEXICON_ADAPTERS")}
	if executable != "" {
		executableDir := filepath.Dir(executable)
		candidates = append(candidates,
			filepath.Join(executableDir, "adapters"),
			filepath.Join(executableDir, "..", "adapters"),
		)
	}
	candidates = append(candidates, filepath.Join(repository, "adapters"))
	if current != "" {
		candidates = append(candidates, filepath.Join(current, "adapters"))
	}
	for _, candidate := range candidates {
		if candidate == "" {
			continue
		}
		absolute, err := filepath.Abs(candidate)
		if err != nil {
			continue
		}
		if info, err := os.Stat(filepath.Join(absolute, "python")); err == nil && info.IsDir() {
			return absolute, nil
		}
	}
	return "", fmt.Errorf("adapter root not found; use --adapters or LEXICON_ADAPTERS")
}
