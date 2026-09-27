use std::collections::BTreeSet;

use crate::repository::{NodeKey, RepositoryFacts, normalize_repository_path};

use super::{ContributionKindView, RepositoryStore, RepositoryStoreReadError};

impl RepositoryStore {
    pub fn owned_node_keys(
        &self,
        paths: &[String],
    ) -> Result<Vec<NodeKey>, RepositoryStoreReadError> {
        let ownership = self.ownership()?;
        let mut node_ids = BTreeSet::new();
        for path in paths {
            let path = normalize_repository_path(path)?;
            for contribution in ownership.contributions(&path)? {
                if contribution.kind == ContributionKindView::Node {
                    node_ids.insert(to_u32(contribution.record_index)?);
                }
            }
        }
        node_ids
            .into_iter()
            .map(|node_id| self.node(node_id).map(|node| node.key()))
            .collect()
    }

    pub fn owned_facts(
        &self,
        paths: &[String],
    ) -> Result<RepositoryFacts, RepositoryStoreReadError> {
        let ownership = self.ownership()?;
        let mut nodes = BTreeSet::new();
        let mut edges = BTreeSet::new();
        let mut unresolved = BTreeSet::new();

        for path in paths {
            let path = normalize_repository_path(path)?;
            for contribution in ownership.contributions(&path)? {
                match contribution.kind {
                    ContributionKindView::Node => {
                        nodes.insert(to_u32(contribution.record_index)?);
                    }
                    ContributionKindView::Edge => {
                        edges.insert(contribution.record_index);
                    }
                    ContributionKindView::Unresolved => {
                        unresolved.insert(contribution.record_index);
                    }
                }
            }
        }

        let mut facts = RepositoryFacts::default();
        for node_id in nodes {
            let node = self.node(node_id)?;
            let fact = node.materialize()?;
            for _ in 0..node.occurrence_count() {
                facts.nodes.push(fact.clone());
            }
        }
        for index in edges {
            facts.edges.push(
                self.edge(index)?
                    .ok_or(RepositoryStoreReadError::InvalidOwnership)?
                    .materialize()?,
            );
        }
        for index in unresolved {
            facts.unresolved.push(
                self.unresolved(index)?
                    .ok_or(RepositoryStoreReadError::InvalidOwnership)?
                    .materialize()?,
            );
        }
        Ok(facts)
    }
}

fn to_u32(value: u64) -> Result<u32, RepositoryStoreReadError> {
    u32::try_from(value).map_err(|_| RepositoryStoreReadError::InvalidOwnership)
}
