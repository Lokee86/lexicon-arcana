#include "structural_declaration_support.h"

#include <string>
#include <utility>

#include "clang/AST/ASTContext.h"
#include "clang/AST/DeclCXX.h"
#include "clang/AST/TypeLoc.h"
#include "clang/Basic/SourceManager.h"
#include "clang/Index/USRGeneration.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/Support/SHA256.h"

#include "structural_source.h"
#include "structural_hot_path.h"
#include "perf.h"

namespace lexicon::clang_frontend {

std::string compiler_id(const clang::Decl *declaration,
                        const clang::SourceManager &sources) {
  const clang::Decl *identity = declaration;
  while (identity) {
    const clang::Decl *pattern = identity;
    if (const auto *method =
            llvm::dyn_cast<clang::CXXMethodDecl>(identity)) {
      if (const auto *instantiated =
              method->getInstantiatedFromMemberFunction()) {
        pattern = instantiated;
      }
    }
    if (const auto *function =
            llvm::dyn_cast<clang::FunctionDecl>(pattern)) {
      if (const auto *instantiated =
              function->getTemplateInstantiationPattern()) {
        pattern = instantiated;
      }
    }
    if (pattern == identity) {
      break;
    }
    identity = pattern;
  }

  if (auto cached = cached_compiler_id(identity)) {
    return *cached;
  }

  const auto started = hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  llvm::SmallString<128> usr;
  std::string result;
  if (!clang::index::generateUSRForDecl(identity, usr)) {
    result = usr.str().str();
  } else {
    auto location = sources.getSpellingLoc(identity->getLocation());
    result = "decl:" + sources.getFilename(location).str() + ":" +
             std::to_string(sources.getFileOffset(location)) + ":" +
             identity->getDeclKindName();
  }
  const auto elapsed_ns = hot_path_profiling()
                              ? static_cast<std::uint64_t>(
                                    std::chrono::duration_cast<std::chrono::nanoseconds>(
                                        PerfClock::now() - started)
                                        .count())
                              : 0;
  store_compiler_id(identity, result, elapsed_ns);
  return result;
}

void record_context_identity(State &state, clang::ASTContext &context,
                             const clang::NamedDecl &declaration,
                             llvm::StringRef repository_root) {
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, declaration.getLocation(), repository_root);
  if (!path || state.owns(*path)) {
    return;
  }
  auto observation = classify_declaration(
      *const_cast<clang::NamedDecl *>(&declaration), *path, context);
  if (!observation || observation->compiler_id.empty()) {
    return;
  }
  state.add_context_identity({
      .compiler_id = observation->compiler_id,
      .path = *path,
      .kind = observation->kind,
      .qualified_name = observation->qualified_name,
      .signature = observation->signature,
      .definition = observation->definition,
  });
}

std::string ensure_callable_declaration(
    State &state, clang::ASTContext &context,
    const clang::FunctionDecl &function, llvm::StringRef repository_root,
    llvm::StringRef translation_unit, llvm::StringRef language) {
  auto &sources = context.getSourceManager();
  auto id = compiler_id(&function, sources);
  if (id.empty()) {
    return {};
  }
  auto path = source_path(sources, function.getLocation(), repository_root);
  if (!path) {
    return {};
  }
  if (!state.owns(*path)) {
    record_context_identity(state, context, function, repository_root);
    return id;
  }
  auto &file = state.file(*path, language.str(), translation_unit.str());
  if (file.declaration_compiler_ids.contains(id)) {
    return id;
  }
  auto observation = classify_declaration(
      *const_cast<clang::FunctionDecl *>(&function), *path, context);
  if (!observation || !observation->callable) {
    return {};
  }
  file.declaration_compiler_ids.insert(observation->compiler_id);
  file.declarations.push_back(std::move(*observation));
  return id;
}

std::string context_id(const clang::DeclContext *context,
                       const clang::SourceManager &sources) {
  const auto started = hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  std::uint64_t steps = 0;
  std::string result;
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    ++steps;
    auto *declaration = clang::Decl::castFromDeclContext(current);
    result = compiler_id(declaration, sources);
    if (!result.empty()) {
      break;
    }
  }
  const auto elapsed_ns = hot_path_profiling()
                              ? static_cast<std::uint64_t>(
                                    std::chrono::duration_cast<std::chrono::nanoseconds>(
                                        PerfClock::now() - started)
                                        .count())
                              : 0;
  record_parent_chain(steps, elapsed_ns);
  return result;
}

std::string parent_type_id(const clang::DeclContext *context,
                           const clang::SourceManager &sources) {
  const auto started = hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  std::uint64_t steps = 0;
  std::string result;
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    ++steps;
    auto *declaration = clang::Decl::castFromDeclContext(current);
    if (llvm::isa<clang::RecordDecl>(declaration)) {
      result = compiler_id(declaration, sources);
      break;
    }
  }
  const auto elapsed_ns = hot_path_profiling()
                              ? static_cast<std::uint64_t>(
                                    std::chrono::duration_cast<std::chrono::nanoseconds>(
                                        PerfClock::now() - started)
                                        .count())
                              : 0;
  record_parent_chain(steps, elapsed_ns);
  return result;
}

bool function_pointer(clang::QualType type) {
  if (type.isNull()) {
    return false;
  }
  if (const auto *pointer = type->getAs<clang::PointerType>()) {
    return pointer->getPointeeType()->isFunctionType();
  }
  return false;
}

std::string printed_type(clang::QualType type, const clang::ASTContext &context) {
  if (type.isNull()) {
    return {};
  }
  auto policy = context.getPrintingPolicy();
  policy.SuppressScope = false;
  return type.getAsString(policy);
}

std::string function_signature(const clang::FunctionDecl &function,
                               const clang::ASTContext &context) {
  const auto &sources = context.getSourceManager();
  auto begin = function.getNameInfo().getBeginLoc();
  if (auto qualifier = function.getQualifierLoc()) {
    begin = qualifier.getBeginLoc();
  }
  clang::FunctionTypeLoc function_type;
  if (auto *type_source = function.getTypeSourceInfo()) {
    function_type = type_source->getTypeLoc().getAs<clang::FunctionTypeLoc>();
  }
  std::string signature;
  if (begin.isValid() && !function_type.isNull()) {
    auto parens = function_type.getParensRange();
    if (parens.getEnd().isValid()) {
      signature = normalize_space(source_text(
          sources, context.getLangOpts(),
          clang::SourceRange(begin, parens.getEnd())));
    }
  }
  if (signature.empty()) {
    signature = function.getNameAsString() + "()";
  }

  if (auto *method = llvm::dyn_cast<clang::CXXMethodDecl>(&function)) {
    if (method->isConst()) {
      signature += " const";
    }
    if (method->isVolatile()) {
      signature += " volatile";
    }
    switch (method->getRefQualifier()) {
    case clang::RQ_LValue:
      signature += " &";
      break;
    case clang::RQ_RValue:
      signature += " &&";
      break;
    case clang::RQ_None:
      break;
    }
  }

  auto exception_text = function_type.isNull()
                            ? std::string()
                            : normalize_space(source_text(
                                  sources, context.getLangOpts(),
                                  function_type.getExceptionSpecRange()));
  if (!exception_text.empty()) {
    signature += " ";
    signature += exception_text;
  }
  return signature;
}

std::string anonymous_name(llvm::StringRef tag, llvm::StringRef source) {
  llvm::SHA256 hasher;
  hasher.update(source);
  auto digest = hasher.final();
  constexpr char hex[] = "0123456789abcdef";
  std::string suffix;
  suffix.reserve(12);
  for (std::size_t index = 0; index < 6; ++index) {
    suffix.push_back(hex[digest[index] >> 4]);
    suffix.push_back(hex[digest[index] & 0x0f]);
  }
  return "(anonymous " + tag.str() + " " + suffix + ")";
}

std::string qualified_name(const clang::NamedDecl &declaration) {
  if (auto cached = cached_qualified_name(&declaration)) {
    return *cached;
  }
  auto result = declaration.getQualifiedNameAsString();
  store_qualified_name(&declaration, result);
  return result;
}

std::string context_qualified_name(const clang::DeclContext *context) {
  const auto started = hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  std::uint64_t steps = 0;
  std::string result;
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    ++steps;
    auto *named = llvm::dyn_cast<clang::NamedDecl>(
        clang::Decl::castFromDeclContext(current));
    if (!named) {
      continue;
    }
    result = qualified_name(*named);
    if (!result.empty()) {
      break;
    }
  }
  const auto elapsed_ns = hot_path_profiling()
                              ? static_cast<std::uint64_t>(
                                    std::chrono::duration_cast<std::chrono::nanoseconds>(
                                        PerfClock::now() - started)
                                        .count())
                              : 0;
  record_parent_chain(steps, elapsed_ns);
  return result;
}

bool internal_linkage(const clang::NamedDecl &declaration) {
  return declaration.getFormalLinkage() == clang::Linkage::Internal;
}

} // namespace lexicon::clang_frontend
