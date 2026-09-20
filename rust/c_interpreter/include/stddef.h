#ifndef __CBOXES_STDDEF_H
#define __CBOXES_STDDEF_H

#ifndef __CBOXES_PTRDIFF_T_DEFINED
#define __CBOXES_PTRDIFF_T_DEFINED
typedef long ptrdiff_t;
#endif

#ifndef __CBOXES_SIZE_T_DEFINED
#define __CBOXES_SIZE_T_DEFINED
typedef unsigned long size_t;
#endif

#ifndef __CBOXES_WCHAR_T_DEFINED
#define __CBOXES_WCHAR_T_DEFINED
typedef int wchar_t;
#endif

typedef long double max_align_t;

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
