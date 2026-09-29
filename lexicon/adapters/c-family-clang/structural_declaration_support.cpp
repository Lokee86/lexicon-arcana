#include "structural_declaration_support.h"

#include <string>

#include "clang/AST/ASTContext.h"
#include "clang/AST/DeclCXX.h"
#include "clang/AST/TypeLoc.h"
#include "clang/Basic/SourceManager.h"
#include "clang/Index/USRGeneration.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/Support/SHA256.h"

#include "structural_source.h"

namespace lexicon::clang_frontend {

std::string compiler_id(const clang::Decl *declaration,
                        const clang::SourceManager &sources) {
  llvm::SmallString<128> usr;
  if (!clang::index::generateUSRForDecl(declaration, usr)) {
    return usr.str().str();
  }
  auto location = sources.getSpellingLoc(declaration->getLocation());
  return "decl:" + sources.getFilename(location).str() + ":" +
         std::to_string(sources.getFileOffset(location)) + ":" +
         declaration->getDeclKindName();
}

std::string context_id(const clang::DeclContext *context,
                       const clang::SourceManager &sources) {
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    auto *declaration = clang::Decl::castFromDeclContext(current);
    auto value = compiler_id(declaration, sources);
    if (!value.empty()) {
      return value;
    }
  }
  return {};
}

std::string parent_type_id(const clang::DeclContext *context,
                           const clang::SourceManager &sources) {
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    auto *declaration = clang::Decl::castFromDeclContext(current);
    if (llvm::isa<clang::RecordDecl>(declaration)) {
      return compiler_id(declaration, sources);
    }
  }
  return {};
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

std::string context_qualified_name(const clang::DeclContext *context) {
  for (auto *current = context; current && !current->isTranslationUnit();
       current = current->getParent()) {
    auto *named = llvm::dyn_cast<clang::NamedDecl>(
        clang::Decl::castFromDeclContext(current));
    if (!named) {
      continue;
    }
    auto qualified = named->getQualifiedNameAsString();
    if (!qualified.empty()) {
      return qualified;
    }
  }
  return {};
}

bool internal_linkage(const clang::NamedDecl &declaration) {
  return declaration.getFormalLinkage() == clang::Linkage::Internal;
}

} // namespace lexicon::clang_frontend
