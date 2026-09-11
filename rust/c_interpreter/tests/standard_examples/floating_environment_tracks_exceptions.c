#include <fenv.h>
#include <math.h>

int main(void) {
    fenv_t saved;
    fexcept_t invalid;

    if (feclearexcept(FE_ALL_EXCEPT) != 0) return 1;
    if (feraiseexcept(FE_INVALID) != 0) return 2;
    if (!(fetestexcept(FE_INVALID) & FE_INVALID)) return 3;
    if (fegetexceptflag(&invalid, FE_INVALID) != 0) return 4;
    if (feclearexcept(FE_ALL_EXCEPT) != 0) return 5;
    if (fesetexceptflag(&invalid, FE_INVALID) != 0) return 6;
    if (!(fetestexcept(FE_INVALID) & FE_INVALID)) return 7;

    if (fegetenv(&saved) != 0) return 8;
    if (feclearexcept(FE_ALL_EXCEPT) != 0) return 9;
    if (fesetenv(&saved) != 0) return 10;
    if (!(fetestexcept(FE_INVALID) & FE_INVALID)) return 11;

    if (feclearexcept(FE_ALL_EXCEPT) != 0) return 12;
    (void)log(0.0);
    if ((math_errhandling & MATH_ERREXCEPT) &&
        !(fetestexcept(FE_DIVBYZERO) & FE_DIVBYZERO)) return 13;
    if (fesetround(FE_DOWNWARD) != 0 || rint(1.75) != 1.0) return 14;
    if (nearbyint(-1.25) != -2.0 || lrint(2.75) != 2) return 15;
    if (fesetround(FE_UPWARD) != 0 || rint(-1.75) != -1.0) return 16;
    if (llrint(2.25) != 3) return 17;
    if (fesetenv(FE_DFL_ENV) != 0 || fegetround() != FE_TONEAREST) return 18;
    if (fesetround(12345) == 0 || fegetround() != FE_TONEAREST) return 19;
    feclearexcept(FE_ALL_EXCEPT);
    (void)sqrt(-1.0);
    if (!(fetestexcept(FE_INVALID) & FE_INVALID)) return 20;
    feclearexcept(FE_ALL_EXCEPT);
    (void)exp(10000.0);
    if (!(fetestexcept(FE_OVERFLOW) & FE_OVERFLOW)) return 21;
    feclearexcept(FE_ALL_EXCEPT);
    (void)pow(0.0, -1.0);
    if (!(fetestexcept(FE_DIVBYZERO) & FE_DIVBYZERO)) return 22;
    return 0;
}
