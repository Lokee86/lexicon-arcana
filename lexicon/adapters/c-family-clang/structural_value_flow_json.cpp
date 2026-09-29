#include "structural_value_flow_json.h"

#include <algorithm>
#include <tuple>

namespace lexicon::clang_frontend {
namespace {

llvm::json::Object symbol_json(const SymbolReference &value) {
  return {
      {"compiler_id", value.compiler_id},
      {"path", value.path},
      {"qualified_name", value.qualified_name},
      {"kind", value.kind},
      {"external", value.external},
  };
}

} // namespace

void normalize_value_flow(File &file) {
  std::sort(file.pointer_bindings.begin(), file.pointer_bindings.end(),
            [](const SemanticPointerBinding &left,
               const SemanticPointerBinding &right) {
              return std::tie(left.span.start_line, left.span.start_column,
                              left.pointer.compiler_id,
                              left.target.compiler_id) <
                     std::tie(right.span.start_line, right.span.start_column,
                              right.pointer.compiler_id,
                              right.target.compiler_id);
            });
  file.pointer_bindings.erase(
      std::unique(file.pointer_bindings.begin(), file.pointer_bindings.end(),
                  [](const SemanticPointerBinding &left,
                     const SemanticPointerBinding &right) {
                    return left.pointer.compiler_id ==
                               right.pointer.compiler_id &&
                           left.target.compiler_id ==
                               right.target.compiler_id &&
                           left.span.start_line == right.span.start_line &&
                           left.span.start_column == right.span.start_column;
                  }),
      file.pointer_bindings.end());

  std::sort(file.accesses.begin(), file.accesses.end(),
            [](const SemanticAccess &left, const SemanticAccess &right) {
              return std::tie(left.span.start_line, left.span.start_column,
                              left.relation, left.target.compiler_id) <
                     std::tie(right.span.start_line, right.span.start_column,
                              right.relation, right.target.compiler_id);
            });
  file.accesses.erase(
      std::unique(file.accesses.begin(), file.accesses.end(),
                  [](const SemanticAccess &left,
                     const SemanticAccess &right) {
                    return left.source_compiler_id ==
                               right.source_compiler_id &&
                           left.target.compiler_id ==
                               right.target.compiler_id &&
                           left.relation == right.relation &&
                           left.span.start_line == right.span.start_line &&
                           left.span.start_column == right.span.start_column;
                  }),
      file.accesses.end());
}

llvm::json::Array pointer_bindings_json(const File &file) {
  llvm::json::Array result;
  for (const auto &value : file.pointer_bindings) {
    result.emplace_back(llvm::json::Object{
        {"pointer", symbol_json(value.pointer)},
        {"target", symbol_json(value.target)},
        {"expression", value.expression},
        {"span", span_json(value.span)},
    });
  }
  return result;
}

llvm::json::Array accesses_json(const File &file) {
  llvm::json::Array result;
  for (const auto &value : file.accesses) {
    result.emplace_back(llvm::json::Object{
        {"source_compiler_id", value.source_compiler_id},
        {"target", symbol_json(value.target)},
        {"relation", value.relation},
        {"expression", value.expression},
        {"span", span_json(value.span)},
    });
  }
  return result;
}

} // namespace lexicon::clang_frontend
