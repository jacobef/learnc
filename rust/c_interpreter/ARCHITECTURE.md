# C interpreter architecture

The crate has four layers:

1. `preprocess.rs`, `lexer.rs`, and `parser.rs` turn source files into `ast.rs`.
   `preprocess/expansion.rs` retains token spelling, whitespace, source lines,
   and macro suppression sets through argument substitution and rescanning.
   Parser components under `parser/` separate declarations, declarators,
   expressions, statements, record layout, and constant evaluation. Binary
   operators use one precedence table; assignment, comma, and conditional
   expressions keep their distinct grammar rules.
2. `lib.rs` owns project loading and the parse/link/execute pipeline.
   `linker.rs` normalizes translation units and resolves C linkage;
   `linker/remap.rs` remaps tag IDs in place and `linker/composite.rs` constructs
   compatible composite types. `browser.rs` owns request decoding, implicit-main
   source adaptation, and the Wasm ABI. `browser_json.rs` serializes results.
3. `interpreter.rs` defines the runtime model and coordinates the evaluator
   components under `src/interpreter/`.
4. `native.rs` exposes the smaller native API used by developer tools. Native
   binaries live in `src/bin`.

`types.rs`, `integer.rs`, `number.rs`, `source.rs`, `token.rs`, and `diag.rs`
are shared domain modules. `fast_hash.rs` is restricted to trusted compiler and
interpreter keys; do not use it for attacker-controlled hash input.

`preprocess.rs::STANDARD_HEADERS` is the bundled-header inventory.
`standard_headers.rs` derives missing-header suggestions from those headers'
expanded declarations and macros, with a cache initialized on the first lookup.
The inventory parser disables suggestions to prevent recursive initialization;
tests require every bundled header to be inventoried successfully.

## Evaluator components

The evaluator is split by responsibility:

- `validation.rs` owns interpreter construction plus translation-unit and
  expression constraint checking.
- `initialization.rs` owns global/automatic initialization, object
  serialization, and call-boundary preparation.
- `execution.rs` evaluates expressions and lvalues.
- `control_flow.rs` handles blocks, loops, switch dispatch, goto, and longjmp
  resumption. `calls.rs` owns function calls, parameter binding, and variadic
  arguments.
- `loop_detection.rs` collects exit dependencies and compares exact bounded
  snapshots from `loop_state.rs`. It remembers recent states and an expanding
  cycle checkpoint, allowing long repeating periods without unbounded storage.
  `loop_progress.rs` and `loop_float_progress.rs` recognize unreachable counter
  exits while respecting integer conversions, unsigned wraparound, and floating
  rounding. Unknown calls, volatile data, arbitrary jumps, and memory that cannot
  be fully snapshotted retain the execution-budget fallback. The browser's default
  budget is 100,000 steps; reaching it alone does not establish an infinite loop.
  Regression cases, including a generated finite-loop matrix, live in
  `browser/tests/loop_detection.rs`.
- `visualization.rs` builds browser state snapshots, traces, and expression
  inspection results. Each trace event records the stdin byte position and
  stdout/stderr byte lengths. Output bytes are stored once in `ProgramOutput`,
  avoiding a growing output copy at every step. The browser decodes each selected
  prefix separately so partially emitted UTF-8 is represented correctly. Scope-close
  events capture retired bindings before the next statement, including `for` scopes.
  The sandbox requests a bounded trace prefix and caches it between steps, extending
  it in pages of 512 events. Extension replays from the start; it is not a suspended
  interpreter. A separate worker discovers later jump boundaries up to the same
  execution budget, returning locations rather than retaining all states on the page.
  Expression inspection stops immediately at the requested event.
  Loop events are tagged for the sandbox's paired first/nearest occurrence jump
  controls. Both controls use the same trace positions; no extra loop-entry,
  initialization, or increment events are generated for navigation.
- `type_compatibility.rs` owns conditional conversions and composite pointer
  and tagged types.
- `expression_types.rs` owns expression types, layout, and member lookup.
- `arithmetic.rs` owns scalar conversions, promotions, and arithmetic checks.
- `pointers.rs` owns pointer domains, offsets, and comparisons.
- `object_memory.rs` owns object representations, lvalues, and memory reads/writes.
  Representation bytes track partially known bits as well as fully known or
  indeterminate bytes, so a bit-field write preserves neighboring initialization
  state through byte copies and allocation resizing.
- `effective_type.rs` tracks effective types and aliasing through stores/copies.
- `lifetimes.rs` owns allocation, scope retirement, and setjmp/longjmp state.
- `sequencing.rs` tracks overlapping accesses, sequence points, and `restrict`.
- `strings.rs` owns string literal storage and bounded character reads.
- `formatted_io.rs` validates format strings and implements the shared scanning
  machinery.
- `library_dispatch.rs` routes modeled library calls and owns their common
  precondition and stream helpers.
- `math.rs` implements complex and real math plus shared library-region checks.
- `calendar.rs` normalizes calendar dates for the browser's UTC environment,
  without calling WASI's trapping local-time conversion stub.
- `stdio.rs` implements streams and formatted I/O, together with the closely
  related byte/string validation paths.
- `runtime_library.rs` implements the remaining standard-library families,
  including wide/multibyte conversion, locale, time, signals, allocation, and
  string/memory operations.
- `tests.rs` provides shared test helpers. The regression suite under `tests/`
  is grouped by language and library subsystem; `tests/standard_examples.rs`
  reuses the C fixtures also executed by the browser-Wasm smoke test.

The files share the private runtime model from `interpreter.rs`. Methods are
module-private unless a sibling evaluator component needs them, in which case
they use `pub(super)` rather than becoming crate API.

New code should follow these ownership rules:

- Browser and native entry-point policy belongs in `browser.rs` or `native.rs`.
- The execution pipeline passes `ProgramOutput` directly; do not add a duplicate
  result model just to copy the same fields between layers.
- `Diagnostic` keeps its payload behind one box so routine evaluator `Result`
  values do not carry source-display and runtime-context storage on the stack.
- Tool-specific source adaptation and drivers belong under that tool's
  `src/bin` directory.
- Syntax-only transformations belong in the parser or linker, before runtime.
- Repeated library operations should share one checker and one evaluator, with
  the function name selecting only the standard-specific differences.
- Interpreted scalar values belong to the explicit cBoxes data model.
  `libc::c_*` types belong only at real host-ABI boundaries; convert
  deliberately when a host width differs from the interpreter's fixed LP64
  model.
- Object-representation writes must go through the shared overlay helpers so
  initialization counts, pointer slots, and modification versions stay in sync.

## Validation

After Rust changes, run `cargo test`. After anything that can affect the browser
build, also run `scripts/build-c-interpreter-wasm.sh` from the repository root
and `tsc -p .`.
Run `node scripts/test-c-interpreter-wasm.ts` to exercise the shared standard
example fixtures through the generated browser build. Native tests alone cannot
detect host ABI differences or trapping WASI library stubs.
