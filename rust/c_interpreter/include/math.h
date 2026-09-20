#ifndef __CBOXES_MATH_H
#define __CBOXES_MATH_H

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

extern int __cboxes_signgam;

int __cboxes_math_errhandling(void);
double __cboxes_huge_val(void);
float __cboxes_huge_valf(void);
long double __cboxes_huge_vall(void);
int __cboxes_fpclassify(double x);
int __cboxes_fpclassifyf(float x);
int __cboxes_fpclassifyl(long double x);
int __cboxes_isfinite(double x);
int __cboxes_isinf(double x);
int __cboxes_isnan(double x);
int __cboxes_isnormal(double x);
int __cboxes_isnormalf(float x);
int __cboxes_isnormall(long double x);
int __cboxes_signbit(double x);
int __cboxes_isgreater(double x, double y);
int __cboxes_isgreaterequal(double x, double y);
int __cboxes_isless(double x, double y);
int __cboxes_islessequal(double x, double y);
int __cboxes_islessgreater(double x, double y);
int __cboxes_isunordered(double x, double y);

#define math_errhandling (__cboxes_math_errhandling())
#define HUGE_VAL (__cboxes_huge_val())
#define HUGE_VALF (__cboxes_huge_valf())
#define HUGE_VALL (__cboxes_huge_vall())
#define INFINITY HUGE_VALF
#define NAN nanf("")

#define fpclassify(x) _Generic((x), float: __cboxes_fpclassifyf, long double: __cboxes_fpclassifyl, default: __cboxes_fpclassify)((x))
#define isfinite(x) __cboxes_isfinite((double)(x))
#define isinf(x) __cboxes_isinf((double)(x))
#define isnan(x) __cboxes_isnan((double)(x))
#define isnormal(x) _Generic((x), float: __cboxes_isnormalf, long double: __cboxes_isnormall, default: __cboxes_isnormal)((x))
#define signbit(x) __cboxes_signbit((double)(x))

#define isgreater(x, y) __cboxes_isgreater((double)(x), (double)(y))
#define isgreaterequal(x, y) __cboxes_isgreaterequal((double)(x), (double)(y))
#define isless(x, y) __cboxes_isless((double)(x), (double)(y))
#define islessequal(x, y) __cboxes_islessequal((double)(x), (double)(y))
#define islessgreater(x, y) __cboxes_islessgreater((double)(x), (double)(y))
#define isunordered(x, y) __cboxes_isunordered((double)(x), (double)(y))

#define signgam (__cboxes_signgam)

#define __CBOXES_DECLARE_UNARY_REAL(name) \
double name(double x); \
float name##f(float x); \
long double name##l(long double x)

#define __CBOXES_DECLARE_BINARY_REAL(name) \
double name(double x, double y); \
float name##f(float x, float y); \
long double name##l(long double x, long double y)

__CBOXES_DECLARE_UNARY_REAL(acos);
__CBOXES_DECLARE_UNARY_REAL(asin);
__CBOXES_DECLARE_UNARY_REAL(atan);
__CBOXES_DECLARE_BINARY_REAL(atan2);
__CBOXES_DECLARE_UNARY_REAL(cos);
__CBOXES_DECLARE_UNARY_REAL(sin);
__CBOXES_DECLARE_UNARY_REAL(tan);

__CBOXES_DECLARE_UNARY_REAL(acosh);
__CBOXES_DECLARE_UNARY_REAL(asinh);
__CBOXES_DECLARE_UNARY_REAL(atanh);
__CBOXES_DECLARE_UNARY_REAL(cosh);
__CBOXES_DECLARE_UNARY_REAL(sinh);
__CBOXES_DECLARE_UNARY_REAL(tanh);

__CBOXES_DECLARE_UNARY_REAL(exp);
__CBOXES_DECLARE_UNARY_REAL(log);
__CBOXES_DECLARE_UNARY_REAL(log10);
__CBOXES_DECLARE_UNARY_REAL(exp2);
__CBOXES_DECLARE_UNARY_REAL(expm1);
__CBOXES_DECLARE_UNARY_REAL(log1p);
__CBOXES_DECLARE_UNARY_REAL(log2);
__CBOXES_DECLARE_UNARY_REAL(logb);
__CBOXES_DECLARE_BINARY_REAL(pow);
__CBOXES_DECLARE_UNARY_REAL(sqrt);
__CBOXES_DECLARE_UNARY_REAL(cbrt);
__CBOXES_DECLARE_BINARY_REAL(hypot);

__CBOXES_DECLARE_UNARY_REAL(erf);
__CBOXES_DECLARE_UNARY_REAL(erfc);
__CBOXES_DECLARE_UNARY_REAL(tgamma);
__CBOXES_DECLARE_UNARY_REAL(lgamma);

__CBOXES_DECLARE_UNARY_REAL(fabs);
__CBOXES_DECLARE_UNARY_REAL(ceil);
__CBOXES_DECLARE_UNARY_REAL(floor);
__CBOXES_DECLARE_BINARY_REAL(fmod);
__CBOXES_DECLARE_UNARY_REAL(trunc);
__CBOXES_DECLARE_UNARY_REAL(round);
__CBOXES_DECLARE_UNARY_REAL(rint);
__CBOXES_DECLARE_UNARY_REAL(nearbyint);
__CBOXES_DECLARE_BINARY_REAL(remainder);
__CBOXES_DECLARE_BINARY_REAL(copysign);
__CBOXES_DECLARE_BINARY_REAL(nextafter);
__CBOXES_DECLARE_BINARY_REAL(fdim);
__CBOXES_DECLARE_BINARY_REAL(fmax);
__CBOXES_DECLARE_BINARY_REAL(fmin);

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

#undef __CBOXES_DECLARE_UNARY_REAL
#undef __CBOXES_DECLARE_BINARY_REAL

#endif
