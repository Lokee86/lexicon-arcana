#pragma once

#include <cstdint>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>

#include "llvm/Support/JSON.h"

namespace lexicon::clang_frontend {

struct Span {
  std::string path;
  std::uint64_t start_line = 0;
  std::uint64_t start_column = 0;
  std::uint64_t end_line = 0;
  std::uint64_t end_column = 0;
};

struct Declaration {
  std::string compiler_id;
  std::string kind;
  std::string name;
  std::string qualified_name;
  std::string signature;
  std::string type_name;
  std::string tag;
  std::string container_compiler_id;
  std::string parent_type_compiler_id;
  Span span;
  bool callable = false;
  bool definition = false;
  bool internal = false;
  bool is_template = false;
  bool virtual_member = false;
  bool function_pointer = false;
  bool alias = false;
  bool enum_member = false;
  std::string alias_target;
  std::optional<std::size_t> parameter_index;
  std::optional<std::size_t> parameter_count;
};

struct Include {
  std::string target;
  std::string resolved_path;
  std::string expression;
  bool system = false;
  std::uint64_t offset = 0;
  Span span;
};

struct Macro {
  std::string compiler_id;
  std::string name;
  std::string replacement;
  bool function_like = false;
  bool conditional = false;
  std::vector<std::string> parameters;
  std::uint64_t offset = 0;
  Span span;
};

struct Diagnostic {
  std::string severity;
  std::string message;
  std::string path;
  std::optional<Span> span;
};

struct SymbolReference {
  std::string compiler_id;
  std::string path;
  std::string qualified_name;
  std::string kind;
  bool external = false;
};

struct SemanticRelationship {
  std::string kind;
  std::string source_compiler_id;
  SymbolReference target;
  std::string expression;
  Span span;
};

struct SemanticArgument {
  std::string expression;
  std::optional<SymbolReference> value;
  std::optional<SymbolReference> callable;
};

struct SemanticPointerBinding {
  SymbolReference pointer;
  SymbolReference target;
  std::string expression;
  Span span;
};

struct SemanticAccess {
  std::string source_compiler_id;
  SymbolReference target;
  std::string relation;
  std::string expression;
  Span span;
};

struct SemanticCall {
  std::string source_compiler_id;
  std::string form;
  std::string resolution;
  std::string expression;
  std::optional<SymbolReference> target;
  std::vector<SymbolReference> candidates;
  std::optional<SymbolReference> receiver_type;
  std::optional<SymbolReference> callee_value;
  std::string receiver_type_name;
  bool virtual_dispatch = false;
  bool overload_selected = false;
  bool macro_expanded = false;
  std::size_t compiler_candidate_count = 0;
  std::vector<SemanticArgument> arguments;
  Span span;
};

struct File {
  std::string path;
  std::set<std::string> languages;
  std::set<std::string> translation_units;
  std::set<std::string> declaration_compiler_ids;
  std::vector<Declaration> declarations;
  std::vector<Include> includes;
  std::vector<Macro> macros;
  std::vector<SemanticRelationship> relationships;
  std::vector<SemanticCall> calls;
  std::vector<SemanticPointerBinding> pointer_bindings;
  std::vector<SemanticAccess> accesses;
  std::vector<Diagnostic> diagnostics;
};

struct TranslationUnit {
  std::string path;
  std::string language;
  std::string directory;
  std::vector<std::string> arguments;
  bool synthesized = false;
};

struct State {
  explicit State(std::string repository_root);

  File &file(const std::string &path, const std::string &language,
             const std::string &translation_unit);
  void add_diagnostic(Diagnostic diagnostic);
  void merge(State other);
  llvm::json::Object response(bool compilation_database,
                              std::string clang_version,
                              llvm::StringRef helper_version);

  std::string repository_root;
  std::map<std::string, File> files;
  std::vector<TranslationUnit> translation_units;
  std::vector<Diagnostic> diagnostics;
  std::uint64_t semantic_analysis_ns = 0;
};

llvm::json::Object span_json(const Span &span);
llvm::json::Object diagnostic_json(const Diagnostic &diagnostic);

} // namespace lexicon::clang_frontend
