#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/rust/c_interpreter"
OUT="$ROOT/shared-c-interpreter-wasm-data.ts"
WASM="$CRATE/target/wasm32-wasip1/release/c_interpreter.wasm"
UNDEFINED_SYMBOLS="$CRATE/wasm-undefined-symbols.txt"
INTERPRETER_STACK_BYTES=33554432

rustup target add wasm32-wasip1 >/dev/null
RUSTFLAGS="-C link-arg=--allow-undefined-file=$UNDEFINED_SYMBOLS -C link-arg=-zstack-size=$INTERPRETER_STACK_BYTES${RUSTFLAGS:+ $RUSTFLAGS}" \
  cargo build --manifest-path "$CRATE/Cargo.toml" --release --lib --target wasm32-wasip1

{
  printf 'export const C_INTERPRETER_WASM_BASE64 = "'
  base64 -i "$WASM" | tr -d '\n'
  printf '";\n'
} > "$OUT"
