#include "structural_model.h"

#include <algorithm>
#include <iterator>
#include <tuple>
#include <utility>

#include "protocol.h"
#include "structural_semantics.h"
#include "structural_value_flow_json.h"

namespace lexicon::clang_frontend {
namespace {

llvm::json::Array strings(const auto &values) {
  llvm::json::Array result;
  for (const auto &value : values) {
    result.emplace_back(value);
  }
  return result;
}

llvm::json::Object declaration_json(const Declaration &value) {
  llvm::json::Object result{
      {"compiler_id", value.compiler_id},
      {"kind", value.kind},
      {"name", value.name},
      {"qualified_name", value.qualified_name},
      {"signature", value.signature},
      {"type_name", value.type_name},
      {"tag", value.tag},
      {"container_compiler_id", value.container_compiler_id},
      {"parent_type_compiler_id", value.parent_type_compiler_id},
      {"span", span_json(value.span)},
      {"callable", value.callable},
      {"definition", value.definition},
      {"internal", value.internal},
      {"template", value.is_template},
      {"virtual_member", value.virtual_member},
      {"function_pointer", value.function_pointer},
      {"alias", value.alias},
      {"enum_member", value.enum_member},
      {"alias_target", value.alias_target},
  };
  if (value.parameter_index) {
    result["parameter_index"] = static_cast<std::int64_t>(*value.parameter_index);
  }
  if (value.parameter_count) {
    result["parameter_count"] = static_cast<std::int64_t>(*value.parameter_count);
  }
  return result;
}

llvm::json::Object include_json(const Include &value) {
  return llvm::json::Object{
      {"target", value.target},
      {"resolved_path", value.resolved_path},
      {"expression", value.expression},
      {"system", value.system},
      {"offset", static_cast<std::int64_t>(value.offset)},
      {"span", span_json(value.span)},
  };
}

llvm::json::Object macro_json(const Macro &value) {
  return llvm::json::Object{
      {"compiler_id", value.compiler_id},
      {"name", value.name},
      {"replacement", value.replacement},
      {"function_like", value.function_like},
      {"conditional", value.conditional},
      {"parameters", strings(value.parameters)},
      {"offset", static_cast<std::int64_t>(value.offset)},
      {"span", span_json(value.span)},
  };
}

llvm::json::Object file_json_impl(File value) {
  normalize_semantics(value);
  normalize_value_flow(value);
  std::sort(value.declarations.begin(), value.declarations.end(),
            [](const Declaration &left, const Declaration &right) {
              return std::tie(left.span.start_line, left.span.start_column,
                              left.kind, left.qualified_name, left.signature,
                              left.compiler_id) <
                     std::tie(right.span.start_line, right.span.start_column,
                              right.kind, right.qualified_name, right.signature,
                              right.compiler_id);
            });
  value.declarations.erase(
      std::unique(value.declarations.begin(), value.declarations.end(),
                  [](const Declaration &left, const Declaration &right) {
                    return left.compiler_id == right.compiler_id &&
                           left.kind == right.kind &&
                           left.qualified_name == right.qualified_name &&
                           left.signature == right.signature &&
                           left.span.start_line == right.span.start_line &&
                           left.span.start_column == right.span.start_column &&
                           left.span.end_line == right.span.end_line &&
                           left.span.end_column == right.span.end_column;
                  }),
      value.declarations.end());
  std::sort(value.includes.begin(), value.includes.end(),
            [](const Include &left, const Include &right) {
              return std::tie(left.offset, left.target, left.expression,
                              left.system) <
                     std::tie(right.offset, right.target, right.expression,
                              right.system);
            });
  value.includes.erase(
      std::unique(value.includes.begin(), value.includes.end(),
                  [](const Include &left, const Include &right) {
                    return left.offset == right.offset &&
                           left.target == right.target &&
                           left.expression == right.expression &&
                           left.system == right.system;
                  }),
      value.includes.end());
  std::sort(value.macros.begin(), value.macros.end(),
            [](const Macro &left, const Macro &right) {
              return std::tie(left.offset, left.name, left.replacement,
                              left.function_like, left.conditional) <
                     std::tie(right.offset, right.name, right.replacement,
                              right.function_like, right.conditional);
            });
  value.macros.erase(
      std::unique(value.macros.begin(), value.macros.end(),
                  [](const Macro &left, const Macro &right) {
                    return left.compiler_id == right.compiler_id &&
                           left.replacement == right.replacement &&
                           left.function_like == right.function_like &&
                           left.conditional == right.conditional &&
                           left.parameters == right.parameters;
                  }),
      value.macros.end());
  std::sort(value.diagnostics.begin(), value.diagnostics.end(),
            [](const Diagnostic &left, const Diagnostic &right) {
              return std::tie(left.path, left.message, left.severity) <
                     std::tie(right.path, right.message, right.severity);
            });
  value.diagnostics.erase(
      std::unique(value.diagnostics.begin(), value.diagnostics.end(),
                  [](const Diagnostic &left, const Diagnostic &right) {
                    return left.path == right.path &&
                           left.message == right.message &&
                           left.severity == right.severity;
                  }),
      value.diagnostics.end());

  llvm::json::Array declarations;
  for (const auto &declaration : value.declarations) {
    declarations.emplace_back(declaration_json(declaration));
  }
  llvm::json::Array includes;
  for (const auto &include : value.includes) {
    includes.emplace_back(include_json(include));
  }
  llvm::json::Array macros;
  for (const auto &macro : value.macros) {
    macros.emplace_back(macro_json(macro));
  }
  llvm::json::Array diagnostics;
  for (const auto &diagnostic : value.diagnostics) {
    diagnostics.emplace_back(diagnostic_json(diagnostic));
  }

  return llvm::json::Object{
      {"path", value.path},
      {"languages", strings(value.languages)},
      {"translation_units", strings(value.translation_units)},
      {"declarations", std::move(declarations)},
      {"includes", std::move(includes)},
      {"macros", std::move(macros)},
      {"relationships", relationships_json(value)},
      {"calls", calls_json(value)},
      {"pointer_bindings", pointer_bindings_json(value)},
      {"accesses", accesses_json(value)},
      {"diagnostics", std::move(diagnostics)},
  };
}

llvm::json::Object context_identity_json(const ContextIdentity &value) {
  return llvm::json::Object{
      {"compiler_id", value.compiler_id},
      {"path", value.path},
      {"kind", value.kind},
      {"qualified_name", value.qualified_name},
      {"signature", value.signature},
      {"definition", value.definition},
  };
}

llvm::json::Object translation_unit_json(const TranslationUnit &value) {
  return llvm::json::Object{
      {"path", value.path},
      {"language", value.language},
      {"directory", value.directory},
      {"arguments", strings(value.arguments)},
      {"synthesized", value.synthesized},
  };
}

} // namespace

llvm::json::Object file_json(File value) {
  return file_json_impl(std::move(value));
}

State::State(std::string repository_root)
    : repository_root(std::move(repository_root)) {}

File &State::file(const std::string &path, const std::string &language,
                  const std::string &translation_unit) {
  auto &[_, value] = *files.try_emplace(path, File{.path = path}).first;
  if (!language.empty()) {
    value.languages.insert(language);
  }
  if (!translation_unit.empty()) {
    value.translation_units.insert(translation_unit);
  }
  return value;
}

void State::set_owned_files(const std::vector<std::string> &paths) {
  active_owned_paths.clear();
  active_owned_paths.insert(paths.begin(), paths.end());
  all_owned_paths.insert(paths.begin(), paths.end());
}

bool State::owns(llvm::StringRef path) const {
  return active_owned_paths.contains(path.str());
}

void State::add_context_identity(ContextIdentity identity) {
  auto existing = std::find_if(
      context_identities.begin(), context_identities.end(),
      [&](const ContextIdentity &value) {
        return value.compiler_id == identity.compiler_id &&
               value.path == identity.path;
      });
  if (existing == context_identities.end()) {
    context_identities.push_back(std::move(identity));
    return;
  }
  if (!existing->definition && identity.definition) {
    *existing = std::move(identity);
  }
}

void State::add_diagnostic(Diagnostic diagnostic) {
  if (!diagnostic.path.empty() && owns(diagnostic.path)) {
    auto &[_, file] =
        *files.try_emplace(diagnostic.path, File{.path = diagnostic.path}).first;
    file.diagnostics.push_back(diagnostic);
  }
  diagnostics.push_back(std::move(diagnostic));
}

void State::merge(State other) {
  semantic_analysis_ns += other.semantic_analysis_ns;
  translation_units.insert(
      translation_units.end(),
      std::make_move_iterator(other.translation_units.begin()),
      std::make_move_iterator(other.translation_units.end()));
  diagnostics.insert(diagnostics.end(),
                     std::make_move_iterator(other.diagnostics.begin()),
                     std::make_move_iterator(other.diagnostics.end()));
  all_owned_paths.merge(other.all_owned_paths);
  for (auto &identity : other.context_identities) {
    add_context_identity(std::move(identity));
  }

  for (auto &[path, source] : other.files) {
    auto &[_, target] = *files.try_emplace(path, File{.path = path}).first;
    target.languages.merge(source.languages);
    target.translation_units.merge(source.translation_units);
    target.declaration_compiler_ids.merge(source.declaration_compiler_ids);

    auto append = [](auto &destination, auto &values) {
      destination.insert(destination.end(),
                         std::make_move_iterator(values.begin()),
                         std::make_move_iterator(values.end()));
    };
    append(target.declarations, source.declarations);
    append(target.includes, source.includes);
    append(target.macros, source.macros);
    append(target.relationships, source.relationships);
    append(target.calls, source.calls);
    append(target.pointer_bindings, source.pointer_bindings);
    append(target.accesses, source.accesses);
    append(target.diagnostics, source.diagnostics);
  }
}

llvm::json::Object State::metadata_response(bool compilation_database,
                                            std::string clang_version,
                                            llvm::StringRef helper_version) {
  std::sort(translation_units.begin(), translation_units.end(),
            [](const TranslationUnit &left, const TranslationUnit &right) {
              return std::tie(left.path, left.directory, left.arguments) <
                     std::tie(right.path, right.directory, right.arguments);
            });
  std::sort(diagnostics.begin(), diagnostics.end(),
            [](const Diagnostic &left, const Diagnostic &right) {
              return std::tie(left.path, left.message, left.severity) <
                     std::tie(right.path, right.message, right.severity);
            });

  std::sort(context_identities.begin(), context_identities.end(),
            [](const ContextIdentity &left, const ContextIdentity &right) {
              return std::tie(left.compiler_id, left.path, left.kind,
                              left.qualified_name, left.signature,
                              left.definition) <
                     std::tie(right.compiler_id, right.path, right.kind,
                              right.qualified_name, right.signature,
                              right.definition);
            });
  context_identities.erase(
      std::unique(context_identities.begin(), context_identities.end(),
                  [](const ContextIdentity &left, const ContextIdentity &right) {
                    return left.compiler_id == right.compiler_id &&
                           left.path == right.path &&
                           left.kind == right.kind &&
                           left.qualified_name == right.qualified_name &&
                           left.signature == right.signature &&
                           left.definition == right.definition;
                  }),
      context_identities.end());

  llvm::json::Array unit_values;
  for (const auto &value : translation_units) {
    unit_values.emplace_back(translation_unit_json(value));
  }
  llvm::json::Array context_identity_values;
  for (const auto &value : context_identities) {
    context_identity_values.emplace_back(context_identity_json(value));
  }
  llvm::json::Array diagnostic_values;
  for (const auto &value : diagnostics) {
    diagnostic_values.emplace_back(diagnostic_json(value));
  }

  return llvm::json::Object{
      {"protocol_version", kProtocolVersion},
      {"helper_version", helper_version},
      {"clang_version", std::move(clang_version)},
      {"compilation_database", compilation_database},
      {"translation_units", std::move(unit_values)},
      {"context_identities", std::move(context_identity_values)},
      {"diagnostics", std::move(diagnostic_values)},
  };
}

llvm::json::Object span_json(const Span &span) {
  return llvm::json::Object{
      {"path", span.path},
      {"start_line", static_cast<std::int64_t>(span.start_line)},
      {"start_column", static_cast<std::int64_t>(span.start_column)},
      {"end_line", static_cast<std::int64_t>(span.end_line)},
      {"end_column", static_cast<std::int64_t>(span.end_column)},
  };
}

llvm::json::Object diagnostic_json(const Diagnostic &diagnostic) {
  llvm::json::Object result{
      {"severity", diagnostic.severity},
      {"message", diagnostic.message},
      {"path", diagnostic.path},
  };
  if (diagnostic.span) {
    result["span"] = span_json(*diagnostic.span);
  }
  return result;
}

} // namespace lexicon::clang_frontend
