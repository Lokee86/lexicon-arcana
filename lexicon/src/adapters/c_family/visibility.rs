use super::{
    discovery::is_header_path,
    includes::FileIndex,
    model::{Declaration, SourceFile},
};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Default)]
pub struct VisibilityIndex {
    include_distance: HashMap<String, HashMap<String, usize>>,
    translation_roots: HashMap<String, HashSet<String>>,
}

impl VisibilityIndex {
    pub fn new(files: &[SourceFile], index: &FileIndex<'_>) -> Self {
        let mut direct = HashMap::<String, Vec<String>>::with_capacity(files.len());
        for file in files {
            let mut targets = file
                .includes
                .iter()
                .filter_map(|include| index.local_target(include))
                .map(|target| target.path.clone())
                .collect::<Vec<_>>();
            targets.sort();
            targets.dedup();
            direct.insert(file.path.clone(), targets);
        }

        let mut value = Self::default();
        for file in files {
            value.include_distance.insert(
                file.path.clone(),
                reachable_include_distances(&file.path, &direct),
            );
        }

        for file in files {
            if is_header_path(&file.path) {
                continue;
            }
            let mut members = value
                .include_distance
                .get(&file.path)
                .cloned()
                .unwrap_or_default();
            members.insert(file.path.clone(), 0);
            for member in members.keys() {
                value
                    .translation_roots
                    .entry(member.clone())
                    .or_default()
                    .insert(file.path.clone());
            }
        }
        value
    }

    pub fn declaration_visible(&self, source_path: &str, declaration: &Declaration) -> bool {
        !declaration.file_local || self.file_local_visible(source_path, &declaration.path)
    }

    fn file_local_visible(&self, source_path: &str, declaration_path: &str) -> bool {
        if source_path == declaration_path {
            return true;
        }
        let Some(source_roots) = self.translation_roots.get(source_path) else {
            return false;
        };
        let Some(declaration_roots) = self.translation_roots.get(declaration_path) else {
            return false;
        };
        source_roots
            .iter()
            .any(|root| declaration_roots.contains(root))
    }

    pub fn include_rank(&self, source_path: &str, declaration_path: &str) -> Option<usize> {
        if source_path == declaration_path {
            return Some(0);
        }
        self.include_distance
            .get(source_path)?
            .get(declaration_path)
            .copied()
    }
}

fn reachable_include_distances(
    source: &str,
    direct: &HashMap<String, Vec<String>>,
) -> HashMap<String, usize> {
    let mut distances = HashMap::<String, usize>::new();
    let mut queue = VecDeque::from([(source.to_owned(), 0usize)]);
    let mut visited = HashSet::from([source.to_owned()]);

    while let Some((path, distance)) = queue.pop_front() {
        if let Some(targets) = direct.get(&path) {
            for target in targets {
                let target_distance = distance + 1;
                distances
                    .entry(target.clone())
                    .and_modify(|existing| *existing = (*existing).min(target_distance))
                    .or_insert(target_distance);
                if visited.insert(target.clone()) {
                    queue.push_back((target.clone(), target_distance));
                }
            }
        }
    }
    distances
}
