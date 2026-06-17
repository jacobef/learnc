#ifndef __CODEX_FENV_H
#define __CODEX_FENV_H

typedef unsigned short fexcept_t;
typedef struct {
    unsigned char __opaque[16];
} fenv_t;

const fenv_t *__codex_fe_dfl_env(void);

#define FE_DIVBYZERO 0x0002
#define FE_INEXACT 0x0010
#define FE_INVALID 0x0001
#define FE_OVERFLOW 0x0004
#define FE_UNDERFLOW 0x0008
#define FE_ALL_EXCEPT 0x009f

#define FE_DOWNWARD 0x00800000
#define FE_TONEAREST 0x00000000
#define FE_TOWARDZERO 0x00C00000
#define FE_UPWARD 0x00400000

#define FE_DFL_ENV (__codex_fe_dfl_env())

int feclearexcept(int excepts);
int fegetexceptflag(fexcept_t *flagp, int excepts);
int feraiseexcept(int excepts);
int fesetexceptflag(const fexcept_t *flagp, int excepts);
int fetestexcept(int excepts);
int fegetround(void);
int fesetround(int round);
int fegetenv(fenv_t *envp);
int feholdexcept(fenv_t *envp);
int fesetenv(const fenv_t *envp);
int feupdateenv(const fenv_t *envp);

#endif
