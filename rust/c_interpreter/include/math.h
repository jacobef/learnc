#ifndef __CODEX_MATH_H
#define __CODEX_MATH_H

typedef float float_t;
typedef double double_t;

#define FP_INFINITE 2
#define FP_NAN 1
#define FP_NORMAL 4
#define FP_SUBNORMAL 5
#define FP_ZERO 3

#define FP_ILOGB0 (-2147483647 - 1)
#define FP_ILOGBNAN (-2147483647 - 1)

#define MATH_ERRNO 1
#define MATH_ERREXCEPT 2

#define FP_FAST_FMA 1
#define FP_FAST_FMAF 1
#define FP_FAST_FMAL 1

extern int __codex_signgam;

int __codex_math_errhandling(void);
double __codex_huge_val(void);
float __codex_huge_valf(void);
long double __codex_huge_vall(void);
int __codex_fpclassify(double x);
int __codex_isfinite(double x);
int __codex_isinf(double x);
int __codex_isnan(double x);
int __codex_isnormal(double x);
int __codex_signbit(double x);
int __codex_isgreater(double x, double y);
int __codex_isgreaterequal(double x, double y);
int __codex_isless(double x, double y);
int __codex_islessequal(double x, double y);
int __codex_islessgreater(double x, double y);
int __codex_isunordered(double x, double y);

#define math_errhandling (__codex_math_errhandling())
#define HUGE_VAL (__codex_huge_val())
#define HUGE_VALF (__codex_huge_valf())
#define HUGE_VALL (__codex_huge_vall())
#define INFINITY HUGE_VALF
#define NAN nanf("")

#define fpclassify(x) __codex_fpclassify((double)(x))
#define isfinite(x) __codex_isfinite((double)(x))
#define isinf(x) __codex_isinf((double)(x))
#define isnan(x) __codex_isnan((double)(x))
#define isnormal(x) __codex_isnormal((double)(x))
#define signbit(x) __codex_signbit((double)(x))

#define isgreater(x, y) __codex_isgreater((double)(x), (double)(y))
#define isgreaterequal(x, y) __codex_isgreaterequal((double)(x), (double)(y))
#define isless(x, y) __codex_isless((double)(x), (double)(y))
#define islessequal(x, y) __codex_islessequal((double)(x), (double)(y))
#define islessgreater(x, y) __codex_islessgreater((double)(x), (double)(y))
#define isunordered(x, y) __codex_isunordered((double)(x), (double)(y))

#define signgam (__codex_signgam)

#define __CODEX_DECLARE_UNARY_REAL(name) \
double name(double x); \
float name##f(float x); \
long double name##l(long double x)

#define __CODEX_DECLARE_BINARY_REAL(name) \
double name(double x, double y); \
float name##f(float x, float y); \
long double name##l(long double x, long double y)

__CODEX_DECLARE_UNARY_REAL(acos);
__CODEX_DECLARE_UNARY_REAL(asin);
__CODEX_DECLARE_UNARY_REAL(atan);
__CODEX_DECLARE_BINARY_REAL(atan2);
__CODEX_DECLARE_UNARY_REAL(cos);
__CODEX_DECLARE_UNARY_REAL(sin);
__CODEX_DECLARE_UNARY_REAL(tan);

__CODEX_DECLARE_UNARY_REAL(acosh);
__CODEX_DECLARE_UNARY_REAL(asinh);
__CODEX_DECLARE_UNARY_REAL(atanh);
__CODEX_DECLARE_UNARY_REAL(cosh);
__CODEX_DECLARE_UNARY_REAL(sinh);
__CODEX_DECLARE_UNARY_REAL(tanh);

__CODEX_DECLARE_UNARY_REAL(exp);
__CODEX_DECLARE_UNARY_REAL(log);
__CODEX_DECLARE_UNARY_REAL(log10);
__CODEX_DECLARE_UNARY_REAL(exp2);
__CODEX_DECLARE_UNARY_REAL(expm1);
__CODEX_DECLARE_UNARY_REAL(log1p);
__CODEX_DECLARE_UNARY_REAL(log2);
__CODEX_DECLARE_UNARY_REAL(logb);
__CODEX_DECLARE_BINARY_REAL(pow);
__CODEX_DECLARE_UNARY_REAL(sqrt);
__CODEX_DECLARE_UNARY_REAL(cbrt);
__CODEX_DECLARE_BINARY_REAL(hypot);

__CODEX_DECLARE_UNARY_REAL(erf);
__CODEX_DECLARE_UNARY_REAL(erfc);
__CODEX_DECLARE_UNARY_REAL(tgamma);
__CODEX_DECLARE_UNARY_REAL(lgamma);

__CODEX_DECLARE_UNARY_REAL(fabs);
__CODEX_DECLARE_UNARY_REAL(ceil);
__CODEX_DECLARE_UNARY_REAL(floor);
__CODEX_DECLARE_BINARY_REAL(fmod);
__CODEX_DECLARE_UNARY_REAL(trunc);
__CODEX_DECLARE_UNARY_REAL(round);
__CODEX_DECLARE_UNARY_REAL(rint);
__CODEX_DECLARE_UNARY_REAL(nearbyint);
__CODEX_DECLARE_BINARY_REAL(remainder);
__CODEX_DECLARE_BINARY_REAL(copysign);
__CODEX_DECLARE_BINARY_REAL(nextafter);
__CODEX_DECLARE_BINARY_REAL(fdim);
__CODEX_DECLARE_BINARY_REAL(fmax);
__CODEX_DECLARE_BINARY_REAL(fmin);

double frexp(double x, int *exp);
float frexpf(float x, int *exp);
long double frexpl(long double x, int *exp);

double ldexp(double x, int exp);
float ldexpf(float x, int exp);
long double ldexpl(long double x, int exp);

int ilogb(double x);
int ilogbf(float x);
int ilogbl(long double x);

double scalbn(double x, int n);
float scalbnf(float x, int n);
long double scalbnl(long double x, int n);

double scalbln(double x, long n);
float scalblnf(float x, long n);
long double scalblnl(long double x, long n);

long lround(double x);
long lroundf(float x);
long lroundl(long double x);

long lrint(double x);
long lrintf(float x);
long lrintl(long double x);

long long llround(double x);
long long llroundf(float x);
long long llroundl(long double x);

long long llrint(double x);
long long llrintf(float x);
long long llrintl(long double x);

double modf(double x, double *iptr);
float modff(float x, float *iptr);
long double modfl(long double x, long double *iptr);

double remquo(double x, double y, int *quo);
float remquof(float x, float y, int *quo);
long double remquol(long double x, long double y, int *quo);

double nan(const char *tagp);
float nanf(const char *tagp);
long double nanl(const char *tagp);

double nexttoward(double x, long double y);
float nexttowardf(float x, long double y);
long double nexttowardl(long double x, long double y);

double fma(double x, double y, double z);
float fmaf(float x, float y, float z);
long double fmal(long double x, long double y, long double z);

#undef __CODEX_DECLARE_UNARY_REAL
#undef __CODEX_DECLARE_BINARY_REAL

#endif
