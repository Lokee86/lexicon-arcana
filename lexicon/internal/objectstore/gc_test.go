package objectstore

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"testing"
	"time"
)

func TestGCPlansPreservationAndSupportsDryRun(t *testing.T) {
	store := Store{Root: t.TempDir()}
	objects := make([]string, 0, 6)
	for _, owner := range []string{"one.py", "two.py", "shared", "three.py", "four.py", "unreachable.py"} {
		objects = append(objects, writeGCObject(t, store, owner))
	}
	manifests := make([]string, 0, 4)
	for index, objectIndex := range []int{0, 1, 3, 4} {
		sharedID := ""
		if index == 1 {
			sharedID = objects[2]
		}
		id, err := store.Publish(Manifest{
			StateCommit: string(rune('1' + index)),
			Languages: []LanguageEntry{{
				Language:       "python",
				SharedObjectID: sharedID,
				Files:          []FileEntry{{Path: "file.py", ObjectID: objects[objectIndex]}},
			}},
		})
		if err != nil {
			t.Fatal(err)
		}
		manifests = append(manifests, id)
	}
	for index, id := range manifests {
		stamp := time.Unix(int64(index+1), 0)
		if err := os.Chtimes(store.snapshotPath(id), stamp, stamp); err != nil {
			t.Fatal(err)
		}
	}
	pinRoot := filepath.Join(store.Root, "consumer-state")
	if err := os.MkdirAll(pinRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	pin := map[string]string{"snapshot_id": manifests[1]}
	data, err := json.Marshal(pin)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(pinRoot, "consumer.json"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	current := manifests[3]
	oldStamp := time.Unix(1, 0)
	if err := os.Chtimes(store.snapshotPath(current), oldStamp, oldStamp); err != nil {
		t.Fatal(err)
	}

	plan, err := store.PlanGC(GCOptions{KeepSnapshots: 1})
	if err != nil {
		t.Fatal(err)
	}
	if plan.CurrentSnapshot != current {
		t.Fatalf("current = %s, want %s", plan.CurrentSnapshot, current)
	}
	if !reflect.DeepEqual(plan.DeleteSnapshots, []string{manifests[0]}) {
		t.Fatalf("delete snapshots = %v", plan.DeleteSnapshots)
	}
	wantDeleteObjects := []string{objects[0], objects[5]}
	sort.Strings(wantDeleteObjects)
	if !reflect.DeepEqual(plan.DeleteObjects, wantDeleteObjects) {
		t.Fatalf("delete objects = %v", plan.DeleteObjects)
	}

	result, err := store.ExecuteGC(plan, true)
	if err != nil {
		t.Fatal(err)
	}
	if !result.DryRun || !reflect.DeepEqual(result.DeletedSnapshots, plan.DeleteSnapshots) || !reflect.DeepEqual(result.DeletedObjects, plan.DeleteObjects) {
		t.Fatalf("dry-run result = %#v", result)
	}
	for _, id := range plan.DeleteSnapshots {
		if _, err := os.Stat(store.snapshotPath(id)); err != nil {
			t.Fatalf("dry-run removed snapshot %s: %v", id, err)
		}
	}
	for _, id := range plan.DeleteObjects {
		if _, err := os.Stat(store.objectPath(id)); err != nil {
			t.Fatalf("dry-run removed object %s: %v", id, err)
		}
	}

	if _, err := store.ExecuteGC(plan, false); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(store.snapshotPath(manifests[0])); !os.IsNotExist(err) {
		t.Fatalf("deleted snapshot stat error = %v", err)
	}
	for _, id := range objects[:1] {
		if _, err := os.Stat(store.objectPath(id)); !os.IsNotExist(err) {
			t.Fatalf("deleted object stat error = %v", err)
		}
	}
	if _, err := os.Stat(store.objectPath(objects[5])); !os.IsNotExist(err) {
		t.Fatalf("unreachable object stat error = %v", err)
	}
	for _, id := range manifests[1:] {
		if _, err := os.Stat(store.snapshotPath(id)); err != nil {
			t.Fatalf("preserved snapshot %s: %v", id, err)
		}
	}
	for _, id := range objects[1:5] {
		if _, err := os.Stat(store.objectPath(id)); err != nil {
			t.Fatalf("preserved object %s: %v", id, err)
		}
	}
	currentID, _, err := store.Current()
	if err != nil || currentID != current {
		t.Fatalf("CURRENT = %s, err=%v", currentID, err)
	}
}

func TestGCRejectsMalformedConsumerPins(t *testing.T) {
	cases := map[string]string{
		"invalid JSON":  "{",
		"missing field": `{}`,
		"invalid ID":    `{"snapshot_id":"not-an-id"}`,
	}
	for name, data := range cases {
		t.Run(name, func(t *testing.T) {
			store := Store{Root: t.TempDir()}
			if _, err := store.Publish(Manifest{StateCommit: "current"}); err != nil {
				t.Fatal(err)
			}
			path := filepath.Join(store.Root, "consumer-state", "pin.json")
			if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(path, []byte(data), 0o644); err != nil {
				t.Fatal(err)
			}
			if _, err := store.PlanGC(GCOptions{}); err == nil {
				t.Fatal("malformed pin was accepted")
			}
		})
	}
}

func TestGCRejectsPinsToMissingSnapshots(t *testing.T) {
	store := Store{Root: t.TempDir()}
	if _, err := store.Publish(Manifest{StateCommit: "current"}); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(store.Root, "consumer-state", "pin.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	data := []byte(`{"snapshot_id":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}`)
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := store.PlanGC(GCOptions{}); err == nil {
		t.Fatal("missing pinned snapshot was accepted")
	}
}

func TestGarbageCollectPlansAndExecutesTransaction(t *testing.T) {
	store := Store{Root: t.TempDir()}
	oldObject := writeGCObject(t, store, "old.py")
	currentObject := writeGCObject(t, store, "current.py")
	oldID, err := store.Publish(Manifest{StateCommit: "old", Languages: []LanguageEntry{{
		Language: "python", Files: []FileEntry{{Path: "old.py", ObjectID: oldObject}},
	}}})
	if err != nil {
		t.Fatal(err)
	}
	currentID, err := store.Publish(Manifest{StateCommit: "current", Languages: []LanguageEntry{{
		Language: "python", Files: []FileEntry{{Path: "current.py", ObjectID: currentObject}},
	}}})
	if err != nil {
		t.Fatal(err)
	}

	dryRun, err := store.GarbageCollect(GCOptions{}, true)
	if err != nil {
		t.Fatal(err)
	}
	if !dryRun.DryRun || !reflect.DeepEqual(dryRun.DeletedSnapshots, []string{oldID}) {
		t.Fatalf("dry-run = %#v", dryRun)
	}
	if _, err := os.Stat(store.snapshotPath(oldID)); err != nil {
		t.Fatalf("dry-run removed old snapshot: %v", err)
	}

	result, err := store.GarbageCollect(GCOptions{}, false)
	if err != nil {
		t.Fatal(err)
	}
	if result.DryRun || !reflect.DeepEqual(result.DeletedSnapshots, []string{oldID}) {
		t.Fatalf("result = %#v", result)
	}
	if _, err := os.Stat(store.snapshotPath(oldID)); !os.IsNotExist(err) {
		t.Fatalf("old snapshot stat error = %v", err)
	}
	if _, err := os.Stat(store.objectPath(oldObject)); !os.IsNotExist(err) {
		t.Fatalf("old object stat error = %v", err)
	}
	resolvedID, _, err := store.Current()
	if err != nil || resolvedID != currentID {
		t.Fatalf("CURRENT = %s, err=%v", resolvedID, err)
	}
}

func TestExecuteGCRejectsChangedCurrent(t *testing.T) {
	store := Store{Root: t.TempDir()}
	oldID, err := store.Publish(Manifest{StateCommit: "old"})
	if err != nil {
		t.Fatal(err)
	}
	plannedCurrent, err := store.Publish(Manifest{StateCommit: "planned-current"})
	if err != nil {
		t.Fatal(err)
	}
	plan, err := store.PlanGC(GCOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if plan.CurrentSnapshot != plannedCurrent {
		t.Fatalf("planned current = %s", plan.CurrentSnapshot)
	}
	if _, err := store.Publish(Manifest{StateCommit: "new-current"}); err != nil {
		t.Fatal(err)
	}
	if _, err := store.ExecuteGC(plan, false); err == nil {
		t.Fatal("changed CURRENT was accepted")
	}
	if _, err := os.Stat(store.snapshotPath(oldID)); err != nil {
		t.Fatalf("changed-current execution deleted old snapshot: %v", err)
	}
}

func writeGCObject(t *testing.T, store Store, owner string) string {
	t.Helper()
	id, err := store.WriteObject(FactObject{
		Language:         "python",
		Owner:            owner,
		SourceContentID:  ContentID([]byte(owner)),
		AdapterVersion:   "test",
		SchemaVersion:    1,
		AnalysisConfigID: "sha256:config",
		Records:          []json.RawMessage{},
	})
	if err != nil {
		t.Fatal(err)
	}
	return id
}
