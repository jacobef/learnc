# Measured results

All kernel times are milliseconds. Medians of five fresh processes, each with one in-process warm-up followed by one timed pass. Input read/conversion is outside the kernel timer. Bend kernel timing includes printing one checksum. Wall time includes BOTH passes and runtime startup, and is not a one-pass compile time.

| Corpus | Stage | Backend | Threads | Order | Load median | Kernel median | Kernel min–max | Samples |
|---|---|---|---:|---|---:|---:|---:|---:|
| chunky | fold | bend | 1 | clustered | 11.575 | 37.648 | 36.943–38.513 | 5 |
| chunky | fold | bend | 4 | clustered | 12.398 | 10.471 | 9.954–10.622 | 5 |
| chunky | fold | bend | 10 | clustered | 11.996 | 7.870 | 7.574–9.570 | 5 |
| chunky | fold | bend-array | 1 | clustered | 3.605 | 35.052 | 34.603–35.515 | 5 |
| chunky | fold | bend-array | 10 | clustered | 3.787 | 6.656 | 6.375–7.053 | 5 |
| chunky | fold | c | 1 | clustered | 1.118 | 7.630 | 7.527–7.766 | 5 |
| chunky | fold | c | 10 | clustered | 1.075 | 1.298 | 1.179–1.741 | 5 |
| chunky | fold | gpu | 10 | clustered | 31.272 | 41.053 | 39.337–53.583 | 5 |
| chunky | fold | gpu-array | 10 | clustered | 3.670 | 34.467 | 28.794–35.847 | 5 |
| chunky | lex | bend | 1 | clustered | 10.748 | 27.366 | 25.614–30.836 | 5 |
| chunky | lex | bend | 4 | clustered | 10.983 | 8.168 | 7.187–8.248 | 5 |
| chunky | lex | bend | 10 | clustered | 12.089 | 6.236 | 5.315–7.005 | 5 |
| chunky | lex | bend-array | 1 | clustered | 3.560 | 23.312 | 23.166–23.852 | 5 |
| chunky | lex | bend-array | 10 | clustered | 3.573 | 4.488 | 4.269–4.597 | 5 |
| chunky | lex | c | 1 | clustered | 1.091 | 7.927 | 7.913–9.068 | 5 |
| chunky | lex | c | 10 | clustered | 1.056 | 1.248 | 1.143–1.797 | 5 |
| chunky | lex | gpu | 10 | clustered | 28.315 | 37.077 | 32.784–43.986 | 5 |
| chunky | lex | gpu-array | 10 | clustered | 3.434 | 25.419 | 23.263–27.936 | 5 |
| many | fold | bend | 1 | clustered | 4.333 | 11.122 | 11.033–11.274 | 5 |
| many | fold | bend | 4 | clustered | 4.452 | 2.960 | 2.927–3.224 | 5 |
| many | fold | bend | 10 | clustered | 4.629 | 2.550 | 2.260–3.473 | 5 |
| many | fold | bend-array | 1 | clustered | 2.064 | 10.403 | 10.331–10.868 | 5 |
| many | fold | bend-array | 10 | clustered | 2.070 | 2.852 | 2.074–9.715 | 5 |
| many | fold | c | 1 | clustered | 1.147 | 3.527 | 3.443–3.568 | 5 |
| many | fold | c | 10 | clustered | 1.049 | 1.022 | 0.979–1.356 | 5 |
| many | fold | gpu | 10 | clustered | 5.018 | 14.066 | 5.084–16.331 | 5 |
| many | fold | gpu-array | 10 | clustered | 2.272 | 6.650 | 6.280–7.627 | 5 |
| many | lex | bend | 1 | clustered | 4.236 | 8.167 | 8.052–8.440 | 5 |
| many | lex | bend | 4 | clustered | 4.387 | 2.205 | 2.164–2.768 | 5 |
| many | lex | bend | 10 | clustered | 4.276 | 1.754 | 1.654–2.323 | 5 |
| many | lex | bend-array | 1 | clustered | 1.933 | 7.321 | 7.269–7.659 | 5 |
| many | lex | bend-array | 10 | clustered | 2.111 | 1.880 | 1.635–1.985 | 5 |
| many | lex | c | 1 | clustered | 1.106 | 2.714 | 2.601–2.828 | 5 |
| many | lex | c | 10 | clustered | 1.188 | 1.001 | 0.951–1.047 | 5 |
| many | lex | gpu | 10 | clustered | 5.278 | 11.080 | 10.503–12.527 | 5 |
| many | lex | gpu-array | 10 | clustered | 2.272 | 5.515 | 5.157–5.542 | 5 |
| skew | fold | bend | 1 | clustered | 6.300 | 21.906 | 21.635–21.937 | 5 |
| skew | fold | bend | 1 | interleaved | 6.651 | 22.370 | 22.096–22.486 | 5 |
| skew | fold | bend | 4 | clustered | 7.004 | 16.769 | 16.441–17.325 | 5 |
| skew | fold | bend | 4 | interleaved | 7.062 | 6.171 | 6.011–7.591 | 5 |
| skew | fold | bend | 10 | clustered | 6.881 | 16.948 | 16.862–17.194 | 5 |
| skew | fold | bend | 10 | interleaved | 7.319 | 5.066 | 4.533–5.309 | 5 |
| skew | fold | bend-array | 1 | clustered | 2.168 | 19.543 | 19.206–19.947 | 5 |
| skew | fold | bend-array | 1 | interleaved | 2.396 | 19.620 | 19.370–20.029 | 5 |
| skew | fold | bend-array | 10 | clustered | 2.341 | 15.314 | 15.051–16.704 | 5 |
| skew | fold | bend-array | 10 | interleaved | 2.431 | 3.963 | 3.528–4.456 | 5 |
| skew | fold | c | 1 | clustered | 0.691 | 4.632 | 4.605–4.950 | 5 |
| skew | fold | c | 1 | interleaved | 0.709 | 5.139 | 4.580–5.517 | 5 |
| skew | fold | c | 10 | clustered | 0.814 | 0.966 | 0.906–1.330 | 5 |
| skew | fold | c | 10 | interleaved | 0.662 | 0.843 | 0.801–1.141 | 5 |
| skew | fold | gpu | 10 | clustered | 17.326 | 39.698 | 38.363–40.197 | 5 |
| skew | fold | gpu | 10 | interleaved | 16.344 | 50.229 | 45.171–62.230 | 5 |
| skew | fold | gpu-array | 10 | clustered | 2.357 | 28.178 | 27.191–33.511 | 5 |
| skew | fold | gpu-array | 10 | interleaved | 2.568 | 39.797 | 30.552–49.343 | 5 |
| skew | lex | bend | 1 | clustered | 5.877 | 16.012 | 15.798–16.513 | 5 |
| skew | lex | bend | 1 | interleaved | 6.262 | 16.555 | 16.382–17.251 | 5 |
| skew | lex | bend | 4 | clustered | 8.479 | 12.258 | 12.147–13.423 | 5 |
| skew | lex | bend | 4 | interleaved | 6.334 | 4.556 | 4.305–5.342 | 5 |
| skew | lex | bend | 10 | clustered | 6.300 | 12.181 | 12.028–13.194 | 5 |
| skew | lex | bend | 10 | interleaved | 6.613 | 3.768 | 3.234–3.879 | 5 |
| skew | lex | bend-array | 1 | clustered | 2.199 | 13.479 | 13.444–13.880 | 5 |
| skew | lex | bend-array | 1 | interleaved | 2.456 | 13.197 | 13.081–13.280 | 5 |
| skew | lex | bend-array | 10 | clustered | 2.334 | 9.890 | 9.714–10.083 | 5 |
| skew | lex | bend-array | 10 | interleaved | 2.452 | 2.522 | 2.458–3.110 | 5 |
| skew | lex | c | 1 | clustered | 0.717 | 4.622 | 4.454–4.993 | 5 |
| skew | lex | c | 1 | interleaved | 0.699 | 4.631 | 4.454–5.256 | 5 |
| skew | lex | c | 10 | clustered | 0.696 | 0.851 | 0.848–1.117 | 5 |
| skew | lex | c | 10 | interleaved | 0.638 | 0.761 | 0.742–1.038 | 5 |
| skew | lex | gpu | 10 | clustered | 15.826 | 33.288 | 32.471–38.475 | 5 |
| skew | lex | gpu | 10 | interleaved | 14.752 | 50.995 | 37.307–58.180 | 5 |
| skew | lex | gpu-array | 10 | clustered | 2.350 | 24.186 | 22.958–27.052 | 5 |
| skew | lex | gpu-array | 10 | interleaved | 2.509 | 32.448 | 30.313–41.666 | 5 |
| smoke | fold | bend-array | 1 | clustered | 0.025 | 0.100 | 0.097–0.102 | 5 |
| smoke | fold | bend-array | 10 | clustered | 0.026 | 0.113 | 0.083–0.189 | 5 |
| smoke | fold | gpu-array | 10 | clustered | 0.104 | 7.281 | 6.233–8.368 | 5 |
| smoke | lex | bend-array | 1 | clustered | 0.031 | 0.077 | 0.066–0.084 | 5 |
| smoke | lex | bend-array | 10 | clustered | 0.030 | 0.167 | 0.082–0.188 | 5 |
| smoke | lex | gpu-array | 10 | clustered | 0.071 | 5.233 | 4.643–6.794 | 5 |
| uniform | fold | bend | 1 | clustered | 8.330 | 18.792 | 18.753–18.909 | 5 |
| uniform | fold | bend | 4 | clustered | 8.330 | 5.661 | 5.006–7.357 | 5 |
| uniform | fold | bend | 10 | clustered | 8.219 | 3.783 | 3.680–4.593 | 5 |
| uniform | fold | bend-array | 1 | clustered | 2.164 | 18.483 | 17.495–19.450 | 5 |
| uniform | fold | bend-array | 10 | clustered | 2.284 | 3.965 | 3.602–4.042 | 5 |
| uniform | fold | c | 1 | clustered | 0.800 | 3.924 | 3.725–4.011 | 5 |
| uniform | fold | c | 10 | clustered | 0.717 | 0.842 | 0.710–1.251 | 5 |
| uniform | fold | gpu | 10 | clustered | 15.343 | 14.934 | 13.564–24.527 | 5 |
| uniform | fold | gpu-array | 10 | clustered | 2.569 | 12.486 | 9.678–15.578 | 5 |
| uniform | lex | bend | 1 | clustered | 6.748 | 13.746 | 13.248–14.137 | 5 |
| uniform | lex | bend | 4 | clustered | 6.744 | 3.556 | 3.527–3.927 | 5 |
| uniform | lex | bend | 10 | clustered | 6.988 | 2.696 | 2.569–3.054 | 5 |
| uniform | lex | bend-array | 1 | clustered | 2.343 | 12.181 | 11.693–13.148 | 5 |
| uniform | lex | bend-array | 10 | clustered | 2.670 | 3.187 | 2.394–17.047 | 5 |
| uniform | lex | c | 1 | clustered | 0.689 | 3.836 | 3.577–3.978 | 5 |
| uniform | lex | c | 10 | clustered | 0.636 | 0.713 | 0.679–0.903 | 5 |
| uniform | lex | gpu | 10 | clustered | 13.590 | 21.163 | 12.625–28.765 | 5 |
| uniform | lex | gpu-array | 10 | clustered | 2.611 | 15.373 | 8.271–18.939 | 5 |

## Clang process-per-file reference

Seconds. Full C parsing/semantics and, in object mode, code generation/object writing. No linking. These do more work than the Bend experiment. Filesystem caches are warm; all compiler invocations are fresh.

| Corpus | Mode | Workers | Median seconds | Min–max | Samples |
|---|---|---:|---:|---:|---:|
| many | object | 10 | 91.726 | 87.050–91.808 | 3 |
| many | syntax | 10 | 69.624 | 59.721–74.486 | 3 |
| uniform | object | 10 | 16.457 | 16.289–16.760 | 3 |
| uniform | syntax | 10 | 13.995 | 13.255–14.406 | 3 |

## Clang unity-build control

Same generated definitions concatenated into one translation unit. This corpus has unique symbols and no headers, so concatenation is safe here; it is not generally safe for arbitrary C projects.

| Corpus | Mode | Median seconds | Samples |
|---|---|---:|---:|
| many | object | 0.410 | 3 |
| many | syntax | 0.066 | 3 |
| uniform | object | 0.478 | 3 |
| uniform | syntax | 0.095 | 3 |

## Reading and packing individual source files

Five samples after one warm-up, cached filesystem. Python reads every individual file and writes the identical packed source corpus; SHA-256 is checked outside the timer. A practical input-cost reference, not an optimized Bend I/O implementation.

| Corpus | Median milliseconds | Min–max |
|---|---:|---:|
| many | 145.882 | 144.917–175.570 |
| uniform | 36.853 | 33.278–53.141 |
