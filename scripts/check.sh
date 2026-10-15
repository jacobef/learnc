#!/usr/bin/env bash
set -euo pipefail

CBOXES_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CBOXES_ROOT"

cargo fmt --manifest-path rust/c_interpreter/Cargo.toml --check
cargo test --manifest-path rust/c_interpreter/Cargo.toml --all-targets
tsc -p .
node scripts/test-progress.ts
node scripts/test-stdin-position.ts
node scripts/test-sandbox-navigation.ts
./scripts/build-c-interpreter-wasm.sh
tsc -p .
node scripts/test-c-interpreter-wasm.ts
node scripts/test-program-state.ts
node scripts/test-trace-lines.ts
node scripts/test-lessons.ts
node scripts/test-sandbox-execution.ts
node scripts/test-function-stepping.ts
git diff --check
