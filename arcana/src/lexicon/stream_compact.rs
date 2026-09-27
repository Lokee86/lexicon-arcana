use std::collections::{BTreeMap, HashMap};

use super::LexiconSnapshotError;
use super::binary_v2_stream::{EdgeRef, NodeRef, UnresolvedRef};
use super::identity::LexiconIdentity;
use super::object::NodeReference;
use super::stream_compact_convert::{compact_span, node_kind, relation_code, unresolved_reason};
use super::stream_compact_node::{NodeSignature, optional_intern, signature};
use crate::repository::{NodeKey, normalize_repository_path};
use crate::repository_store::{CompactRepositoryAssembler, CompactRepositoryBuild, Sha256Identity};

pub(super) type CompatibilityCounts = BTreeMap<String, usize>;

pub(super) struct CompactPass {
    nodes: BTreeMap<LexiconIdentity, NodeSignature>,
    keys: HashMap<NodeKey, LexiconIdentity>,
    external_ids: HashMap<LexiconIdentity, NodeKey>,
    assembler: CompactRepositoryAssembler,
    compatibility: CompatibilityCounts,
}

impl CompactPass {
    pub(super) fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            keys: HashMap::new(),
            external_ids: HashMap::new(),
            assembler: CompactRepositoryAssembler::with_capacity(0, 0, 0),
            compatibility: BTreeMap::new(),
        }
    }

    pub(super) fn ingest_node(&mut self, record: NodeRef<'_>) -> Result<(), LexiconSnapshotError> {
        let signature = signature(&mut self.assembler, &record)?;
        if let Some(existing) = self.nodes.get(&record.id) {
            if existing != &signature {
                return Err(LexiconSnapshotError::ConflictingNode(
                    record.id.canonical_string(),
                ));
            }
            return Ok(());
        }

        validate_owner(record.owner)?;
        let path = normalized(record.path)?;
        if record.qualified_name.is_empty() {
            return Err(LexiconSnapshotError::Malformed("node qualified name"));
        }
        let key = record.id.node_key();
        if self
            .keys
            .insert(key, record.id)
            .is_some_and(|existing| existing != record.id)
        {
            return Err(LexiconSnapshotError::Malformed("node identity collision"));
        }
        self.external_ids.insert(record.id, key);

        let kind_code = node_kind(record.kind, &mut self.compatibility);
        let path = self.assembler.intern(&path)?;
        let name = self.assembler.intern(record.name)?;
        let qualified_name = self.assembler.intern(record.qualified_name)?;
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler.push_node(
            key,
            Sha256Identity(record.id.digest()),
            record.content_id.map(LexiconIdentity::content_id),
            kind_code,
            path,
            name,
            qualified_name,
            span,
        );
        self.nodes.insert(record.id, signature);
        Ok(())
    }

    pub(super) fn finish_node_pass(&mut self) {
        self.nodes.clear();
        self.keys.clear();
        self.keys.shrink_to_fit();
    }

    pub(super) fn ingest_edge(&mut self, record: EdgeRef<'_>) -> Result<(), LexiconSnapshotError> {
        validate_owner(record.owner)?;
        let source = self.resolve(record.source)?;
        let target = self.resolve(record.target)?;
        let Some(relation_code) = relation_code(record.relation, "edge", &mut self.compatibility)
        else {
            return Ok(());
        };
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler
            .push_edge(source, target, relation_code, span);
        Ok(())
    }

    pub(super) fn ingest_unresolved(
        &mut self,
        record: UnresolvedRef<'_>,
    ) -> Result<(), LexiconSnapshotError> {
        validate_owner(record.owner)?;
        let source = self.resolve(record.source)?;
        let Some(relation_code) =
            relation_code(record.relation, "unresolved", &mut self.compatibility)
        else {
            return Ok(());
        };
        let (reason_code, unknown) = unresolved_reason(record.reason, &mut self.compatibility)?;
        let expression = self.assembler.intern(record.expression)?;
        let candidate_namespace = optional_intern(&mut self.assembler, record.candidate_namespace)?;
        let candidate_name = optional_intern(&mut self.assembler, record.candidate_name)?;
        let unknown_reason = unknown
            .then(|| self.assembler.intern(record.reason))
            .transpose()?;
        let span = compact_span(&mut self.assembler, record.span)?;
        self.assembler.push_unresolved(
            source,
            relation_code,
            reason_code,
            expression,
            candidate_namespace,
            candidate_name,
            unknown_reason,
            span,
        );
        Ok(())
    }

    pub(super) fn finish(
        self,
    ) -> Result<(CompactRepositoryBuild, Vec<String>), LexiconSnapshotError> {
        let Self {
            assembler,
            compatibility,
            external_ids,
            ..
        } = self;
        drop(external_ids);
        let warnings = compatibility
            .into_iter()
            .map(|(message, count)| format!("{message} ({count} record(s))"))
            .collect();
        Ok((assembler.finish()?, warnings))
    }

    fn resolve(&self, reference: NodeReference) -> Result<NodeKey, LexiconSnapshotError> {
        match reference {
            NodeReference::Key(key) => Ok(key),
            NodeReference::Identity(identity) => self
                .external_ids
                .get(&identity)
                .copied()
                .ok_or(LexiconSnapshotError::Malformed("unknown relationship node")),
        }
    }
}

fn validate_owner(owner: Option<&str>) -> Result<(), LexiconSnapshotError> {
    if let Some(owner) = owner {
        normalized(owner)?;
    }
    Ok(())
}

fn normalized(path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field: "fact",
        path: path.to_owned(),
    })
}
