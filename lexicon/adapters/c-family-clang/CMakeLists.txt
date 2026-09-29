cmake_minimum_required(VERSION 3.20)
project(lexicon_c_family_clang LANGUAGES CXX)

find_package(LLVM REQUIRED CONFIG)
find_package(Clang REQUIRED CONFIG)

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/VERSION" HELPER_VERSION)
string(STRIP "${HELPER_VERSION}" HELPER_VERSION)

list(APPEND CMAKE_MODULE_PATH "${LLVM_CMAKE_DIR}")
include(AddLLVM)
add_definitions(${LLVM_DEFINITIONS})

add_executable(lexicon-c-family-clang main.cpp)
target_compile_features(lexicon-c-family-clang PRIVATE cxx_std_17)
target_include_directories(
    lexicon-c-family-clang
    SYSTEM PRIVATE
    ${LLVM_INCLUDE_DIRS}
    ${CLANG_INCLUDE_DIRS}
)
target_compile_definitions(
    lexicon-c-family-clang
    PRIVATE
    LEXICON_CLANG_HELPER_VERSION="${HELPER_VERSION}"
)
llvm_map_components_to_libnames(LEXICON_LLVM_LIBS Support)
target_link_libraries(
    lexicon-c-family-clang
    PRIVATE
    clangBasic
    clangTooling
    ${LEXICON_LLVM_LIBS}
)
