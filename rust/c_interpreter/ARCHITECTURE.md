# C interpreter architecture

The crate has four layers:

1. `preprocess.rs`, `lexer.rs`, and `parser.rs` turn source files into `ast.rs`.
2. `lib.rs` owns virtual-project loading, translation-unit normalization, linking,
   and the browser ABI.
3. `interpreter.rs` defines the runtime model and coordinates the evaluator
   components under `src/interpreter/`.
4. `native.rs` exposes the smaller native API used by developer tools. Native
   binaries live in `src/bin`.

`types.rs`, `integer.rs`, `number.rs`, `source.rs`, `token.rs`, and `diag.rs`
are shared domain modules. `fast_hash.rs` is restricted to trusted compiler and
interpreter keys; do not use it for attacker-controlled hash input.

## Evaluator components

The evaluator is split by responsibility:

- `validation.rs` owns interpreter construction plus translation-unit and
  expression constraint checking.
- `initialization.rs` owns global/automatic initialization, object
  serialization, and call-boundary preparation.
- `execution.rs` handles control flow, expression evaluation, and function
  calls.
- `value_semantics.rs` handles objects, pointers, effective types, conversions,
  arithmetic, sequencing, and `restrict` tracking.
- `formatted_io.rs` validates format strings and implements the shared scanning
  machinery.
- `library_dispatch.rs` routes modeled library calls and owns their common
  precondition and stream helpers.
- `math.rs` implements complex and real math plus shared library-region checks.
- `stdio.rs` implements streams and formatted I/O, together with the closely
  related byte/string validation paths.
- `runtime_library.rs` implements the remaining standard-library families,
  including wide/multibyte conversion, locale, time, signals, allocation, and
  string/memory operations.
- `tests.rs` contains the evaluator's unit and conformance regression suite.

The files share the private runtime model from `interpreter.rs`. Methods are
module-private unless a sibling evaluator component needs them, in which case
they use `pub(super)` rather than becoming crate API.

New code should follow these ownership rules:

- Browser and native entry-point policy belongs in `lib.rs` or `native.rs`.
- Tool-specific source adaptation and drivers belong under that tool's
  `src/bin` directory.
- Syntax-only transformations belong in the parser or linker, before runtime.
- Repeated library operations should share one checker and one evaluator, with
  the function name selecting only the standard-specific differences.
- Object-representation writes must go through the shared overlay helpers so
  initialization counts, pointer slots, and modification versions stay in sync.

## Validation

After Rust changes, run `cargo test`. After anything that can affect the browser
build, also run `scripts/build-c-interpreter-wasm.sh` from the repository root
and `tsc -p .`.
