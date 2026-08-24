//go:build windows

package repostatefs

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"unsafe"
)

func TestPrepareHidesStateWithoutHidingOrdinaryParent(t *testing.T) {
	root := t.TempDir()
	parent := filepath.Join(root, "docs")
	state := filepath.Join(parent, ".ddocs")
	if err := Prepare(root, state); err != nil {
		t.Fatal(err)
	}
	stateAttributes, err := fileAttributes(state)
	if err != nil {
		t.Fatal(err)
	}
	if stateAttributes&fileAttributeHidden == 0 {
		t.Fatalf("state directory is not hidden: %#x", stateAttributes)
	}
	parentAttributes, err := fileAttributes(parent)
	if err != nil {
		t.Fatal(err)
	}
	if parentAttributes&fileAttributeHidden != 0 {
		t.Fatalf("ordinary parent was hidden: %#x", parentAttributes)
	}
}

func fileAttributes(path string) (uint32, error) {
	if _, err := os.Stat(path); err != nil {
		return 0, err
	}
	value, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return 0, err
	}
	attributes, _, callErr := getFileAttributesProc.Call(uintptr(unsafe.Pointer(value)))
	if uint32(attributes) == invalidFileAttributes {
		return 0, windowsCallError("GetFileAttributesW", callErr)
	}
	return uint32(attributes), nil
}
