#!/bin/sh
set -eu

case_name="$1"
source_root="$2"
run_mode="${3:-full}"
work_root="/work/corpus"
repo_name="$(basename "$source_root")"
case_root="$work_root/$repo_name"

git config --global --add safe.directory '*'
mkdir -p "$work_root"
rm -rf "$case_root"
cp -a "$source_root" "$case_root"

case "$case_name" in
  leveldb)
    rm -rf /tmp/leveldb-final-build
    cmake -S "$case_root" -B /tmp/leveldb-final-build \
      -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
      -DLEVELDB_BUILD_TESTS=OFF -DLEVELDB_BUILD_BENCHMARKS=OFF >/dev/null
    cp /tmp/leveldb-final-build/compile_commands.json "$case_root/compile_commands.json"
    ;;
  fmt)
    rm -rf /tmp/fmt-final-build
    cmake -S "$case_root" -B /tmp/fmt-final-build \
      -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DFMT_TEST=OFF -DFMT_DOC=OFF >/dev/null
    cp /tmp/fmt-final-build/compile_commands.json "$case_root/compile_commands.json"
    ;;
  catch2)
    rm -rf /tmp/catch2-final-build
    cmake -S "$case_root" -B /tmp/catch2-final-build \
      -DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DCATCH_BUILD_TESTING=OFF >/dev/null
    cp /tmp/catch2-final-build/compile_commands.json "$case_root/src/compile_commands.json"
    ;;
  codebase-memory)
    python3 /repo/scripts/c_family_make_compdb.py \
      --repository "$case_root" --makefile Makefile.cbm --target cbm \
      --make-arg=CC=clang --make-arg=CXX=clang++ \
      --output "$case_root/compile_commands.json"
    ;;
  git)
    rm -rf /tmp/git-final-build
    cmake -S "$case_root/contrib/buildsystems" -B /tmp/git-final-build \
      -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
      -DBUILD_TESTING=OFF >/dev/null
    cp /tmp/git-final-build/compile_commands.json "$case_root/compile_commands.json"
    ;;
  nlohmann-json)
    ;;
  *)
    echo "unsupported C-family Phase 2.7 case: $case_name" >&2
    exit 2
    ;;
esac

case "$run_mode" in
  full)
    run_args=""
    ;;
  cold)
    run_args="--cold-only"
    ;;
  warm)
    run_args="--warm-only"
    ;;
  *)
    echo "unsupported Phase 2.7 run mode: $run_mode" >&2
    exit 2
    ;;
esac

python3 /repo/scripts/c_family_phase2_baseline.py \
  --corpus-root "$work_root" \
  --output /out/final-matrix.json \
  --executable /out/adapter_eval \
  --timeout-seconds 600 \
  --append \
  --case "$case_name" \
  $run_args
