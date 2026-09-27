use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use lexicon::{SnapshotManifest, Store, content_id};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source_root = PathBuf::from(args.next().ok_or("missing source root")?);
    let target_root = PathBuf::from(args.next().ok_or("missing target root")?);
    let target_path = args.next().ok_or("missing target file path")?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    if target_root.exists() {
        fs::remove_dir_all(&target_root)?;
    }
    fs::create_dir_all(target_root.join("objects"))?;
    fs::create_dir_all(target_root.join("snapshots"))?;

    let source = Store::new(&source_root);
    let target = Store::new(&target_root);
    let (base_id, mut manifest) = source.current()?;
    link_snapshot(&source, &target, &base_id)?;

    let object_ids = referenced_objects(&manifest);
    for id in &object_ids {
        link_object(&source, &target, id)?;
    }

    let (language, file) = find_file_mut(&mut manifest, &target_path)?;
    let mut object = source.load_object(&file.object_id)?;
    let new_content_id = content_id(b"arcana-phase8-semantic-noop-delta");
    object.source_content_id = new_content_id.clone();
    let new_object_id = target.write_object(&object)?;
    file.content_id = new_content_id;
    file.object_id = new_object_id;

    let changed_path = file.path.clone();
    let language = language.to_owned();
    let new_id = target.publish(&manifest)?;
    println!("base_snapshot={base_id}");
    println!("new_snapshot={new_id}");
    println!("changed_language={language}");
    println!("changed_path={changed_path}");
    println!("linked_objects={}", object_ids.len());
    Ok(())
}

fn referenced_objects(manifest: &SnapshotManifest) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for language in manifest.languages.as_deref().unwrap_or_default() {
        if !language.shared_object_id.is_empty() {
            ids.insert(language.shared_object_id.clone());
        }
        for file in language.files.as_deref().unwrap_or_default() {
            ids.insert(file.object_id.clone());
        }
    }
    ids
}

fn find_file_mut<'a>(
    manifest: &'a mut SnapshotManifest,
    target_path: &str,
) -> Result<(&'a str, &'a mut lexicon::FileEntry), Box<dyn std::error::Error>> {
    for language in manifest.languages.as_mut().into_iter().flatten() {
        if language.language != "python" {
            continue;
        }
        if let Some(file) = language
            .files
            .as_mut()
            .into_iter()
            .flatten()
            .find(|file| file.path == target_path)
        {
            return Ok((language.language.as_str(), file));
        }
    }
    Err(format!("target Python file not found: {target_path}").into())
}

fn link_object(source: &Store, target: &Store, id: &str) -> std::io::Result<()> {
    let source_path = source.object_path(id);
    let target_path = target.object_path(id);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::hard_link(source_path, target_path)
}

fn link_snapshot(source: &Store, target: &Store, id: &str) -> std::io::Result<()> {
    let source_path = source.snapshot_path(id);
    let target_path = target.snapshot_path(id);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::hard_link(source_path, target_path)
}
