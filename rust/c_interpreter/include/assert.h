#ifndef __CODEX_ASSERT_H
#define __CODEX_ASSERT_H

#define static_assert _Static_assert

#ifdef NDEBUG
#define assert(ignore) ((void)0)
#else
void __codex_assert_fail(const char *expression, const char *file, int line);
#define assert(expr) ((expr) ? (void)0 : __codex_assert_fail(#expr, __FILE__, __LINE__))
#endif

#endif
