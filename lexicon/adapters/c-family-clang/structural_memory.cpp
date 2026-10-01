#include "structural_memory.h"

#include <initializer_list>
#include <optional>

namespace lexicon::clang_frontend {
namespace {
using Bytes = std::size_t;

Bytes extra(const std::string &v) { return v.capacity() + 1; }
Bytes strings(std::initializer_list<const std::string *> values) {
  Bytes total = 0;
  for (auto value : values) total += extra(*value);
  return total;
}
template <typename T> Bytes extra(const std::vector<T> &values);
template <typename T> Bytes extra(const std::optional<T> &value);
Bytes extra(const Span &v) { return extra(v.path); }
Bytes extra(const Diagnostic &v) {
  return strings({&v.severity, &v.message, &v.path}) + extra(v.span);
}
Bytes extra(const SymbolReference &v) {
  return strings({&v.compiler_id, &v.path, &v.qualified_name, &v.kind});
}
Bytes extra(const Declaration &v) {
  return strings({&v.compiler_id, &v.kind, &v.name, &v.qualified_name,
                  &v.signature, &v.type_name, &v.tag, &v.container_compiler_id,
                  &v.parent_type_compiler_id, &v.alias_target}) + extra(v.span);
}
Bytes extra(const Include &v) {
  return strings({&v.target, &v.resolved_path, &v.expression}) + extra(v.span);
}
Bytes extra(const Macro &v) {
  return strings({&v.compiler_id, &v.name, &v.replacement}) +
         extra(v.parameters) + extra(v.span);
}
Bytes extra(const SemanticRelationship &v) {
  return strings({&v.kind, &v.source_compiler_id, &v.expression}) +
         extra(v.target) + extra(v.span);
}
Bytes extra(const SemanticArgument &v) {
  return extra(v.expression) + extra(v.value) + extra(v.callable);
}
Bytes extra(const SemanticPointerBinding &v) {
  return extra(v.pointer) + extra(v.target) + extra(v.expression) + extra(v.span);
}
Bytes extra(const SemanticAccess &v) {
  return strings({&v.source_compiler_id, &v.relation, &v.expression}) +
         extra(v.target) + extra(v.span);
}
Bytes extra(const SemanticCall &v) {
  return strings({&v.source_compiler_id, &v.form, &v.resolution, &v.expression,
                  &v.receiver_type_name}) + extra(v.target) + extra(v.candidates) +
         extra(v.receiver_type) + extra(v.callee_value) + extra(v.arguments) +
         extra(v.span);
}
Bytes extra(const ContextIdentity &v) {
  return strings({&v.compiler_id, &v.path, &v.kind, &v.qualified_name, &v.signature});
}
Bytes extra(const TranslationUnit &v) {
  return strings({&v.path, &v.language, &v.directory}) + extra(v.arguments);
}
template <typename T> Bytes extra(const std::optional<T> &value) {
  return value ? extra(*value) : 0;
}
template <typename T> Bytes extra(const std::vector<T> &values) {
  Bytes total = values.capacity() * sizeof(T);
  for (const auto &value : values) total += extra(value);
  return total;
}
Bytes extra(const std::set<std::string> &values) {
  Bytes total = values.size() * (sizeof(std::string) + 4 * sizeof(void *));
  for (const auto &value : values) total += extra(value);
  return total;
}
Bytes extra(const File &v) {
  return extra(v.path) + extra(v.languages) + extra(v.translation_units) +
         extra(v.declaration_compiler_ids) + extra(v.declarations) +
         extra(v.includes) + extra(v.macros) + extra(v.relationships) +
         extra(v.calls) + extra(v.pointer_bindings) + extra(v.accesses) +
         extra(v.diagnostics);
}
} // namespace

std::size_t estimated_retained_bytes(const State &state) {
  Bytes total = sizeof(State) + extra(state.repository_root) +
                extra(state.active_owned_paths) + extra(state.all_owned_paths) +
                extra(state.suppressed_observation_paths) +
                extra(state.context_identities) + extra(state.translation_units) +
                extra(state.diagnostics);
  for (const auto &[path, file] : state.files) {
    total += sizeof(std::pair<const std::string, File>) + 4 * sizeof(void *) +
             extra(path) + extra(file);
  }
  return total;
}
} // namespace lexicon::clang_frontend
