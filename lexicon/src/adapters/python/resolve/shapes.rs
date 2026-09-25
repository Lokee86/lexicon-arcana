use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeShape {
    pub direct: BTreeSet<String>,
    pub elements: BTreeSet<String>,
    pub callables: BTreeSet<String>,
    pub element_callables: BTreeSet<String>,
    pub runtime_reasons: BTreeSet<String>,
    pub call_reasons: BTreeSet<String>,
    pub element_runtime_reasons: BTreeSet<String>,
    pub element_call_reasons: BTreeSet<String>,
}

impl TypeShape {
    pub fn merge(mut self, other: Self) -> Self {
        self.direct.extend(other.direct);
        self.elements.extend(other.elements);
        self.callables.extend(other.callables);
        self.element_callables.extend(other.element_callables);
        self.runtime_reasons.extend(other.runtime_reasons);
        self.call_reasons.extend(other.call_reasons);
        self.element_runtime_reasons
            .extend(other.element_runtime_reasons);
        self.element_call_reasons.extend(other.element_call_reasons);
        self
    }

    pub fn element_shape(&self) -> Self {
        Self {
            direct: self.elements.clone(),
            callables: self.element_callables.clone(),
            runtime_reasons: self.element_runtime_reasons.clone(),
            call_reasons: self.element_call_reasons.clone(),
            ..Self::default()
        }
    }

    pub fn direct(id: impl Into<String>) -> Self {
        Self {
            direct: BTreeSet::from([id.into()]),
            ..Self::default()
        }
    }

    pub fn callable(id: impl Into<String>) -> Self {
        Self {
            callables: BTreeSet::from([id.into()]),
            ..Self::default()
        }
    }

    pub fn runtime(reason: &str) -> Self {
        Self {
            runtime_reasons: BTreeSet::from([reason.to_owned()]),
            ..Self::default()
        }
    }

    pub fn dynamic_callable() -> Self {
        Self {
            call_reasons: BTreeSet::from(["dynamic-target".to_owned()]),
            ..Self::default()
        }
    }

    pub fn elements(shapes: impl IntoIterator<Item = Self>) -> Self {
        let mut result = Self::runtime("builtin-target");
        for shape in shapes {
            result.elements.extend(shape.direct);
            result.elements.extend(shape.elements);
            result.element_callables.extend(shape.callables);
            result.element_callables.extend(shape.element_callables);
            result.element_runtime_reasons.extend(shape.runtime_reasons);
            result
                .element_runtime_reasons
                .extend(shape.element_runtime_reasons);
            result.element_call_reasons.extend(shape.call_reasons);
            result
                .element_call_reasons
                .extend(shape.element_call_reasons);
        }
        result
    }

    pub fn reason(&self) -> Option<String> {
        reason_from(&self.call_reasons).or_else(|| reason_from(&self.runtime_reasons))
    }
}

pub fn reason_from(reasons: &BTreeSet<String>) -> Option<String> {
    match reasons.len() {
        0 => None,
        1 => reasons.first().cloned(),
        _ => Some("dynamic-target".into()),
    }
}

pub fn sequence_origin(name: &str) -> bool {
    matches!(
        name,
        "AsyncIterable"
            | "AsyncIterator"
            | "Collection"
            | "Generator"
            | "Iterable"
            | "Iterator"
            | "List"
            | "Sequence"
            | "Set"
            | "Tuple"
            | "list"
            | "set"
            | "tuple"
            | "frozenset"
    )
}

pub fn mapping_origin(name: &str) -> bool {
    matches!(name, "Dict" | "Mapping" | "MutableMapping" | "dict")
}

pub fn union_origin(name: &str) -> bool {
    matches!(name, "Annotated" | "Optional" | "Union")
}

pub fn wrapper_origin(name: &str) -> bool {
    matches!(
        name,
        "ClassVar" | "Final" | "Required" | "NotRequired" | "Type" | "type"
    )
}

pub fn semantic_decorator(name: &str) -> bool {
    matches!(
        name,
        "abstractmethod"
            | "cached_property"
            | "classmethod"
            | "dataclass"
            | "final"
            | "overload"
            | "override"
            | "property"
            | "staticmethod"
    )
}
