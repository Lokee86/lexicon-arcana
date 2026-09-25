use rustpython_parser::ast;

use super::super::model::ImportInfo;
use super::super::source::text;
use super::Visitor;

impl Visitor<'_> {
    pub(super) fn visit_import(&mut self, value: &ast::StmtImport) {
        for alias in &value.names {
            let target_module = alias.name.to_string();
            let binding = alias
                .asname
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    target_module
                        .split('.')
                        .next()
                        .unwrap_or(&target_module)
                        .to_owned()
                });
            self.add_import(value, target_module, Some(binding), None, 0, false);
        }
    }

    pub(super) fn visit_import_from(&mut self, value: &ast::StmtImportFrom) {
        let target_module = value
            .module
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let level = value.level.as_ref().map_or(0, |value| value.to_u32());
        for alias in &value.names {
            let target_name = alias.name.to_string();
            let star = target_name == "*";
            let binding = (!star).then(|| {
                alias
                    .asname
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| target_name.clone())
            });
            self.add_import(
                value,
                target_module.clone(),
                binding,
                (!star).then_some(target_name),
                level,
                star,
            );
        }
    }

    fn add_import(
        &mut self,
        statement: &impl ast::Ranged,
        target_module: String,
        binding: Option<String>,
        target_name: Option<String>,
        relative_level: u32,
        star: bool,
    ) {
        self.import_index += 1;
        let record_span = self.node_span(statement);
        let mut expression = text(statement, &self.file.source).unwrap_or_default();
        if let Some(target) = target_name.as_deref() {
            expression = format!("{expression} [{target}]");
        }
        let line = record_span.as_ref().map_or(0, |span| span.start_line);
        let label = binding
            .as_deref()
            .or(target_name.as_deref())
            .unwrap_or(&target_module);
        let qname = format!(
            "{}::import:{line}:{}:{label}",
            self.file.module, self.import_index
        );
        let id = self.facts.add_node(
            "import",
            label,
            &self.file.relative,
            &qname,
            Some(&qname),
            record_span.clone(),
            Some(serde_json::json!({"expression": expression})),
            None,
        );
        let owner = self.owner().to_owned();
        self.facts
            .add_edge(&owner, &id, "defines", record_span.clone(), None);
        self.facts.imports.push(ImportInfo {
            module_name: self.file.module.clone(),
            owner_id: owner.clone(),
            expression,
            binding: binding.clone(),
            target_module,
            target_name,
            relative_level,
            star,
            is_package: self.file.relative == "__init__.py"
                || self.file.relative.ends_with("/__init__.py"),
            span: record_span,
        });

        if let Some(binding) = binding {
            let result = (None, "unresolved".to_owned());
            self.facts
                .scope_bindings
                .insert((owner.clone(), binding.clone()), result.clone());
            if self
                .facts
                .modules
                .get(&self.file.module)
                .is_some_and(|module| module == &owner)
            {
                self.facts
                    .module_bindings
                    .insert((self.file.module.clone(), binding), result);
            }
        }
    }
}
