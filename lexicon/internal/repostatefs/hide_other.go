//go:build !windows

package repostatefs

func markHidden(string) error { return nil }
