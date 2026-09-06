#undef static_assert
#define static_assert _Static_assert

#undef assert
#ifdef NDEBUG
#define assert(ignore) ((void)0)
#else
void __codex_assert_fail(const char *expression, const char *file, int line, const char *function);
#define assert(expr) ((expr) ? (void)0 : __codex_assert_fail(#expr, __FILE__, __LINE__, __func__))
#endif
