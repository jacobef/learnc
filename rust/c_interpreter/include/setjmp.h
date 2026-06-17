#ifndef __CODEX_SETJMP_H
#define __CODEX_SETJMP_H

typedef unsigned long jmp_buf[1];

int __codex_setjmp(jmp_buf env);
#define setjmp(env) __codex_setjmp(env)

void longjmp(jmp_buf env, int val);

#endif
