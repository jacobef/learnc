#ifndef __CODEX_STDDEF_H
#define __CODEX_STDDEF_H

#ifndef __CODEX_PTRDIFF_T_DEFINED
#define __CODEX_PTRDIFF_T_DEFINED
typedef long ptrdiff_t;
#endif

#ifndef __CODEX_SIZE_T_DEFINED
#define __CODEX_SIZE_T_DEFINED
typedef unsigned long size_t;
#endif

#ifndef __CODEX_WCHAR_T_DEFINED
#define __CODEX_WCHAR_T_DEFINED
typedef int wchar_t;
#endif

#ifndef NULL
#define NULL ((void *)0)
#endif

#define PTRDIFF_MIN (-9223372036854775807L - 1)
#define PTRDIFF_MAX 9223372036854775807L
#define SIZE_MAX 18446744073709551615UL
#define WCHAR_MIN (-2147483647 - 1)
#define WCHAR_MAX 2147483647

#define offsetof(type, member) __builtin_offsetof(type, member)

#endif
