package repostate

func mergeWarnings(previous, current []string) []string {
	if len(previous) == 0 {
		return append([]string(nil), current...)
	}
	result := append([]string(nil), previous...)
	seen := make(map[string]struct{}, len(previous)+len(current))
	for _, warning := range previous {
		seen[warning] = struct{}{}
	}
	for _, warning := range current {
		if _, exists := seen[warning]; exists {
			continue
		}
		seen[warning] = struct{}{}
		result = append(result, warning)
	}
	return result
}
