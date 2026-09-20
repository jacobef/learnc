#ifndef __CBOXES_SETJMP_H
#define __CBOXES_SETJMP_H

typedef unsigned long jmp_buf[1];

int __cboxes_setjmp(jmp_buf env);
#define setjmp(env) __cboxes_setjmp(env)

void longjmp(jmp_buf env, int val);

#endif
