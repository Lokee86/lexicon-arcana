use std::thread;

use crate::{LanguageEntry, ScanExecutionError, Store};

use super::{Library, Node};

const MAX_NODE_LOAD_WORKERS: usize = 16;

pub(crate) fn library_from_entry(
    store: &Store,
    entry: &LanguageEntry,
) -> Result<Library, ScanExecutionError> {
    let mut objects = entry
        .files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|file| file.object_id.as_str())
        .collect::<Vec<_>>();
    if !entry.shared_object_id.is_empty() {
        objects.push(&entry.shared_object_id);
    }
    let nodes = load_nodes(store, &objects)?;
    Ok(Library {
        language: entry.language.clone(),
        repository: entry.repository.clone(),
        nodes,
    })
}

fn load_nodes(store: &Store, objects: &[&str]) -> Result<Vec<Node>, ScanExecutionError> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let workers = objects
        .len()
        .min(
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        )
        .min(MAX_NODE_LOAD_WORKERS)
        .max(1);
    if workers == 1 {
        return objects.iter().try_fold(Vec::new(), |mut nodes, object_id| {
            nodes.extend(load_object_nodes(store, object_id)?);
            Ok(nodes)
        });
    }

    let mut buckets = (0..workers).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, object_id) in objects.iter().copied().enumerate() {
        buckets[index % workers].push((index, object_id));
    }

    thread::scope(|scope| {
        let handles = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(index, object_id)| Ok((index, load_object_nodes(store, object_id)?)))
                        .collect::<Result<Vec<_>, ScanExecutionError>>()
                })
            })
            .collect::<Vec<_>>();

        let mut ordered = (0..objects.len())
            .map(|_| None)
            .collect::<Vec<Option<Vec<Node>>>>();
        for handle in handles {
            let completed = handle
                .join()
                .map_err(|_| ScanExecutionError::new("Interstack node loader panicked"))??;
            for (index, nodes) in completed {
                ordered[index] = Some(nodes);
            }
        }

        let mut nodes = Vec::new();
        for item in ordered {
            nodes.extend(
                item.ok_or_else(|| ScanExecutionError::new("Interstack node load missing result"))?,
            );
        }
        Ok(nodes)
    })
}

fn load_object_nodes(store: &Store, object_id: &str) -> Result<Vec<Node>, ScanExecutionError> {
    let (_, loaded) = store.load_node_facts(object_id)?;
    Ok(loaded.into_iter().map(Node::from).collect())
}
