#include <filesystem>
#include <iostream>
#include <optional>
#include <string>
#include <vector>

#include "clang/Basic/Version.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/raw_ostream.h"

#include "structural.h"

namespace {

constexpr int kProtocolVersion = 1;
constexpr const char *kHelperVersion = LEXICON_CLANG_HELPER_VERSION;

struct Options {
  int protocol_version = 0;
  std::string helper_version;
  bool show_version = false;
};

std::optional<Options> parse_options(int argc, char **argv) {
  Options options;
  for (int index = 1; index < argc; ++index) {
    std::string argument = argv[index];
    if (argument == "--version") {
      options.show_version = true;
    } else if (argument == "--protocol-version" && index + 1 < argc) {
      options.protocol_version = std::stoi(argv[++index]);
    } else if (argument == "--helper-version" && index + 1 < argc) {
      options.helper_version = argv[++index];
    } else {
      return std::nullopt;
    }
  }
  return options;
}

int fail(const std::string &message, int code = 1) {
  llvm::errs() << message << "\n";
  return code;
}

int emit_capabilities(const llvm::json::Object &request) {
  auto protocol = request.getInteger("protocol_version");
  auto operation = request.getString("operation");
  auto repository_root = request.getString("repository_root");
  if (!protocol || *protocol != kProtocolVersion) {
    return fail("unsupported C-family Clang protocol version");
  }
  if (!operation || *operation != "capabilities") {
    return fail("unsupported C-family Clang operation");
  }
  if (!repository_root || repository_root->empty()) {
    return fail("repository_root is required");
  }

  std::filesystem::path root(repository_root->str());
  if (!root.is_absolute()) {
    return fail("repository_root must be absolute");
  }

  std::string database_error;
  auto database =
      clang::tooling::CompilationDatabase::autoDetectFromDirectory(
          root.string(), database_error);

  llvm::json::Array capabilities;
  capabilities.emplace_back("ast");
  capabilities.emplace_back("compile-database");
  capabilities.emplace_back("preprocessor");
  capabilities.emplace_back("source-manager");

  llvm::json::Object response{
      {"protocol_version", kProtocolVersion},
      {"helper_version", kHelperVersion},
      {"clang_version", clang::getClangFullVersion()},
      {"capabilities", std::move(capabilities)},
      {"compilation_database", database != nullptr},
  };
  if (!database && !database_error.empty()) {
    response["compilation_database_error"] = database_error;
  }

  llvm::outs() << llvm::formatv("{0}\n", llvm::json::Value(std::move(response)));
  return 0;
}

} // namespace

int main(int argc, char **argv) {
  auto options = parse_options(argc, argv);
  if (!options) {
    return fail("invalid C-family Clang helper arguments", 2);
  }
  if (options->show_version) {
    llvm::outs() << "lexicon-c-family-clang " << kHelperVersion << "\n";
    return 0;
  }
  if (options->protocol_version != kProtocolVersion) {
    return fail("unsupported C-family Clang protocol version", 2);
  }
  if (options->helper_version != kHelperVersion) {
    return fail("C-family Clang helper version mismatch", 2);
  }

  std::string payload;
  if (!std::getline(std::cin, payload) || payload.empty()) {
    return fail("read C-family Clang request");
  }
  auto parsed = llvm::json::parse(payload);
  if (!parsed) {
    return fail("decode C-family Clang request");
  }
  auto *object = parsed->getAsObject();
  if (!object) {
    return fail("C-family Clang request must be an object");
  }
  auto operation = object->getString("operation");
  if (!operation) {
    return fail("C-family Clang operation is required");
  }
  if (*operation == "capabilities") {
    return emit_capabilities(*object);
  }
  if (*operation == "structural") {
    llvm::json::Object response;
    std::string error;
    if (!emit_structural(*object, response, error)) {
      return fail(error);
    }
    llvm::outs() << llvm::formatv("{0}\n",
                                  llvm::json::Value(std::move(response)));
    return 0;
  }
  return fail("unsupported C-family Clang operation");
}
