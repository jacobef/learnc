#include <stdio.h>

#ifdef NATIVE_BENCH
#define NOINLINE __attribute__((noinline))
#else
#define NOINLINE
#endif

NOINLINE unsigned mix(unsigned n, unsigned seed) {
    unsigned x = seed;
    for (unsigned i = 0; i < n; i++) {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
    }
    return x;
}

NOINLINE unsigned array(unsigned n, unsigned seed) {
    unsigned a[32];
    for (unsigned i = 0; i < 32; i++) a[i] = seed + i;
    for (unsigned i = 0; i < n; i++) {
        unsigned j = i & 31;
        a[j] = (a[j] * 1664525u + 1013904223u) ^ a[(j + 1) & 31];
    }
    unsigned sum = 0;
    for (unsigned i = 0; i < 32; i++) sum ^= a[i];
    return sum;
}

NOINLINE unsigned recursive(unsigned n, unsigned seed) {
    if (n < 2) return seed + n;
    return recursive(n - 1, seed) + recursive(n - 2, seed);
}

#ifndef NATIVE_BENCH
int main(void) {
    unsigned kind, n, seed;
    if (scanf("%u %u %u", &kind, &n, &seed) != 3) return 1;
    unsigned result;
    if (kind == 0) result = mix(n, seed);
    else if (kind == 1) result = array(n, seed);
    else result = recursive(n, seed);
    printf("%u\n", result);
    return 0;
}
#endif
