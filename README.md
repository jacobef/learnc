# C Boxes

An interactive C tutorial and sandbox, backed by a Rust C interpreter compiled
for the browser. Pages are static HTML; TypeScript produces the checked-in
`js/` modules. The interpreter and generated Wasm payload are kept in this
repository so the site can run without a compilation service.

## Develop

Use a Rust toolchain, TypeScript (`tsc`), and Node.js with TypeScript script
support. To compile the frontend and serve the repository locally:

```sh
tsc -p .
python3 -m http.server 8000 --bind 127.0.0.1
```

Open `http://127.0.0.1:8000/`. After TypeScript edits, run `tsc -p .` again and
reload. Edit TypeScript sources; never edit generated JavaScript manually.

After interpreter changes, regenerate the browser payload before compiling the
frontend:

```sh
./scripts/build-c-interpreter-wasm.sh
tsc -p .
```

The build script installs the `wasm32-wasip1` Rust target if needed. Its output,
`shared-c-interpreter-wasm-data.ts`, is generated and should not be edited by
hand. Native and browser execution use the same 32 MiB evaluator stack budget.

## Validate

Run the complete native, frontend, and browser-Wasm checks:

```sh
./scripts/check.sh
```

The individual checks are:

```sh
cargo fmt --manifest-path rust/c_interpreter/Cargo.toml --check
cargo test --manifest-path rust/c_interpreter/Cargo.toml --all-targets
tsc -p .
node scripts/test-progress.ts
./scripts/build-c-interpreter-wasm.sh
tsc -p .
node scripts/test-c-interpreter-wasm.ts
node scripts/test-program-state.ts
git diff --check
```

The Wasm check runs the C fixtures in
`rust/c_interpreter/tests/standard_examples/` through the generated JavaScript
bridge. It catches browser-specific host ABI differences that native tests
cannot cover. Progress tests exercise persistence and browser-storage failures.
Use the actual lesson pages for interaction and responsive-layout checks.

## Level replays

The optional level recorder sends a per-page replay on Check. Encoding,
compression and uploads run in a dedicated Web Worker. The collector, privacy
boundary, deployment configuration and local validation instructions are in
[`telemetry/README.md`](telemetry/README.md). Open `replay.html` to play downloaded
replays locally; `privacy.html` provides the learner's opt-out setting.

## Source map

- Numbered `.ts` files define lesson content; `nav-items.ts` defines navigation.
- `shared-program-template.ts`, `shared-expression-template.ts`,
  `shared-code-editor.ts`, and `shared-code-output-template.ts` implement the
  four exercise types. `shared-program-state.ts` compares workspace answers
  and generates hints; `sandbox.ts` implements the multi-file playground.
- `shared-core-dom.ts` provides page layout, navigation, text rendering, and
  stepping. `shared-workspace-dom.ts` renders variables, arrays, and aggregates.
- `shared-code-editor-surface.ts` owns code highlighting and gutters.
  `shared-diagnostics.ts` connects annotated messages to source highlights;
  `shared-code-runtime-issues.ts` presents diagnostics and runtime stops.
- `shared-progress.ts` owns lesson and sandbox persistence.
- `shared-c-interpreter.ts`, `shared-c-wasm-bridge.ts`, and
  `shared-c-result-normalization.ts` connect the UI to Wasm.
- [`rust/c_interpreter/ARCHITECTURE.md`](rust/c_interpreter/ARCHITECTURE.md)
  describes parsing, linking, browser requests, and evaluator components.
