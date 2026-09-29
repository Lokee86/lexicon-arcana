#include "structural_declaration_support.h"

#include <utility>

#include "clang/AST/ASTContext.h"
#include "clang/AST/DeclCXX.h"
#include "clang/Basic/SourceManager.h"

#include "structural_source.h"

namespace lexicon::clang_frontend {

std::optional<Declaration>
classify_declaration(clang::NamedDecl &named, const std::string &path,
                     clang::ASTContext &context) {
  auto &sources = context.getSourceManager();
  Declaration value;
  value.compiler_id = compiler_id(&named, sources);
  value.name = named.getNameAsString();
  value.qualified_name = named.getQualifiedNameAsString();
  value.container_compiler_id =
      context_id(named.getLexicalDeclContext(), sources);
  value.parent_type_compiler_id =
      parent_type_id(named.getLexicalDeclContext(), sources);
  value.span =
      source_span(sources, context.getLangOpts(), named.getSourceRange(), path);
  value.internal = internal_linkage(named);

  if (auto *parameter = llvm::dyn_cast<clang::ParmVarDecl>(&named)) {
    value.kind = "parameter";
    value.type_name = printed_type(parameter->getType(), context);
    value.function_pointer = function_pointer(parameter->getType());
    if (auto *function =
            llvm::dyn_cast<clang::FunctionDecl>(parameter->getDeclContext())) {
      value.container_compiler_id = compiler_id(function, sources);
      value.parent_type_compiler_id =
          parent_type_id(function->getLexicalDeclContext(), sources);
      value.qualified_name =
          function->getQualifiedNameAsString() + "::" + value.name;
      for (unsigned index = 0; index < function->getNumParams(); ++index) {
        if (function->getParamDecl(index) == parameter) {
          value.parameter_index = index;
          value.signature = "parameter#" + std::to_string(index);
          break;
        }
      }
    }
    value.definition = true;
  } else if (auto *function = llvm::dyn_cast<clang::FunctionDecl>(&named)) {
    const auto *lexical = function->getLexicalDeclContext();
    const bool lexically_in_type =
        lexical && !lexical->isTranslationUnit() &&
        llvm::isa<clang::RecordDecl>(clang::Decl::castFromDeclContext(lexical));
    value.kind =
        lexically_in_type && llvm::isa<clang::CXXConstructorDecl>(function)
            ? "constructor"
            : lexically_in_type && llvm::isa<clang::CXXMethodDecl>(function)
                  ? "method"
                  : "function";
    value.callable = true;
    value.definition = function->isThisDeclarationADefinition();
    value.signature = function_signature(*function, context);
    value.parameter_count = function->getNumParams();
    value.is_template = function->getDescribedFunctionTemplate() != nullptr;
    if (auto *method = llvm::dyn_cast<clang::CXXMethodDecl>(function)) {
      value.virtual_member = method->isVirtual();
    }
  } else if (auto *field = llvm::dyn_cast<clang::FieldDecl>(&named)) {
    value.kind = "field";
    value.type_name = printed_type(field->getType(), context);
    value.function_pointer = function_pointer(field->getType());
    value.definition = true;
  } else if (auto *variable = llvm::dyn_cast<clang::VarDecl>(&named)) {
    const auto *lexical = variable->getLexicalDeclContext();
    const bool lexically_in_type =
        lexical && !lexical->isTranslationUnit() &&
        llvm::isa<clang::RecordDecl>(clang::Decl::castFromDeclContext(lexical));
    value.kind = lexically_in_type
                     ? "field"
                     : variable->getType().isConstQualified() ||
                               variable->isConstexpr()
                           ? "constant"
                           : "variable";
    value.type_name = printed_type(variable->getType(), context);
    value.function_pointer = function_pointer(variable->getType());
    value.definition = true;
    if (variable->isLocalVarDecl()) {
      auto location = sources.getSpellingLoc(variable->getLocation());
      value.signature =
          "local@" + std::to_string(sources.getFileOffset(location));
      if (auto *function =
              llvm::dyn_cast<clang::FunctionDecl>(variable->getDeclContext())) {
        value.qualified_name =
            function->getQualifiedNameAsString() + "::" + value.name;
      }
    }
  } else if (llvm::isa<clang::EnumConstantDecl>(&named)) {
    value.kind = "constant";
    value.enum_member = true;
    value.definition = true;
  } else if (auto *type = llvm::dyn_cast<clang::TypedefNameDecl>(&named)) {
    value.kind = "type";
    value.alias = true;
    value.alias_target = printed_type(type->getUnderlyingType(), context);
    value.function_pointer = function_pointer(type->getUnderlyingType());
    value.definition = true;
  } else if (auto *record = llvm::dyn_cast<clang::RecordDecl>(&named)) {
    value.kind = "type";
    value.definition = record->isThisDeclarationADefinition();
    value.is_template =
        llvm::isa<clang::CXXRecordDecl>(record) &&
        llvm::cast<clang::CXXRecordDecl>(record)->getDescribedClassTemplate();
    value.tag = record->isClass()   ? "class"
                : record->isUnion() ? "union"
                                    : "struct";
  } else if (auto *enumeration = llvm::dyn_cast<clang::EnumDecl>(&named)) {
    value.kind = "type";
    value.tag = "enum";
    value.definition = enumeration->isThisDeclarationADefinition();
  } else if (llvm::isa<clang::NamespaceDecl>(&named)) {
    value.kind = "namespace";
    value.definition = true;
  } else {
    return std::nullopt;
  }

  if (value.name.empty()) {
    auto declaration_text = normalize_space(
        source_text(sources, context.getLangOpts(), named.getSourceRange()));
    value.name = anonymous_name(
        value.tag.empty() ? llvm::StringRef(value.kind)
                          : llvm::StringRef(value.tag),
        declaration_text);
  }
  if (value.qualified_name.empty()) {
    auto container = context_qualified_name(named.getLexicalDeclContext());
    value.qualified_name =
        container.empty() ? value.name : container + "::" + value.name;
  }
  return value;
}

} // namespace lexicon::clang_frontend
