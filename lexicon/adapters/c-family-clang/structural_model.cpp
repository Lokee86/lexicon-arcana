#include "structural_model.h"

#include <algorithm>
#include <tuple>
#include <utility>

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
  return {
      {"target", value.target},
      {"resolved_path", value.resolved_path},
      {"expression", value.expression},
      {"system", value.system},
      {"offset", static_cast<std::int64_t>(value.offset)},
      {"span", span_json(value.span)},
  };
}

llvm::json::Object macro_json(const Macro &value) {
  return {
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

llvm::json::Object file_json(File value) {
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

  return {
      {"path", value.path},
      {"languages", strings(value.languages)},
      {"translation_units", strings(value.translation_units)},
      {"declarations", std::move(declarations)},
      {"includes", std::move(includes)},
      {"macros", std::move(macros)},
      {"diagnostics", std::move(diagnostics)},
  };
}

llvm::json::Object translation_unit_json(const TranslationUnit &value) {
  return {
      {"path", value.path},
      {"language", value.language},
      {"directory", value.directory},
      {"arguments", strings(value.arguments)},
      {"synthesized", value.synthesized},
  };
}

} // namespace

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

void State::add_diagnostic(Diagnostic diagnostic) {
  if (!diagnostic.path.empty()) {
    auto &[_, file] =
        *files.try_emplace(diagnostic.path, File{.path = diagnostic.path}).first;
    file.diagnostics.push_back(diagnostic);
  }
  diagnostics.push_back(std::move(diagnostic));
}

llvm::json::Object State::response(bool compilation_database,
                                   llvm::StringRef clang_version,
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

  llvm::json::Array file_values;
  for (auto &[_, value] : files) {
    file_values.emplace_back(file_json(std::move(value)));
  }
  llvm::json::Array unit_values;
  for (const auto &value : translation_units) {
    unit_values.emplace_back(translation_unit_json(value));
  }
  llvm::json::Array diagnostic_values;
  for (const auto &value : diagnostics) {
    diagnostic_values.emplace_back(diagnostic_json(value));
  }

  return {
      {"protocol_version", 1},
      {"helper_version", helper_version},
      {"clang_version", clang_version},
      {"compilation_database", compilation_database},
      {"translation_units", std::move(unit_values)},
      {"files", std::move(file_values)},
      {"diagnostics", std::move(diagnostic_values)},
  };
}

llvm::json::Object span_json(const Span &span) {
  return {
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
