#include "structural_compilation.h"

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <tuple>
#include <utility>

namespace lexicon::clang_frontend {
namespace {

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

std::string relative_path(llvm::StringRef file) {
  return std::filesystem::path(file.str()).lexically_normal().generic_string();
}

std::string absolute_file(const std::string &root, llvm::StringRef relative) {
  return std::filesystem::absolute(
             std::filesystem::path(root) /
             std::filesystem::path(relative.str()))
      .lexically_normal()
      .string();
}

clang::tooling::CompileCommand synthetic_command(const std::string &root,
                                                 llvm::StringRef file) {
  clang::tooling::CompileCommand command;
  command.Directory = root;
  command.Filename = file.str();
  const bool cpp = !c_source(file);
  command.CommandLine = {"clang", "-fsyntax-only", cpp ? "-xc++" : "-xc",
                         cpp ? "-std=c++17" : "-std=c11", "-I", root};
  const auto conventional_include = std::filesystem::path(root) / "include";
  if (std::filesystem::is_directory(conventional_include)) {
    command.CommandLine.push_back("-I");
    command.CommandLine.push_back(conventional_include.string());
  }
  command.CommandLine.push_back(file.str());
  return command;
}

void sort_unique(std::vector<std::string> &paths) {
  for (auto &path : paths) {
    path = relative_path(path);
  }
  std::sort(paths.begin(), paths.end());
  paths.erase(std::unique(paths.begin(), paths.end()), paths.end());
}

} // namespace

CompilationCommands::CompilationCommands(
    std::string root, const clang::tooling::CompilationDatabase *base)
    : root_(std::filesystem::absolute(std::move(root))
                .lexically_normal()
                .string()) {
  if (!base) {
    return;
  }
  for (auto command : base->getAllCompileCommands()) {
    auto directory = std::filesystem::path(command.Directory);
    if (directory.is_relative()) {
      directory = std::filesystem::path(root_) / directory;
    }
    directory = std::filesystem::absolute(directory).lexically_normal();
    if (!directory.has_filename() && directory != directory.root_path()) {
      directory = directory.parent_path();
    }
    auto file = std::filesystem::path(command.Filename);
    if (file.is_relative()) {
      file = directory / file;
    }
    file = std::filesystem::absolute(file).lexically_normal();
    command.Directory = directory.string();
    command.Filename = file.string();
    if (!command.Output.empty()) {
      auto output = std::filesystem::path(command.Output);
      if (output.is_relative()) {
        output = directory / output;
      }
      command.Output =
          std::filesystem::absolute(output).lexically_normal().string();
    }
    exact_commands_[command.Filename].push_back(std::move(command));
  }
}

std::vector<clang::tooling::CompileCommand>
CompilationCommands::getCompileCommands(llvm::StringRef file) const {
  auto commands = real_commands(file);
  if (commands.empty()) {
    return {synthetic_command(root_, file)};
  }
  std::sort(commands.begin(), commands.end(),
            [](const auto &left, const auto &right) {
              return std::tie(left.Directory, left.Filename, left.CommandLine,
                              left.Output) <
                     std::tie(right.Directory, right.Filename,
                              right.CommandLine, right.Output);
            });
  commands.resize(1);
  return commands;
}

std::vector<std::string> CompilationCommands::getAllFiles() const {
  std::vector<std::string> files;
  files.reserve(exact_commands_.size());
  for (const auto &[path, _] : exact_commands_) {
    files.push_back(path);
  }
  return files;
}

bool CompilationCommands::synthesized(llvm::StringRef file) const {
  return !has_real_command(file);
}

bool CompilationCommands::has_real_command(llvm::StringRef file) const {
  return !real_commands(file).empty();
}

std::vector<clang::tooling::CompileCommand>
CompilationCommands::real_commands(llvm::StringRef file) const {
  auto path = std::filesystem::path(file.str());
  if (path.is_relative()) {
    path = std::filesystem::path(root_) / path;
  }
  const auto found = exact_commands_.find(path.lexically_normal().string());
  return found == exact_commands_.end()
             ? std::vector<clang::tooling::CompileCommand>{}
             : found->second;
}

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

ParsePlan build_parse_plan(const std::string &root,
                           CompilationCommands &database,
                           std::vector<std::string> owned_files,
                           std::vector<std::string> context_files) {
  sort_unique(owned_files);
  sort_unique(context_files);

  ParsePlan plan;
  std::vector<std::string> owned_headers;
  std::vector<std::string> source_units;
  for (const auto &path : owned_files) {
    if (header_source(path)) {
      owned_headers.push_back(path);
    } else {
      source_units.push_back(path);
    }
  }
  plan.orphan_candidates = owned_headers;

  // Context sources are compiler candidates only when an owned header needs
  // a real translation-unit context. Changed-source scans stay source-only.
  if (!owned_headers.empty()) {
    for (const auto &path : context_files) {
      if (!header_source(path)) {
        source_units.push_back(path);
      }
    }
  }
  sort_unique(source_units);

  std::vector<std::string> explicit_headers;
  for (const auto &path : owned_headers) {
    if (database.has_real_command(absolute_file(root, path))) {
      explicit_headers.push_back(path);
    }
  }

  auto append = [&](const std::vector<std::string> &paths) {
    for (const auto &path : paths) {
      const bool synthesized =
          database.synthesized(absolute_file(root, path));
      plan.primary_units.push_back(
          {plan.primary_units.size(), path, synthesized});
      if (synthesized) {
        ++plan.synthetic_units;
      } else {
        ++plan.real_units;
      }
    }
  };

  append(explicit_headers);
  plan.explicit_header_units = explicit_headers.size();

  std::vector<std::string> real_sources;
  std::vector<std::string> synthetic_sources;
  for (const auto &path : source_units) {
    if (database.has_real_command(absolute_file(root, path))) {
      real_sources.push_back(path);
    } else {
      synthetic_sources.push_back(path);
    }
  }
  append(real_sources);
  append(synthetic_sources);
  return plan;
}

} // namespace lexicon::clang_frontend
