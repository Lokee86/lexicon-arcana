package objectstore

import (
	"fmt"
	"io"
	"sort"
)

// Analysis is one validated adapter result. File-backed callers may retain
// canonical records for compatibility; streamed adapters are partitioned by
// owner while they are decoded so raw JSON does not remain resident.
type Analysis struct {
	Header     Header
	records    []parsedRecord
	partitions *analysisPartitions
}

type analysisPartitions struct {
	groups map[string]typedRecords
	shared typedRecords
}

func ReadAnalysis(path, language string) (*Analysis, error) {
	header, records, err := parseOutput(path)
	if err != nil {
		return nil, err
	}
	if err := validateAnalysisHeader(header, language, path); err != nil {
		return nil, err
	}
	return &Analysis{Header: header, records: records}, nil
}

func ReadAnalysisReader(reader io.Reader, language, source string) (*Analysis, error) {
	header, partitions, err := parsePartitionedOutput(reader, source)
	if err != nil {
		return nil, err
	}
	if err := validateAnalysisHeader(header, language, source); err != nil {
		return nil, err
	}
	if partitions.len() == 0 {
		return nil, fmt.Errorf("adapter output contains no facts: %s", source)
	}
	return &Analysis{Header: header, partitions: partitions}, nil
}

func (p *analysisPartitions) len() int {
	if p == nil {
		return 0
	}
	total := p.shared.len()
	for _, records := range p.groups {
		total += records.len()
	}
	return total
}

func validateAnalysisHeader(header Header, language, source string) error {
	if err := validateHeader(header, language, source); err != nil {
		return err
	}
	if header.Mode == "incremental" {
		if header.ChangedFiles == nil || header.RemovedFiles == nil {
			return fmt.Errorf("incremental adapter output must declare changed_files and removed_files")
		}
		if header.SharedComplete == nil {
			return fmt.Errorf("incremental adapter output must declare shared_complete")
		}
	}
	return nil
}

func (a *Analysis) IsIncremental() bool {
	return a != nil && a.Header.Mode == "incremental"
}

func (a *Analysis) groups(allowedOwners map[string]struct{}) (map[string]typedRecords, typedRecords) {
	if a.partitions != nil {
		return a.partitionedGroups(allowedOwners)
	}
	owners := nodeOwners(a.records)
	groups := make(map[string]typedRecords)
	shared := typedRecords{}
	for _, record := range a.records {
		owner := recordOwner(record.value, owners)
		if owner == "" {
			shared.append(record.typed)
			continue
		}
		if allowedOwners != nil {
			if _, allowed := allowedOwners[owner]; !allowed {
				shared.append(record.typed)
				continue
			}
		}
		group := groups[owner]
		group.append(record.typed)
		groups[owner] = group
	}
	return groups, shared
}

func (a *Analysis) partitionedGroups(allowedOwners map[string]struct{}) (map[string]typedRecords, typedRecords) {
	if allowedOwners == nil {
		return a.partitions.groups, a.partitions.shared
	}
	groups := make(map[string]typedRecords, len(a.partitions.groups))
	shared := a.partitions.shared.clone()
	owners := make([]string, 0, len(a.partitions.groups))
	for owner := range a.partitions.groups {
		owners = append(owners, owner)
	}
	sort.Strings(owners)
	for _, owner := range owners {
		records := a.partitions.groups[owner]
		if _, allowed := allowedOwners[owner]; !allowed {
			shared.appendAll(records)
			continue
		}
		groups[owner] = records
	}
	return groups, shared
}

func (a *Analysis) allTypedRecords() typedRecords {
	if a.partitions == nil {
		records := typedRecords{}
		for _, record := range a.records {
			records.append(record.typed)
		}
		return records
	}
	records := a.partitions.shared.clone()
	owners := make([]string, 0, len(a.partitions.groups))
	for owner := range a.partitions.groups {
		owners = append(owners, owner)
	}
	sort.Strings(owners)
	for _, owner := range owners {
		records.appendAll(a.partitions.groups[owner])
	}
	return records
}

func samePaths(left, right []string) bool {
	left = normalizedPaths(left)
	right = normalizedPaths(right)
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func normalizedPaths(paths []string) []string {
	result := make([]string, 0, len(paths))
	seen := make(map[string]struct{}, len(paths))
	for _, path := range paths {
		path = normalizeOwner(path)
		if path == "" {
			continue
		}
		if _, exists := seen[path]; exists {
			continue
		}
		seen[path] = struct{}{}
		result = append(result, path)
	}
	sort.Strings(result)
	return result
}
