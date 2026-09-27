use crate::repository::RepositoryFacts;

use super::{RepositoryStore, RepositoryStoreReadError};

impl RepositoryStore {
    pub fn materialize_facts(&self) -> Result<RepositoryFacts, RepositoryStoreReadError> {
        let mut facts = RepositoryFacts::default();

        for node_id in 0..self.node_count() {
            let node = self.node(node_id)?;
            let fact = node.materialize()?;
            facts.nodes.reserve(node.occurrence_count() as usize);
            for _ in 0..node.occurrence_count() {
                facts.nodes.push(fact.clone());
            }
        }

        facts.edges.reserve(self.edge_count() as usize);
        for index in 0..self.edge_count() {
            let edge = self
                .edge(index)?
                .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
            facts.edges.push(edge.materialize()?);
        }

        facts.unresolved.reserve(self.unresolved_count() as usize);
        for index in 0..self.unresolved_count() {
            let reference = self
                .unresolved(index)?
                .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
            facts.unresolved.push(reference.materialize()?);
        }

        Ok(facts)
    }
}
