# Feasibility assessment

**The experiments demonstrate useful Bend CPU parallelism, but do not demonstrate
that a Bend C compiler would beat an efficiently parallel conventional compiler.
The clearest measured opportunity is batching many tiny files; that advantage is
available to Clang too.** The Metal GPU lost to the CPU on these front-end kernels.

This used the actual September 17 Bend 2.0.5 release, pinned at
`e6676b080f25b1bc1bf5b5b7d7a17e22f8022599`, on an Apple M4 with 32 GB RAM.
No full compiler was built. See [methodology and reproduction](README.md),
[all timing tables](results/SUMMARY.md) and [raw kernel samples](results/kernels.jsonl).

## Experiment 1: parallel lexer and constant-expression folder

The balanced corpus has 4,096 files, 16,384 functions and 3.00 MB of source.
Each time below is the median of five **steady-state kernel** samples, excluding
input conversion and runtime startup. Folding includes lexing into a token IR.

| Implementation | Lex + fold time | Input conversion/read time |
|---|---:|---:|
| C, 1 CPU thread, compact arrays | 3.924 ms | 0.800 ms |
| C, 10 CPU threads, compact arrays | **0.842 ms** | 0.717 ms |
| Bend, 1 CPU thread, source lists | 18.792 ms | 8.330 ms |
| Bend, 10 CPU threads, source lists | **3.783 ms** | 8.219 ms |
| Bend, Metal GPU, source lists | 14.934 ms | 15.343 ms |
| Bend, 10 CPU threads, source arrays | 3.965 ms | **2.284 ms** |
| Bend, Metal GPU, source arrays | 12.486 ms | 2.569 ms |

Bend's CPU kernel scales **4.97×** from one to ten threads. Nevertheless, the
10-thread Bend list version is **4.49× slower** than the 10-thread C implementation
of the same restricted task. GPU source arrays still take **3.15×** the corresponding
Bend CPU kernel time. Individual arrays/list GPU samples vary substantially, so
small differences should not be generalized.

This is not a pure code-generator comparison: C uses compact token arrays and
Bend uses linked tokens. The distinction is practical for a compiler implementation,
and directly motivates the representation control.

## Experiment 2: source layout and input cost

Replacing only source lists with owned `Array<U32>` reduces the balanced CPU
input-conversion median from **8.219 to 2.284 ms (3.60×)**. CPU processing stays
near 4 ms. Token allocation/traversal and expression handling remain, so fixing
source storage alone does not close the gap. On 16,384 tiny files the GPU improves
from 14.066 to 6.650 ms, but the array CPU version is 2.852 ms and C is 1.022 ms.

Reading the actual individual files and packing their bytes with the Python
control takes **36.85 ms for 4,096 files** and **145.88 ms for 16,384 files**, on a
warm filesystem. These are measured scaffolding costs, not a lower bound on C or
Bend I/O. They do show why reporting a 2–4 ms compute kernel as a full project
compile time would be misleading. This prototype's source representation is built
serially; a future design could parallelize preparation, but that is unmeasured.

## Experiment 3: unequal files and static scheduling

The skew corpus has 256 large files and 3,840 small ones. The source and expected
result stay identical; only placement in the fork/join tree changes.

| Lex + fold, 10 CPU threads | Large files clustered | Large files interleaved |
|---|---:|---:|
| Bend, lists | 16.948 ms | 5.066 ms |
| Bend, arrays | 15.314 ms | **3.963 ms** |
| C, dynamic file queue | 0.966 ms | 0.843 ms |

Interleaving gives the Bend array implementation **3.86×** lower processing time.
Static parallelism needs explicit workload planning for uneven translation units.
The CPU benefit does not transfer directly to the GPU: the array GPU gets worse,
from 28.178 to 39.797 ms. Increased divergence among neighboring GPU lanes is a
plausible explanation, not a profiler-confirmed diagnosis. Runtime memory layout
and placement also change with ordering. No GPU kernel here beat its corresponding
multicore CPU implementation on the substantial corpora.

## Experiment 4: conventional compiler and batching control

Both columns below run **real Clang**, not the restricted Bend front end. They
compile exactly the same generated function definitions at `-O0` and write object
output, without linking. Medians are from three full runs.

| Corpus | Separate files, 10 workers | One unity translation unit | Ratio |
|---|---:|---:|---:|
| 4,096 files, 16,384 functions | 16.457 s | 0.478 s | 34.4× |
| 16,384 files, 16,384 functions | 91.726 s | 0.410 s | **223.7×** |

The unity objects each retain all **16,384 external function symbols**, checked
with `llvm-nm`. This is not dead-code elimination of the workload. Parsing-only
Clang times are also recorded separately in the timing tables.

The large ratio exposes process, per-translation-unit and separate-output overhead
on this deliberately tiny-file corpus; it does not isolate process startup alone.
The separate-file reference uses a Python launcher rather than Ninja, and the
unity case produces one object instead of thousands. The largest corpus averages
only 119 source bytes per file and has no includes or dependencies. It is an
intentionally favorable case for batching, not a model of a typical optimized
production build. Real C projects also have static-name collisions, macros and
other reasons why concatenation may change semantics. A resident compiler could
preserve separate translation-unit contexts while amortizing setup; **that design
was not implemented or timed here**.

The numerical gap between millisecond Bend kernels and second-scale full Clang
builds is **not a compiler speedup measurement**. Clang does declaration parsing,
C semantics, code generation and object emission that the prototype omits.

## Verification and confidence

- **480 final timed kernel runs**, across 96 configurations, passed the independent
  expected checksums, with five samples per configuration. Untimed warm-ups are
  also checked. The additional array smoke samples are included in that count.
- Clang-compiled samples from all five corpora produced the expected unsigned
  arithmetic results. All process-per-file compilations succeeded.
- Metal was explicitly required and ran on the GPU; final samples have no missing-
  archive/fallback warnings. Startup and earlier cold-process measurements are
  retained separately and excluded from the steady-state table.
- Timing spread, corpus hashes, source hashes, build times and environment details
  are saved in `results/`. There are no formal proof claims or cross-machine claims.
- Suites ran sequentially, but this was an interactive fanless laptop. Clang's
  longer runs varied (the many-file syntax trials ranged 59.7–74.5 seconds), so
  ratios should be treated as workload-specific observations, not precise constants.

## What would have to be true

A complete build needs discovery/input, preprocessing, parsing, symbol/type
analysis, IR construction/optimization, machine-code/object emission and linking.
File-level independence is already available to ordinary parallel builds. Bend
has to lower the total cost of those stages or expose useful finer parallelism;
expressing the same independent files with a different scheduler is insufficient.

For a GPU stage, a necessary condition is that CPU preparation, runtime setup,
scheduling, memory traffic, GPU execution and collection together beat the
already-parallel CPU stage. Compiler algorithms that are uniform and arithmetic
heavy are a more plausible GPU target than pointer-heavy token streams. This
experiment directly tests the latter and does not test the former.

## Follow-up experiments justified by this work

1. Use realistic preprocessed C from a large public project; keep a conventional
   parallel build as the primary baseline. Measure preprocessing separately,
   including headers and invalidation. Do not extrapolate from tiny header-free
   functions to a whole project.
2. Keep the compiler process resident and batch inputs. Include file discovery,
   source read, conversion, diagnostics and output in the measured critical path.
   Compare equally batched conventional implementations, not just one-process-per-
   file builds.
3. Use compact tokens, indexed syntax/IR arenas and per-worker ownership. Measure
   whether the release compiler can express those structures efficiently without
   copying shared contexts. The input-array control here changes source storage
   only; the token stream still allocates linked nodes.
4. Estimate work using source size/function count, then partition by expected work.
   Real C has irregular files, includes and optimization passes. CPU and GPU
   placement need separate treatment because balancing CPU tasks may increase
   divergence among neighboring GPU lanes.
5. Only then test a uniform numeric optimization or analysis pass on the GPU,
   with resident input, correct outputs and an efficient multicore CPU comparator.
   A positive result there would justify expanding the prototype. A full Bend C
   compiler is not the necessary next experiment.

## Limits of the conclusion

The prototype accepts only its generated subset and is not a compiler that can
replace Clang. It does not diagnose malformed C, preserve per-function emitted
objects, perform linking, support the C standard, or establish formal correctness.
It checks computed results but uses collision-prone 32-bit aggregate digests.

The host is one Apple M4 laptop. Static fork/join, Metal behavior, unified memory,
source representation, token allocation, thermal conditions, compiler maturity,
process launch overhead and this workload all affect the outcome. Results neither
establish a theoretical limit for Bend nor predict CUDA performance on a discrete
GPU. The release is pinned because the compiler and runtime are new and changing.
