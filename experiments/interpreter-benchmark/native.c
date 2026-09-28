#define NATIVE_BENCH
#ifdef PROJECT_SOURCE
#include PROJECT_SOURCE
#else
#include "kernels.c"
#endif
#include <stdlib.h>
#include <time.h>

static double seconds(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec * 1e-9;
}

int main(int argc, char **argv) {
    if (argc != 4) return 1;
    unsigned kind = (unsigned)strtoul(argv[1], NULL, 10);
    unsigned n = (unsigned)strtoul(argv[2], NULL, 10);
    volatile unsigned seed = (unsigned)strtoul(argv[3], NULL, 10);
    unsigned (*volatile kernel)(unsigned, unsigned) =
#ifdef PROJECT_SOURCE
        benchmark_run;
    (void)kind;
#else
        kind == 0 ? mix : kind == 1 ? array : recursive;
#endif
    unsigned check = kernel(n, seed);
    unsigned checksum = 0;
    unsigned long repeats = 1;
    double elapsed;
    do {
        double start = seconds();
        for (unsigned long i = 0; i < repeats; i++) checksum += kernel(n, seed);
        elapsed = seconds() - start;
        if (elapsed >= 0.05) break;
        repeats *= 2;
    } while (1);
    printf("{\"output\":%u,\"checksum\":%u,\"repeats\":%lu,\"ms\":[", check, checksum, repeats);
    for (unsigned trial = 0; trial < 7; trial++) {
        double start = seconds();
        for (unsigned long i = 0; i < repeats; i++) checksum += kernel(n, seed);
        elapsed = seconds() - start;
        printf("%s%.9f", trial ? "," : "", elapsed * 1000 / repeats);
    }
    printf("],\"finalChecksum\":%u}\n", checksum);
    return 0;
}
