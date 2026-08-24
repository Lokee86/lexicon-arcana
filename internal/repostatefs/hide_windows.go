//go:build windows

package repostatefs

import (
	"errors"
	"fmt"
	"syscall"
	"unsafe"
)

const (
	fileAttributeHidden   = 0x2
	invalidFileAttributes = 0xffffffff
)

var (
	kernel32              = syscall.NewLazyDLL("kernel32.dll")
	getFileAttributesProc = kernel32.NewProc("GetFileAttributesW")
	setFileAttributesProc = kernel32.NewProc("SetFileAttributesW")
)

func markHidden(path string) error {
	value, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	attributes, _, callErr := getFileAttributesProc.Call(uintptr(unsafe.Pointer(value)))
	if uint32(attributes) == invalidFileAttributes {
		return windowsCallError("GetFileAttributesW", callErr)
	}
	if uint32(attributes)&fileAttributeHidden != 0 {
		return nil
	}
	result, _, callErr := setFileAttributesProc.Call(
		uintptr(unsafe.Pointer(value)),
		uintptr(uint32(attributes)|fileAttributeHidden),
	)
	if result == 0 {
		return windowsCallError("SetFileAttributesW", callErr)
	}
	return nil
}

func windowsCallError(name string, err error) error {
	if err == nil || errors.Is(err, syscall.Errno(0)) {
		err = errors.New("Windows API call failed")
	}
	return fmt.Errorf("%s: %w", name, err)
}
