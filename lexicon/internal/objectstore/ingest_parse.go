package objectstore

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type rawRecord struct {
	Record string `json:"record"`
	Owner  string `json:"owner"`
	Path   string `json:"path"`
	Kind   string `json:"kind"`
	Source string `json:"source"`
	Span   *struct {
		Path string `json:"path"`
	} `json:"span"`
}

type parsedRecord struct {
	raw   json.RawMessage
	value rawRecord
	typed typedRecord
}

func parseOutput(path string) (Header, []parsedRecord, error) {
	file, err := os.Open(path)
	if err != nil {
		return Header{}, nil, err
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 64*1024), 32*1024*1024)
	if !scanner.Scan() {
		return Header{}, nil, fmt.Errorf("adapter output is empty: %s", path)
	}
	var header Header
	if err := json.Unmarshal(scanner.Bytes(), &header); err != nil {
		return Header{}, nil, fmt.Errorf("decode adapter header: %w", err)
	}
	if err := validateHeader(header, header.Language, path); err != nil {
		return Header{}, nil, err
	}
	records := make([]parsedRecord, 0)
	for scanner.Scan() {
		line := bytes.TrimSpace(scanner.Bytes())
		if len(line) == 0 {
			continue
		}
		var compact bytes.Buffer
		if err := json.Compact(&compact, line); err != nil {
			return Header{}, nil, fmt.Errorf("decode adapter record: %w", err)
		}
		raw := append(json.RawMessage(nil), compact.Bytes()...)
		typed, err := parseTypedRecord(raw)
		if err != nil {
			return Header{}, nil, fmt.Errorf("decode adapter record: %w", err)
		}
		records = append(records, parsedRecord{raw: raw, value: typed.ownership(), typed: typed})
	}
	if err := scanner.Err(); err != nil {
		return Header{}, nil, err
	}
	return header, records, nil
}

func parsePartitionedOutput(reader io.Reader, source string) (Header, *analysisPartitions, error) {
	scanner := bufio.NewScanner(reader)
	scanner.Buffer(make([]byte, 64*1024), 32*1024*1024)
	if !scanner.Scan() {
		return Header{}, nil, fmt.Errorf("adapter output is empty: %s", source)
	}
	var header Header
	if err := json.Unmarshal(scanner.Bytes(), &header); err != nil {
		return Header{}, nil, fmt.Errorf("decode adapter header: %w", err)
	}

	partitions := &analysisPartitions{groups: make(map[string]typedRecords)}
	owners := make(map[string]string)
	relationshipsStarted := false
	for scanner.Scan() {
		line := bytes.TrimSpace(scanner.Bytes())
		if len(line) == 0 {
			continue
		}
		typed, err := parseTypedRecord(line)
		if err != nil {
			return Header{}, nil, fmt.Errorf("decode adapter record: %w", err)
		}
		if typed.kind == recordNode {
			if relationshipsStarted {
				return Header{}, nil, fmt.Errorf("adapter output is not canonical: node record follows relationship records")
			}
			ownership := typed.ownership()
			if identity := typed.nodeID(); identity != "" {
				if owner := directOwner(ownership); owner != "" {
					owners[identity] = owner
				}
			}
		} else {
			relationshipsStarted = true
		}

		owner := recordOwner(typed.ownership(), owners)
		if owner == "" {
			partitions.shared.append(typed)
			continue
		}
		group := partitions.groups[owner]
		group.append(typed)
		partitions.groups[owner] = group
	}
	if err := scanner.Err(); err != nil {
		return Header{}, nil, err
	}
	return header, partitions, nil
}

func ValidateOutput(path, language string) error {
	header, _, err := parseOutput(path)
	if err != nil {
		return err
	}
	return validateFullHeader(header, language, path)
}

func validateHeader(header Header, language, path string) error {
	if header.Record != "lexicon" || header.SchemaVersion != 1 || header.AdapterVersion == "" || header.Language == "" || header.Repository == "" {
		return fmt.Errorf("invalid Lexicon adapter header in %s", path)
	}
	if header.Language != language {
		return fmt.Errorf("adapter output language %q does not match %q", header.Language, language)
	}
	if header.Mode != "" && header.Mode != "full" && header.Mode != "incremental" {
		return fmt.Errorf("unsupported adapter output mode %q", header.Mode)
	}
	return nil
}

func validateFullHeader(header Header, language, path string) error {
	if err := validateHeader(header, language, path); err != nil {
		return err
	}
	if header.Mode == "incremental" {
		return fmt.Errorf("application requires full adapter output, got mode %q", header.Mode)
	}
	return nil
}

func nodeOwners(records []parsedRecord) map[string]string {
	owners := make(map[string]string)
	for _, record := range records {
		if record.value.Record != "node" {
			continue
		}
		identity := record.typed.nodeID()
		if identity == "" {
			continue
		}
		if owner := directOwner(record.value); owner != "" {
			owners[identity] = owner
		}
	}
	return owners
}

func recordOwner(record rawRecord, owners map[string]string) string {
	if owner := directOwner(record); owner != "" {
		return owner
	}
	return normalizeOwner(owners[record.Source])
}

func directOwner(record rawRecord) string {
	if record.Owner != "" {
		return normalizeOwner(record.Owner)
	}
	if record.Span != nil && record.Span.Path != "" {
		return normalizeOwner(record.Span.Path)
	}
	if record.Record == "node" && record.Kind == "file" {
		return normalizeOwner(record.Path)
	}
	return ""
}

func normalizeOwner(path string) string {
	path = filepath.ToSlash(path)
	if path == "" || filepath.IsAbs(filepath.FromSlash(path)) {
		return ""
	}
	for _, part := range strings.Split(path, "/") {
		if part == ".." {
			return ""
		}
	}
	path = filepath.ToSlash(filepath.Clean(filepath.FromSlash(path)))
	if path == "." || path == "" {
		return ""
	}
	return path
}
