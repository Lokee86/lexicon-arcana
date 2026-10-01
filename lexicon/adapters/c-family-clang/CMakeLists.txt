cmake_minimum_required(VERSION 3.20)
project(lexicon_c_family_clang LANGUAGES C CXX)

find_package(LLVM REQUIRED CONFIG)
find_package(Clang REQUIRED CONFIG)

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/VERSION" HELPER_VERSION)
string(STRIP "${HELPER_VERSION}" HELPER_VERSION)

list(APPEND CMAKE_MODULE_PATH "${LLVM_CMAKE_DIR}")
include(AddLLVM)
add_definitions(${LLVM_DEFINITIONS})

add_executable(
    lexicon-c-family-clang
    main.cpp
    structural.cpp
    structural_action.cpp
    structural_access_flow.cpp
    structural_calls.cpp
    structural_commit.cpp
    structural_compilation.cpp
    structural_declaration_support.cpp
    structural_declarations.cpp
    structural_execution.cpp
    structural_frontend.cpp
    structural_traversal_scope.cpp
    structural_hot_path.cpp
    structural_model.cpp
    structural_memory.cpp
    structural_profile.cpp
    structural_heap.cpp
    structural_relationships.cpp
    structural_semantic_support.cpp
    structural_semantics.cpp
    structural_source.cpp
    structural_transport.cpp
    structural_value_flow.cpp
    structural_value_flow_json.cpp
)
target_compile_features(lexicon-c-family-clang PRIVATE cxx_std_20)
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
    clangAST
    clangBasic
    clangFrontend
    clangIndex
    clangLex
    clangTooling
    ${LEXICON_LLVM_LIBS}
)

if(WIN32)
    target_link_libraries(lexicon-c-family-clang PRIVATE Psapi)
endif()

enable_testing()
find_package(Threads REQUIRED)

add_executable(
    lexicon-c-family-clang-compilation-test
    structural_compilation_test.cpp
    structural_compilation.cpp
)
target_compile_features(lexicon-c-family-clang-compilation-test PRIVATE cxx_std_20)
target_include_directories(
    lexicon-c-family-clang-compilation-test
    SYSTEM PRIVATE
    ${LLVM_INCLUDE_DIRS}
    ${CLANG_INCLUDE_DIRS}
)
target_link_libraries(
    lexicon-c-family-clang-compilation-test
    PRIVATE
    clangBasic
    clangTooling
    ${LEXICON_LLVM_LIBS}
)
add_test(
    NAME lexicon-c-family-clang-compilation
    COMMAND lexicon-c-family-clang-compilation-test
)

add_executable(
    lexicon-c-family-clang-commit-test
    structural_commit_test.cpp
    structural_coverage_test.cpp
    structural_commit_buffer_test.cpp
    structural_commit.cpp
    structural_model.cpp
    structural_memory.cpp
    structural_semantics.cpp
    structural_value_flow_json.cpp
)
target_compile_features(lexicon-c-family-clang-commit-test PRIVATE cxx_std_20)
target_include_directories(
    lexicon-c-family-clang-commit-test
    SYSTEM PRIVATE
    ${LLVM_INCLUDE_DIRS}
    ${CLANG_INCLUDE_DIRS}
)
target_compile_definitions(
    lexicon-c-family-clang-commit-test
    PRIVATE
    LEXICON_CLANG_HELPER_VERSION="${HELPER_VERSION}"
)
target_link_libraries(
    lexicon-c-family-clang-commit-test
    PRIVATE
    Threads::Threads
    ${LEXICON_LLVM_LIBS}
)
add_test(
    NAME lexicon-c-family-clang-commit
    COMMAND lexicon-c-family-clang-commit-test
)
set_tests_properties(lexicon-c-family-clang-commit PROPERTIES TIMEOUT 30)

find_package(Python3 REQUIRED COMPONENTS Interpreter)
add_test(
    NAME lexicon-c-family-clang-heap
    COMMAND ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_heap_test.py
        $<TARGET_FILE:lexicon-c-family-clang> ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-heap PROPERTIES TIMEOUT 60)
add_test(
    NAME lexicon-c-family-clang-pointer-ownership
    COMMAND ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_pointer_ownership_test.py
        $<TARGET_FILE:lexicon-c-family-clang> ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-pointer-ownership PROPERTIES TIMEOUT 60)
add_test(
    NAME lexicon-c-family-clang-profile
    COMMAND
        ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_profile_test.py
        $<TARGET_FILE:lexicon-c-family-clang>
        ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-profile PROPERTIES TIMEOUT 60)
find_program(LEXICON_CLANG_EXECUTABLE NAMES clang clang-18
             HINTS "${LLVM_TOOLS_BINARY_DIR}")
if(LEXICON_CLANG_EXECUTABLE)
    add_test(
        NAME lexicon-c-family-clang-pch-context
        COMMAND
            ${Python3_EXECUTABLE}
            ${CMAKE_CURRENT_SOURCE_DIR}/../../../scripts/c_family_pch_probe.py
            --helper $<TARGET_FILE:lexicon-c-family-clang>
            --compiler ${LEXICON_CLANG_EXECUTABLE}
            --results-dir ${CMAKE_CURRENT_BINARY_DIR}/pch-regression
            --repetitions 1
    )
    set_tests_properties(lexicon-c-family-clang-pch-context PROPERTIES TIMEOUT 60)
endif()
add_test(
    NAME lexicon-c-family-clang-source-coverage
    COMMAND
        ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_source_coverage_test.py
        $<TARGET_FILE:lexicon-c-family-clang>
        ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-source-coverage PROPERTIES TIMEOUT 60)
add_test(
    NAME lexicon-c-family-clang-observation-work
    COMMAND
        ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_observation_work_test.py
        $<TARGET_FILE:lexicon-c-family-clang>
        ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-observation-work PROPERTIES TIMEOUT 60)
add_test(
    NAME lexicon-c-family-clang-execution-baseline
    COMMAND
        ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_execution_baseline_test.py
        $<TARGET_FILE:lexicon-c-family-clang>
        ${HELPER_VERSION}
)
set_tests_properties(lexicon-c-family-clang-execution-baseline PROPERTIES TIMEOUT 90)
add_test(
    NAME lexicon-c-family-clang-ownership
    COMMAND
        ${Python3_EXECUTABLE}
        ${CMAKE_CURRENT_SOURCE_DIR}/structural_ownership_test.py
        $<TARGET_FILE:lexicon-c-family-clang>
        ${HELPER_VERSION}
)
