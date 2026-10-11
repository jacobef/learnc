# Interpreter performance work

Real-project measurements supplement the original small kernels in [README.md](README.md).
All measurements use the actual app Wasm bridge, with function tracing disabled,
and verify the result against optimized native C. The timing includes parsing,
execution, main's five trace events, serialization, and JavaScript normalization.

The saved measurements describe earlier versions. The expression side-effect,
switch-dispatch, and union-type analysis caches were removed on October 4, 2026;
the current interpreter always performs sequencing checks and recomputes those analyses.

## Verified results at the usage cutoff

Stopped when account usage reached 15% used (85% remaining). These warm minimal-trace
results were measured on Apple M4, Node 26.7.0. The generated payload hash in
[`projects-stage15.json`](projects-stage15.json) exactly matches the final checked-in-file
contents (changes remain uncommitted). Baseline samples are in
[`projects-baseline-recheck.json`](projects-baseline-recheck.json).

| Real project | Before | After | Speedup | After / native C |
| --- | ---: | ---: | ---: | ---: |
| jsmn | 2,474 ms | 1,033 ms | 2.39× | 11,474× |
| cJSON | 879 ms | 170 ms | 5.17× | 2,648× |
| tiny-AES-c | 1,835 ms | 354 ms | 5.19× | 69,531× |
| zlib puff | 369 ms | 159 ms | 2.32× | 9,635× |

The remaining gap is roughly **10³–10⁵× slower than optimized native C**, depending
on the project. Interpreter timings include parsing and result handling, while
native timings measure the compiled workload, so these are end-to-end comparisons,
not isolated instruction-dispatch costs.

The baseline includes the small library-function-address initializer fix needed
to run cJSON; it otherwise uses the original interpreter and size-optimized build.
All timed workloads verify their output against native C.

Full tracing also improved: the pre-final stage14 run measured 155 / 373 / 114 ms
for the larger mixing / array / recursion kernels, versus the original
313 / 825 / 232 ms (about 2.0–2.2× faster). These are supporting measurements of
an earlier optimization stage, not a rerun of the final payload. Sharing trace type
metadata reduced one 466-event array trace from 8,514,575 to 5,860,622 JSON bytes
(31% smaller; [`trace-payload.json`](trace-payload.json)).

The speed-oriented build increases Wasm size from 1,771,698 to 2,550,974 bytes
(44%). Cold-start timing is not reported: an overlapping verification process
invalidated that measurement. `startup-interfered.json` is an invalid experimental
artifact and must not be used for conclusions. Other numbered stage files are
intermediate experiments; stage16 includes a write optimization subsequently
removed. Larger-document stage16 results likewise are not final measurements.

## Validation

The final source passed `./scripts/check.sh`, including Rust formatting, 904 Rust
tests (one ignored), TypeScript compilation, Wasm rebuild, and the repository's
Node/browser-runtime checks. Array conformance passed 206/206 cases against
native Clang (146 defined, 60 undefined-behavior cases); core conformance passed
35 cases. Regression coverage includes literal reuse, bitfield promotion,
loop-state hash collisions, and trace metadata reference validation.

Six focused normalized trace comparisons passed before/after metadata pooling.
An additional bounded real-project comparison passed for jsmn, then stopped on
cJSON global ordering/address differences. The linker collects globals through
HashMap iteration, which can vary across separate Wasm instances; this additional
comparison is unresolved and is not counted as a passing equivalence check.

## Workloads

- **jsmn 1.1.0:** tokenize a nested JSON document and traverse the tokens, 100 times.
- **cJSON 1.7.18:** parse, navigate, print, and delete a nested JSON document, eight times.
- **tiny-AES-c 1.0.0:** AES-CBC encrypt/decrypt a 64-byte buffer four times; verify every round trip.
- **zlib puff from 1.3.1:** inflate four copies of the JSON document, four times; checksum the output.

Pinned upstream source files and license notices are in [vendor](vendor/README.md).
The harness removes local header includes when combining translation units.
The AES buffer is an enclosing array of the library's `state_t` type, accessed
through its object representation, to respect the interpreter's pointer bounds.

Each case has two warmups followed by five timed runs; the median is reported.
Native C uses Clang `-O2`, seven calibrated batches, and checksums that prevent
dead-code elimination. Native timing excludes process startup and compilation.
Initial Wasm compilation and instantiation are excluded as well. Results here
measure warm API calls, not browser rendering or the first page load.

## Changes

- Build the Wasm release for execution speed instead of minimum binary size.
- Enable existing safe preprocessing of pure expressions and type/switch metadata in the browser runner.
- Remove redundant arithmetic type walks and temporary expression-tree copies.
- Update ordinary scalar storage directly while retaining applicable safety checks.
- Maintain initialization counts instead of rescanning partially initialized arrays on every write.
- Update existing byte-backed storage in place, and serialize only the affected array element for byte reads.
- Keep string literals in compact byte storage and reuse their static allocations.
- Use a bounded hash table for recent loop states, with exact equality checks and one cached hash per snapshot.
- Reuse loop syntax analysis while resolving actual variables and object identities separately for every invocation.
- Share trace type descriptions, send a per-trace type table, and decode each description once; find pointer aliases through an address index.

## Reproduce

```sh
./scripts/build-c-interpreter-wasm.sh
tsc -p .
node experiments/interpreter-benchmark/projects.ts
```

Useful options:

- `BENCH_QUICK=1`: run just the larger input sizes.
- `BENCH_PROJECT=cJSON`: run one named project (`jsmn`, `cJSON`, `tiny-AES-c`, or `zlib-puff`).
- `BENCH_DOCUMENT=large`: use 64 JSON records instead of eight.
- `BENCH_ITERATIONS=1`: override the iteration count for larger documents.
- `BENCH_MUTABLE_INPUT=1`: use writable JSON buffers instead of literals.
- `BENCH_OUTPUT=filename.json`: preserve a separate result file.
- `BENCH_SAMPLES=7`: change the number of measured trials.
- `BENCH_WASM_DATA=/absolute/path/to/payload.ts`: load a saved generated payload for a controlled comparison.
- `BENCH_INTERPRETER_MODULE=/absolute/path/to/js/shared-c-interpreter.js`: use a saved matching JavaScript runtime when comparing a payload with an older wire format.

`startup.ts` measures the first API call in a fresh Node process, followed by five warm calls. Run it in separate processes to compare initialization cost. It excludes module imports and network transfer.

Each result records source and payload hashes, all timing samples, the CPU and Node version,
and a checksum comparison with native C. Run timing commands sequentially without
compilation or other benchmarks competing for the CPU. A few percent is measurement
noise; the large before/after differences are the relevant result.

SQLite 3.53.4 was also tried using the repository's existing SQLite adapter. Both
the original and optimized interpreter stop on the same converted-pointer-bounds
diagnostic during database opening. Failed runs are excluded from performance claims.
