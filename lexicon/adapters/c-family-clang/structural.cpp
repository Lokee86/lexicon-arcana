#include "structural.h"

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "clang/Basic/Version.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "clang/Tooling/Tooling.h"

#include "structural_frontend.h"
#include "structural_model.h"

namespace {

constexpr const char *kHelperVersion = LEXICON_CLANG_HELPER_VERSION;

std::string extension(llvm::StringRef file) {
  auto value = std::filesystem::path(file.str()).extension().string();
  if (value == ".C") {
    return "C";
  }
  std::transform(value.begin(), value.end(), value.begin(),
                 [](unsigned char ch) { return std::tolower(ch); });
  if (!value.empty() && value.front() == '.') {
    value.erase(value.begin());
  }
  return value;
}

bool c_source(llvm::StringRef file) { return extension(file) == "c"; }

bool header_source(llvm::StringRef file) {
  const auto value = extension(file);
  return value == "h" || value == "h++" || value == "hh" ||
         value == "hpp" || value == "hxx" || value == "inc" ||
         value == "inl" || value == "ipp" || value == "tpp";
}

std::string language_for(llvm::StringRef file,
                         const std::vector<std::string> &arguments) {
  std::string joined;
  for (const auto &argument : arguments) {
    if (!joined.empty()) {
      joined.push_back(' ');
    }
    joined += argument;
  }
  std::transform(joined.begin(), joined.end(), joined.begin(),
                 [](unsigned char value) { return std::tolower(value); });
  if (joined.find("-x c++") != std::string::npos ||
      joined.find("-xc++") != std::string::npos ||
      joined.find("clang++") != std::string::npos ||
      joined.find("g++") != std::string::npos) {
    return "cpp";
  }
  if (joined.find("-x c ") != std::string::npos ||
      joined.find("-xc ") != std::string::npos ||
      joined.ends_with("-x c") || joined.ends_with("-xc")) {
    return "c";
  }
  return c_source(file) ? "c" : "cpp";
}

clang::tooling::CompileCommand synthetic_command(const std::string &root,
                                                 llvm::StringRef file,
                                                 bool cpp) {
  clang::tooling::CompileCommand command;
  command.Directory = root;
  command.Filename = file.str();
  command.CommandLine = {
      "clang",
      "-fsyntax-only",
      cpp ? "-xc++" : "-xc",
      cpp ? "-std=c++17" : "-std=c11",
      file.str(),
  };
  return command;
}

bool canonical_relative(llvm::StringRef value) {
  if (value.empty() || value.contains('\\')) {
    return false;
  }
  std::filesystem::path path(value.str());
  if (path.is_absolute()) {
    return false;
  }
  for (const auto &part : path) {
    if (part == "..") {
      return false;
    }
  }
  return path.lexically_normal().generic_string() == value;
}

class CompilationCommands final : public clang::tooling::CompilationDatabase {
public:
  CompilationCommands(std::string root,
                      const clang::tooling::CompilationDatabase *base)
      : root_(std::move(root)), base_(base) {}

  std::vector<clang::tooling::CompileCommand>
  getCompileCommands(llvm::StringRef file) const override {
    if (base_) {
      auto commands = base_->getCompileCommands(file);
      if (!commands.empty()) {
        return commands;
      }
    }
    return {synthetic_command(root_, file, !c_source(file))};
  }

  std::vector<std::string> getAllFiles() const override {
    return base_ ? base_->getAllFiles() : std::vector<std::string>{};
  }

  bool synthesized(llvm::StringRef file) const {
    return !base_ || base_->getCompileCommands(file).empty();
  }

private:
  std::string root_;
  const clang::tooling::CompilationDatabase *base_;
};

bool validate_request(const llvm::json::Object &request, std::string &root,
                      std::vector<std::string> &files, std::string &error) {
  auto protocol = request.getInteger("protocol_version");
  auto operation = request.getString("operation");
  auto repository_root = request.getString("repository_root");
  const auto *input_files = request.getArray("files");
  if (!protocol || *protocol != 1) {
    error = "unsupported C-family Clang protocol version";
    return false;
  }
  if (!operation || *operation != "structural") {
    error = "unsupported C-family Clang operation";
    return false;
  }
  if (!repository_root || repository_root->empty()) {
    error = "repository_root is required";
    return false;
  }
  std::filesystem::path root_path(repository_root->str());
  if (!root_path.is_absolute()) {
    error = "repository_root must be absolute";
    return false;
  }
  if (!input_files) {
    error = "files is required";
    return false;
  }
  root = root_path.lexically_normal().string();
  for (const auto &entry : *input_files) {
    auto value = entry.getAsString();
    if (!value || !canonical_relative(*value)) {
      error = "files must use canonical repository-relative paths";
      return false;
    }
    files.push_back(value->str());
  }
  std::sort(files.begin(), files.end());
  files.erase(std::unique(files.begin(), files.end()), files.end());
  return true;
}

int run_batch(const std::string &root, CompilationCommands &database,
              lexicon::clang_frontend::State &state,
              const std::vector<std::string> &files) {
  if (files.empty()) {
    return 0;
  }

  std::vector<std::string> absolute_files;
  absolute_files.reserve(files.size());
  for (const auto &file : files) {
    const auto absolute =
        (std::filesystem::path(root) / std::filesystem::path(file)).string();
    absolute_files.push_back(absolute);
    for (const auto &command : database.getCompileCommands(absolute)) {
      const auto language = language_for(file, command.CommandLine);
      state.translation_units.push_back({
          .path = file,
          .language = language,
          .directory = command.Directory,
          .arguments = command.CommandLine,
          .synthesized = database.synthesized(absolute),
      });
      state.file(file, language, file);
    }
  }

  clang::tooling::ClangTool tool(database, absolute_files);
  auto factory = lexicon::clang_frontend::make_frontend_factory(state, root);
  return tool.run(factory.get());
}

} // namespace

bool emit_structural(const llvm::json::Object &request,
                     llvm::json::Object &response, std::string &error) {
  std::string root;
  std::vector<std::string> files;
  if (!validate_request(request, root, files, error)) {
    return false;
  }

  std::string database_error;
  auto base = clang::tooling::CompilationDatabase::autoDetectFromDirectory(
      root, database_error);
  CompilationCommands database(root, base.get());
  lexicon::clang_frontend::State state(root);

  std::vector<std::string> sources;
  std::vector<std::string> headers;
  for (const auto &file : files) {
    (header_source(file) ? headers : sources).push_back(file);
  }

  int status = run_batch(root, database, state, sources);
  std::vector<std::string> orphan_headers;
  for (const auto &header : headers) {
    if (!state.files.contains(header)) {
      orphan_headers.push_back(header);
    }
  }
  status |= run_batch(root, database, state, orphan_headers);
  if (status != 0) {
    state.add_diagnostic({
        .severity = "error",
        .message = "Clang tooling returned status " + std::to_string(status),
    });
  }

  response = state.response(base != nullptr, clang::getClangFullVersion(),
                            kHelperVersion);
  return true;
}
