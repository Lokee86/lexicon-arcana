#include "structural_semantics.h"

#include <algorithm>
#include <string>
#include <tuple>

namespace lexicon::clang_frontend {
namespace {

llvm::json::Object symbol_json(const SymbolReference &value) {
  return llvm::json::Object{
      {"compiler_id", value.compiler_id},
      {"path", value.path},
      {"qualified_name", value.qualified_name},
      {"kind", value.kind},
      {"external", value.external},
  };
}

llvm::json::Array symbols_json(const std::vector<SymbolReference> &values) {
  llvm::json::Array result;
  for (const auto &value : values) {
    result.emplace_back(symbol_json(value));
  }
  return result;
}

llvm::json::Object argument_json(const SemanticArgument &value) {
  llvm::json::Object result{{"expression", value.expression}};
  if (value.value) {
    result["value"] = symbol_json(*value.value);
  }
  if (value.callable) {
    result["callable"] = symbol_json(*value.callable);
  }
  return result;
}

llvm::json::Array arguments_json(const std::vector<SemanticArgument> &values) {
  llvm::json::Array result;
  for (const auto &value : values) {
    result.emplace_back(argument_json(value));
  }
  return result;
}

llvm::json::Object relationship_json(const SemanticRelationship &value) {
  return llvm::json::Object{
      {"kind", value.kind},
      {"source_compiler_id", value.source_compiler_id},
      {"target", symbol_json(value.target)},
      {"expression", value.expression},
      {"span", span_json(value.span)},
  };
}

llvm::json::Object call_json(const SemanticCall &value) {
  llvm::json::Object result{
      {"source_compiler_id", value.source_compiler_id},
      {"form", value.form},
      {"resolution", value.resolution},
      {"expression", value.expression},
      {"candidates", symbols_json(value.candidates)},
      {"receiver_type_name", value.receiver_type_name},
      {"virtual_dispatch", value.virtual_dispatch},
      {"overload_selected", value.overload_selected},
      {"macro_expanded", value.macro_expanded},
      {"compiler_candidate_count",
       static_cast<std::int64_t>(value.compiler_candidate_count)},
      {"arguments", arguments_json(value.arguments)},
      {"span", span_json(value.span)},
  };
  if (value.target) {
    result["target"] = symbol_json(*value.target);
  }
  if (value.receiver_type) {
    result["receiver_type"] = symbol_json(*value.receiver_type);
  }
  if (value.callee_value) {
    result["callee_value"] = symbol_json(*value.callee_value);
  }
  return result;
}

std::string target_key(const SemanticCall &value) {
  return value.target ? value.target->compiler_id : std::string();
}

} // namespace

void normalize_semantics(File &file) {
  std::sort(file.relationships.begin(), file.relationships.end(),
            [](const SemanticRelationship &left,
               const SemanticRelationship &right) {
              return std::tie(left.span.start_line, left.span.start_column,
                              left.kind, left.source_compiler_id,
                              left.target.compiler_id, left.target.path) <
                     std::tie(right.span.start_line, right.span.start_column,
                              right.kind, right.source_compiler_id,
                              right.target.compiler_id, right.target.path);
            });
  file.relationships.erase(
      std::unique(file.relationships.begin(), file.relationships.end(),
                  [](const SemanticRelationship &left,
                     const SemanticRelationship &right) {
                    return left.kind == right.kind &&
                           left.source_compiler_id == right.source_compiler_id &&
                           left.target.compiler_id == right.target.compiler_id &&
                           left.target.path == right.target.path &&
                           left.span.start_line == right.span.start_line &&
                           left.span.start_column == right.span.start_column;
                  }),
      file.relationships.end());

  for (auto &call : file.calls) {
    std::sort(call.candidates.begin(), call.candidates.end(),
              [](const SymbolReference &left, const SymbolReference &right) {
                return std::tie(left.compiler_id, left.path,
                                left.qualified_name) <
                       std::tie(right.compiler_id, right.path,
                                right.qualified_name);
              });
    call.candidates.erase(
        std::unique(call.candidates.begin(), call.candidates.end(),
                    [](const SymbolReference &left,
                       const SymbolReference &right) {
                      return left.compiler_id == right.compiler_id &&
                             left.path == right.path;
                    }),
        call.candidates.end());
  }

  std::sort(file.calls.begin(), file.calls.end(),
            [](const SemanticCall &left, const SemanticCall &right) {
              return std::tuple(left.span.start_line, left.span.start_column,
                                left.form, left.expression, target_key(left)) <
                     std::tuple(right.span.start_line, right.span.start_column,
                                right.form, right.expression, target_key(right));
            });
  file.calls.erase(
      std::unique(file.calls.begin(), file.calls.end(),
                  [](const SemanticCall &left, const SemanticCall &right) {
                    return left.source_compiler_id == right.source_compiler_id &&
                           left.form == right.form &&
                           left.expression == right.expression &&
                           target_key(left) == target_key(right) &&
                           left.span.start_line == right.span.start_line &&
                           left.span.start_column == right.span.start_column &&
                           left.span.end_line == right.span.end_line &&
                           left.span.end_column == right.span.end_column;
                  }),
      file.calls.end());
}

llvm::json::Array relationships_json(const File &file) {
  llvm::json::Array result;
  for (const auto &value : file.relationships) {
    result.emplace_back(relationship_json(value));
  }
  return result;
}

llvm::json::Array calls_json(const File &file) {
  llvm::json::Array result;
  for (const auto &value : file.calls) {
    result.emplace_back(call_json(value));
  }
  return result;
}

} // namespace lexicon::clang_frontend
