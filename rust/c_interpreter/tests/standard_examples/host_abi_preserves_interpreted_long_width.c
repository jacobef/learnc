#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

int main(void) {
    char text[80];
    ldiv_t quotient;

    if (atol("4294967296") != 4294967296L) return 1;
    if (strtol("4294967296", NULL, 10) != 4294967296L) return 2;
    if (strtoul("4294967296", NULL, 10) != 4294967296UL) return 3;
    if (wcstol(L"4294967296", NULL, 10) != 4294967296L) return 4;
    if (wcstoul(L"4294967296", NULL, 10) != 4294967296UL) return 5;

    quotient = ldiv(4294967296L, 3L);
    if (quotient.quot != 1431655765L || quotient.rem != 1L) return 6;
    if (lround(4294967296.0) != 4294967296L) return 7;
    if (!isinf(scalbln(1.0, 4294967296L))) return 8;

    snprintf(text, sizeof text, "%ld %lu %.2Lf", 4294967296L, 4294967296UL, 1.25L);
    if (strcmp(text, "4294967296 4294967296 1.25") != 0) return 9;
    return 0;
}
