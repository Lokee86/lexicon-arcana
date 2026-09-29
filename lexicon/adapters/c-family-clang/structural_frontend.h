#pragma once

#include <memory>
#include <string>

#include "clang/AST/ASTConsumer.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Tooling/Tooling.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

std::unique_ptr<clang::ASTConsumer>
make_ast_consumer(State &state, clang::CompilerInstance &compiler,
                  std::string repository_root, std::string translation_unit,
                  std::string language);

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_frontend_factory(State &state, std::string repository_root);

} // namespace lexicon::clang_frontend
