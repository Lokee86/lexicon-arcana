//go:build special

package tagged

import (
	"os"
	"os/exec"
	"testing"
)

func TestTagged(t *testing.T) {
	if Enabled() {
		t.Fatal("unexpected")
	}
	exe, _ := os.Executable()
	cmd := exec.Command(exe)
	_ = cmd.Start()
	_ = append([]int{}, 1)
}
