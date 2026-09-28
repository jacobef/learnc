# Interpreter versus compiled C

This page records the original baseline. See [optimization results](OPTIMIZATIONS.md)
for the subsequent real-project measurements and verified performance changes.

Measured September 28, 2026 on an Apple M4, macOS arm64, Node 26.7.0,
Apple Clang 17. Raw samples and the bundled Wasm payload's SHA-256 are in
[`results.json`](results.json).

## Order-of-magnitude estimate

For these CPU-bound workloads, use **10,000× slower than optimized native C**
as a rough estimate for the browser's interpreter when it does not record the
benchmark function's individual steps. Fully recording those steps costs roughly
**100,000–1,000,000×**, reaching **2,500,000×** for the array workload.
The native Rust interpreter with visualization disabled costs roughly
**1,000–10,000×**.

These are workload-dependent ratios, not a universal interpreter constant.
Tracing is particularly expensive when every step captures an array's state.

## Results

Medians of seven samples at the larger workload size:

| Workload | Clang -O2 | Native Rust interpreter, no visualization | Wasm, main trace only | Wasm, full function trace |
| --- | ---: | ---: | ---: | ---: |
| Integer mixing, 2,000 iterations | 2.704 µs | 3.949 ms (1,461×) | 10.643 ms (3,936×) | 313.257 ms (115,857×) |
| Array updates, 1,024 iterations | 0.337 µs | 4.641 ms (13,772×) | 12.051 ms (35,765×) | 824.730 ms (2,447,589×) |
| Recursive Fibonacci-shaped calls, depth 13 | 0.373 µs | 1.411 ms (3,778×) | 4.950 ms (13,257×) | 231.760 ms (620,653×) |

Against unoptimized Clang `-O0`, the corresponding slowdowns are smaller:

| Workload | Native Rust interpreter | Wasm, main trace only | Wasm, full function trace |
| --- | ---: | ---: | ---: |
| Integer mixing | 409× | 1,101× | 32,418× |
| Array updates | 2,540× | 6,597× | 451,443× |
| Recursive calls | 1,669× | 5,858× | 274,227× |

To check whether source parsing and fixed setup dominated, subtract the small
case's time from the large case's time, for both interpreter and native C.
The resulting incremental slowdowns versus `-O2` are:

| Workload | Native Rust interpreter | Wasm, main trace only | Wasm, full function trace |
| --- | ---: | ---: | ---: |
| Integer mixing | 918× | 2,842× | 114,878× |
| Array updates | 12,927× | 31,087× | 2,548,574× |
| Recursive calls | 2,720× | 9,656× | 645,253× |

The same approximate orders of magnitude remain after this subtraction.

## Method and limits

- All paths run the same unsigned-integer C kernels in `kernels.c`. Unsigned
  overflow is defined. Inputs are runtime values, and every output is checked
  against both native builds. Every interpreted run must finish successfully
  without an execution-limit stop.
- Native C times only the kernel, repeatedly called through a volatile function
  pointer with a volatile input and an observable checksum. Functions are marked
  noinline in native builds. Each timed batch lasts roughly 50 ms or more; seven
  batches are measured. Process startup, compilation, and printing are excluded.
- Interpreter timing includes preprocessing/parsing, execution, a single scanf
  and printf, and result construction. Wasm timing also includes the bridge,
  trace serialization, and JavaScript result normalization. Two runs of each
  configuration warm it before seven timed runs; initial Wasm compilation and
  instantiation are excluded. This is an end-to-end warm API comparison, not a
  measurement of the evaluator alone. The size-difference calculation above
  approximately removes fixed costs but is not an isolated evaluator timer.
- Wasm uses the existing generated app payload, which is built with the
  repository's size-optimized release profile. Node runs the actual app bridge
  and normalization code. This is not a measurement inside a browser tab.
- The native Rust interpreter is freshly built from current workspace source
  using opt-level 3, fat LTO, one codegen unit, the default allocator and default
  optimizing precomputations. Browser and native timings therefore describe two
  actual operating modes, not a controlled measurement of Wasm overhead alone.
- `traceFunctions: false` records main's 9–10 events but skips the kernel's
  individual events. `traceFunctions: true` captures all kernel events, matching
  the sandbox's function-tracing setting. Larger cases produce 10,014, 4,306,
  and 3,022 events respectively. The benchmark raises the step budget and leaves
  the trace count uncapped so that the entire workload completes. The UI normally
  starts with a 512-event prefix and reruns with larger prefixes as needed;
  these full-run times are not initial UI latency estimates.
- No DOM rendering, editor work, user interaction, or repeated prefix execution
  is included. Large programs, I/O-heavy programs, and library calls may have
  very different ratios. Samples were taken sequentially on one machine;
  differences of a few percent should not be treated as significant.

## Reproduce

From the repository root:

```sh
tsc -p .
node experiments/interpreter-benchmark/benchmark.ts
```

Requires Node with TypeScript stripping, Clang, Cargo, and cached Rust
dependencies (`cargo build --offline`). Builds are placed in a new temporary
directory; `BENCH_BUILD_DIR` can point to an existing benchmark build directory
to reuse Rust build artifacts. Running the script replaces `results.json`.
The native interpreter harness and benchmark files are separate from app code.
