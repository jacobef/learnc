#ifndef __CODEX_COMPLEX_H
#define __CODEX_COMPLEX_H

#define complex _Complex

float _Complex __codex_cmplxf(float real, float imag);
double _Complex __codex_cmplx(double real, double imag);
long double _Complex __codex_cmplxl(long double real, long double imag);

#define _Complex_I (__codex_cmplxf(0.0f, 1.0f))
#define I _Complex_I

#define CMPLXF(real, imag) (__codex_cmplxf((float)(real), (float)(imag)))
#define CMPLX(real, imag) (__codex_cmplx((double)(real), (double)(imag)))
#define CMPLXL(real, imag) (__codex_cmplxl((long double)(real), (long double)(imag)))

float _Complex cacosf(float _Complex z);
double _Complex cacos(double _Complex z);
long double _Complex cacosl(long double _Complex z);

float _Complex casinf(float _Complex z);
double _Complex casin(double _Complex z);
long double _Complex casinl(long double _Complex z);

float _Complex catanf(float _Complex z);
double _Complex catan(double _Complex z);
long double _Complex catanl(long double _Complex z);

float _Complex ccosf(float _Complex z);
double _Complex ccos(double _Complex z);
long double _Complex ccosl(long double _Complex z);

float _Complex csinf(float _Complex z);
double _Complex csin(double _Complex z);
long double _Complex csinl(long double _Complex z);

float _Complex ctanf(float _Complex z);
double _Complex ctan(double _Complex z);
long double _Complex ctanl(long double _Complex z);

float _Complex cacoshf(float _Complex z);
double _Complex cacosh(double _Complex z);
long double _Complex cacoshl(long double _Complex z);

float _Complex casinhf(float _Complex z);
double _Complex casinh(double _Complex z);
long double _Complex casinhl(long double _Complex z);

float _Complex catanhf(float _Complex z);
double _Complex catanh(double _Complex z);
long double _Complex catanhl(long double _Complex z);

float _Complex ccoshf(float _Complex z);
double _Complex ccosh(double _Complex z);
long double _Complex ccoshl(long double _Complex z);

float _Complex csinhf(float _Complex z);
double _Complex csinh(double _Complex z);
long double _Complex csinhl(long double _Complex z);

float _Complex ctanhf(float _Complex z);
double _Complex ctanh(double _Complex z);
long double _Complex ctanhl(long double _Complex z);

float _Complex cexpf(float _Complex z);
double _Complex cexp(double _Complex z);
long double _Complex cexpl(long double _Complex z);

float _Complex clogf(float _Complex z);
double _Complex clog(double _Complex z);
long double _Complex clogl(long double _Complex z);

float cabsf(float _Complex z);
double cabs(double _Complex z);
long double cabsl(long double _Complex z);

float _Complex cpowf(float _Complex x, float _Complex y);
double _Complex cpow(double _Complex x, double _Complex y);
long double _Complex cpowl(long double _Complex x, long double _Complex y);

float _Complex csqrtf(float _Complex z);
double _Complex csqrt(double _Complex z);
long double _Complex csqrtl(long double _Complex z);

float cargf(float _Complex z);
double carg(double _Complex z);
long double cargl(long double _Complex z);

float cimagf(float _Complex z);
double cimag(double _Complex z);
long double cimagl(long double _Complex z);

float _Complex conjf(float _Complex z);
double _Complex conj(double _Complex z);
long double _Complex conjl(long double _Complex z);

float _Complex cprojf(float _Complex z);
double _Complex cproj(double _Complex z);
long double _Complex cprojl(long double _Complex z);

float crealf(float _Complex z);
double creal(double _Complex z);
long double creall(long double _Complex z);

#endif
