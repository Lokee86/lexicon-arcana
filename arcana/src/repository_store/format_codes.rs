use crate::repository::{
    NodeKind, RelationKind, UnresolvedReason, edge_kind_to_relation, relation_to_edge_kind,
};
use crate::synthetic::EdgeKind;

use super::UNKNOWN_REASON_CODE;

pub fn node_kind_code(kind: &NodeKind) -> u16 {
    match kind {
        NodeKind::Repository => 1,
        NodeKind::Directory => 2,
        NodeKind::File => 3,
        NodeKind::Module => 4,
        NodeKind::Namespace => 5,
        NodeKind::Symbol => 6,
        NodeKind::Type => 7,
        NodeKind::Interface => 8,
        NodeKind::Trait => 9,
        NodeKind::Function => 10,
        NodeKind::Method => 11,
        NodeKind::Constructor => 12,
        NodeKind::Field => 13,
        NodeKind::Variable => 14,
        NodeKind::Constant => 15,
        NodeKind::Signal => 16,
        NodeKind::Parameter => 17,
        NodeKind::Import => 18,
        NodeKind::Export => 19,
        NodeKind::Test => 20,
        NodeKind::HttpEndpoint => 21,
        NodeKind::MessageChannel => 22,
        NodeKind::ConfigKey => 23,
        NodeKind::Process => 24,
        NodeKind::CliCommand => 25,
        NodeKind::Protocol => 26,
        NodeKind::StatePath => 27,
    }
}

pub fn node_kind_from_code(code: u16) -> Option<NodeKind> {
    Some(match code {
        1 => NodeKind::Repository,
        2 => NodeKind::Directory,
        3 => NodeKind::File,
        4 => NodeKind::Module,
        5 => NodeKind::Namespace,
        6 => NodeKind::Symbol,
        7 => NodeKind::Type,
        8 => NodeKind::Interface,
        9 => NodeKind::Trait,
        10 => NodeKind::Function,
        11 => NodeKind::Method,
        12 => NodeKind::Constructor,
        13 => NodeKind::Field,
        14 => NodeKind::Variable,
        15 => NodeKind::Constant,
        16 => NodeKind::Signal,
        17 => NodeKind::Parameter,
        18 => NodeKind::Import,
        19 => NodeKind::Export,
        20 => NodeKind::Test,
        21 => NodeKind::HttpEndpoint,
        22 => NodeKind::MessageChannel,
        23 => NodeKind::ConfigKey,
        24 => NodeKind::Process,
        25 => NodeKind::CliCommand,
        26 => NodeKind::Protocol,
        27 => NodeKind::StatePath,
        _ => return None,
    })
}

pub fn relation_code(relation: &RelationKind) -> u16 {
    relation_to_edge_kind(relation).0
}

pub fn relation_from_code(code: u16) -> Option<RelationKind> {
    edge_kind_to_relation(EdgeKind(code))
}

pub fn unresolved_reason_code(reason: &UnresolvedReason) -> u16 {
    match reason {
        UnresolvedReason::MissingTarget => 1,
        UnresolvedReason::AmbiguousTarget => 2,
        UnresolvedReason::UnsupportedForm => 3,
        UnresolvedReason::DynamicTarget => 4,
        UnresolvedReason::ExternalTarget => 5,
        UnresolvedReason::BuiltinTarget => 6,
        UnresolvedReason::GeneratedTarget => 7,
        UnresolvedReason::TypeConversion => 8,
        UnresolvedReason::SelfTarget => 9,
        UnresolvedReason::UnsupportedMacroExpansion => 10,
        UnresolvedReason::MacroArgumentMismatch => 11,
        UnresolvedReason::MacroExpansionCycle => 12,
        UnresolvedReason::MacroExpansionDepth => 13,
        UnresolvedReason::CompilerAnalysisFailed => 14,
        UnresolvedReason::CompilerIdentityMismatch => 15,
        UnresolvedReason::Unknown(_) => UNKNOWN_REASON_CODE,
    }
}

pub fn unresolved_reason_from_code(code: u16) -> Option<UnresolvedReason> {
    Some(match code {
        1 => UnresolvedReason::MissingTarget,
        2 => UnresolvedReason::AmbiguousTarget,
        3 => UnresolvedReason::UnsupportedForm,
        4 => UnresolvedReason::DynamicTarget,
        5 => UnresolvedReason::ExternalTarget,
        6 => UnresolvedReason::BuiltinTarget,
        7 => UnresolvedReason::GeneratedTarget,
        8 => UnresolvedReason::TypeConversion,
        9 => UnresolvedReason::SelfTarget,
        10 => UnresolvedReason::UnsupportedMacroExpansion,
        11 => UnresolvedReason::MacroArgumentMismatch,
        12 => UnresolvedReason::MacroExpansionCycle,
        13 => UnresolvedReason::MacroExpansionDepth,
        14 => UnresolvedReason::CompilerAnalysisFailed,
        15 => UnresolvedReason::CompilerIdentityMismatch,
        UNKNOWN_REASON_CODE => return None,
        _ => return None,
    })
}
