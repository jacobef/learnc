#include <tgmath.h>
#include <stdlib.h>
#include <wchar.h>

int main(void) {
    long double x = 1.0L, integer;
    int exponent, quotient;

    if (atan(x) < 0.78L || atan(x) > 0.79L) return 1;
    if (copysign(1, -x) != -1.0L) return 2;
    if (nexttoward(1.0f, 1.0L + 0x1p-40L) <= 1.0f) return 3;
    if (nexttoward(1.0, 2.0L) <= 1.0) return 4;
    if (modfl(1.5L, &integer) != 0.5L || integer != 1.0L) return 5;
    if (frexpl(8.0L, &exponent) != 0.5L || exponent != 4) return 6;
    if (remquol(5.0L, 2.0L, &quotient) != 1.0L) return 7;
    if (strtold("1.25", 0) != 1.25L || wcstold(L"1.25", 0) != 1.25L) return 8;
    if (llroundl(1.75L) != 2 || sqrtl(4.0L) != 2.0L) return 9;
    return 0;
}
