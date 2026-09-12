use super::*;

#[test]
fn casts_to_void_evaluate_the_operand_and_discard_any_value_type() {
    let source = r#"
            struct Pair { int first; int second; };

            void increment(int *value) {
                ++*value;
            }

            int main(void) {
                int value = 1;
                int *pointer = &value;
                struct Pair pair = {2, 3};
                (void)(value += 4);
                (void)pointer;
                (void)pair;
                (void)increment(&value);
                return value != 6;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn dereference_after_block_exit_is_ub() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int *p = 0;
                {
                    int x = 1;
                    p = &x;
                }
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn conditional_pointer_merges_qualifiers_regardless_of_operand_order() {
    let source = r#"
            int main(void) {
                int x = 0;
                const int *cp = &x;
                int *p = &x;
                int *q = 1 ? p : cp;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert const int* to int*");
}

#[test]
fn unresolved_linkage_use_in_constant_sizeof_operand_needs_no_definition() {
    let source = r#"
            extern int missing;
            int main(void) {
                return sizeof(missing + 1) != sizeof(int);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn run_result_preserves_exit_function_status() {
    let source = r#"
            #include <stdlib.h>
            int main(void) {
                exit(3);
            }
        "#;
    assert_exit_status(source, 3);
}

#[test]
fn non_dynamic_allocation_budget_is_released_at_scope_exit() {
    let source = r#"
            static void use_limit(void) {
                unsigned char bytes[1024];
                bytes[0] = 1;
            }
            int main(void) {
                use_limit();
                use_limit();
                return 0;
            }
        "#;
    let options = RunOptions {
        allocation_limit_bytes: Some(1100),
        ..RunOptions::default()
    };
    let result = run_source_with_options("test.c", source, &options).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn strlen_of_an_unterminated_array_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[3] = {'a', 'b', 'c'};
                return (int)strlen(text);
            }
        "#;
    assert_diagnostic_contains(source, "not terminated");
}

#[test]
fn strcpy_copies_a_string() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char dest[8];
                strcpy(dest, "hi");
                printf("%s\n", &dest[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "hi\n");
}

#[test]
fn strcpy_overlap_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[8] = "hello";
                strcpy(&text[1], text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn string_locale_and_error_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char a[64];
                char b[64];
                unsigned long na = strxfrm(a, "abc", sizeof a);
                unsigned long nb = strxfrm(b, "abc", sizeof b);
                char *msg = strerror(0);
                printf("%d %d %d %d\n",
                    strcoll("abc", "abc") == 0,
                    na == nb,
                    strcmp(a, b) == 0,
                    msg != 0);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1\n");
}

#[test]
fn strerror_result_is_read_only() {
    let source = r#"
            #include <string.h>

            int main(void) {
                *strerror(0) = 'x';
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "read-only");
}

#[test]
fn strerror_result_is_invalid_after_subsequent_strerror_call() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char *first = strerror(0);
                strerror(1);
                return first[0];
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(
        rendered.contains("lifetime has ended")
            || rendered.contains("indeterminate pointer")
            || rendered.contains("became indeterminate")
    );
}

#[test]
fn copied_strerror_pointer_is_invalid_after_subsequent_strerror_call() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char *first = strerror(0);
                char *copy = first;
                strerror(1);
                char *again = copy;
                return again != 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(
        rendered.contains("lifetime has ended")
            || rendered.contains("indeterminate pointer")
            || rendered.contains("became indeterminate")
    );
}

#[test]
fn strcat_overlap_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[8] = "ab";
                strcat(&text[1], text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn strncat_overlap_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[8] = "ab";
                strncat(text, &text[1], 2);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn strxfrm_overlap_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[16] = "abc";
                strxfrm(text, text, sizeof text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn strtok_mutates_and_tracks_state() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char text[] = "a,b,,c";
                char *t1 = strtok(text, ",");
                char *t2 = strtok(0, ",");
                char *t3 = strtok(0, ",");
                char *t4 = strtok(0, ",");
                printf("%s %s %s %d %s\n", t1, t2, t3, t4 == 0, &text[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "a b c 1 a\n");
}

#[test]
fn strtok_state_after_block_exit_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                {
                    char text[] = "a,b";
                    strtok(text, ",");
                }
                strtok(0, ",");
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn strtok_overlapping_source_and_delimiter_is_ub_when_write_overlaps_delimiter_bytes() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[] = "x,y";
                strtok(text, &text[1]);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn strtok_with_separate_delimiter_buffer_is_allowed() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char text[] = "x,y";
                char delim[] = ",y";
                char *token = strtok(text, delim);
                printf("%s\n", token);
                return 0;
            }
        "#;
    assert_stdout(source, "x\n");
}

#[test]
fn strtok_writing_string_literal_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char *text = "a,b";
                strtok(text, ",");
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "read-only");
}

#[test]
fn strtok_state_after_free_is_ub() {
    let source = r#"
            #include <stdlib.h>
            #include <string.h>

            int main(void) {
                char *text = (char *)malloc(4);
                text[0] = 'a';
                text[1] = ',';
                text[2] = 'b';
                text[3] = 0;
                strtok(text, ",");
                free(text);
                strtok(0, ",");
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("lifetime has ended") || rendered.contains("indeterminate pointer"));
}

#[test]
fn rand_state_is_repeatable_and_private_to_each_execution() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                int default_first = rand();
                srand(7);
                int seeded_first = rand();
                int seeded_second = rand();
                srand(7);
                printf("%d %d %d %d\n", default_first, seeded_first,
                       seeded_second, seeded_first == rand());
                return 0;
            }
        "#;
    let first = run_source("first.c", source).unwrap().stdout;
    let second = run_source("second.c", source).unwrap().stdout;
    assert_eq!(first, second);
    assert!(first.ends_with(" 1\n"), "{first}");
}

#[test]
fn atexit_handlers_run_in_reverse_registration_order() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            void first(void) {
                printf("1");
            }

            void second(void) {
                printf("2");
            }

            int main(void) {
                atexit(first);
                atexit(second);
                exit(0);
            }
        "#;
    assert_stdout(source, "21");
}

#[test]
fn returning_from_main_runs_atexit_handlers() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            void first(void) {
                printf("1");
            }

            void second(void) {
                printf("2");
            }

            int main(void) {
                atexit(first);
                atexit(second);
                printf("M");
                return 7;
            }
        "#;
    let output = run_source("test.c", source).unwrap();
    assert_eq!(output.stdout, "M21");
    assert_eq!(output.exit_status, 7);
}

#[test]
fn quick_exit_from_atexit_handler_runs_quick_exit_handlers() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            void quick_handler(void) {
                printf("Q");
            }

            void normal_handler(void) {
                printf("A");
                quick_exit(9);
            }

            int main(void) {
                at_quick_exit(quick_handler);
                atexit(normal_handler);
                return 0;
            }
        "#;
    let output = run_source("test.c", source).unwrap();
    assert_eq!(output.stdout, "AQ");
    assert_eq!(output.exit_status, 9);
}

#[test]
fn exit_from_atexit_handler_is_ub() {
    let source = r#"
            #include <stdlib.h>

            void again(void) {
                exit(0);
            }

            int main(void) {
                atexit(again);
                exit(0);
            }
        "#;
    assert_diagnostic_contains(source, "atexit handler");
}

#[test]
fn qsort_and_bsearch_work_with_user_comparator() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int cmp(const void *lhs, const void *rhs) {
                int a = *(const int *)lhs;
                int b = *(const int *)rhs;
                return (a > b) - (a < b);
            }

            int main(void) {
                int values[4] = { 3, 1, 4, 2 };
                int key = 3;
                qsort(&values, 4, sizeof(int), cmp);
                int *found = bsearch(&key, &values, 4, sizeof(int), cmp);
                printf("%d %d %d %d %d\n",
                    values[0], values[1], values[2], values[3],
                    found != 0 ? *found : -1);
                return 0;
            }
        "#;
    assert_stdout(source, "1 2 3 4 3\n");
}

#[test]
fn qsort_comparator_modifying_array_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int cmp(const void *lhs, const void *rhs) {
                *(int *)lhs = 99;
                return 0;
            }

            int main(void) {
                int values[2] = { 1, 2 };
                qsort(&values, 2, sizeof(int), cmp);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "comparison function modified");
}

#[test]
fn stdlib_multibyte_functions_work_for_basic_ascii() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                wchar_t wc = 0;
                wchar_t wide[4];
                char text[] = "Hi";
                char out[4] = {0, 0, 0, 0};
                int len = mblen("A", 1);
                int one = mbtowc(&wc, "A", 1);
                int two = wctomb(out, wc);
                size_t wn = mbstowcs(wide, text, 4);
                size_t bn = wcstombs(out, wide, 4);
                printf("%d %d %d %d %lu %lu %d %d\n",
                    len, one, two, MB_CUR_MAX >= 1,
                    wn, bn, wc == 'A', out[0] == 'H' && out[1] == 'i');
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1 2 2 1 1\n");
}

#[test]
fn locale_header_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <locale.h>

            int main(void) {
                char *name = setlocale(LC_ALL, 0);
                struct lconv *lc = localeconv();
                printf("%d %d %d\n", name != 0, lc != 0, lc->decimal_point != 0);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1\n");
}

#[test]
fn localeconv_result_remains_live_after_setlocale() {
    let source = r#"
            #include <locale.h>

            int main(void) {
                struct lconv *lc = localeconv();
                setlocale(LC_ALL, 0);
                return lc->decimal_point[0] != '.';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn copied_localeconv_string_remains_live_after_setlocale() {
    let source = r#"
            #include <locale.h>

            int main(void) {
                char *decimal = localeconv()->decimal_point;
                setlocale(LC_ALL, 0);
                return decimal[0] != '.';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn wctype_header_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <wctype.h>

            int main(void) {
                wctype_t alpha = wctype("alpha");
                wctrans_t lower = wctrans("tolower");
                printf("%d %d %d %d\n",
                    iswctype('A', alpha),
                    towctrans('A', lower) == 'a',
                    iswspace(' '),
                    WEOF == -1);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1\n");
}

#[test]
fn stale_wctype_descriptor_after_setlocale_is_ub() {
    let source = r#"
            #include <locale.h>
            #include <wctype.h>

            int main(void) {
                wctype_t alpha = wctype("alpha");
                setlocale(LC_ALL, "C");
                return iswctype('A', alpha);
            }
        "#;
    assert_diagnostic_contains(source, "locale-stale");
}

#[test]
fn time_header_functions_and_sizes_work() {
    let source = r#"
            #include <stdio.h>
            #include <time.h>

            int main(void) {
                time_t t = time(0);
                struct tm *tm = localtime(&t);
                printf("%lu %lu %lu %d %d\n",
                    sizeof(time_t),
                    sizeof(clock_t),
                    sizeof(struct tm),
                    CLOCKS_PER_SEC > 0,
                    tm != 0);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        format!(
            "{} {} {} 1 1\n",
            std::mem::size_of::<i64>(),
            std::mem::size_of::<u64>(),
            56
        )
    );
}

#[test]
fn wchar_header_and_wide_literals_work() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                wchar_t text[] = L"hello";
                printf("%lu %d %d\n", wcslen(text), text[1], L'Z');
                return 0;
            }
        "#;
    assert_stdout(source, "5 101 90\n");
}

#[test]
fn basic_wide_string_search_and_tokenization_work() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                wchar_t text[] = L"a,b,c";
                wchar_t delim[] = L",";
                wchar_t *state = 0;
                wchar_t *tok1 = wcstok(text, delim, &state);
                wchar_t *tok2 = wcstok((wchar_t *)0, delim, &state);
                wchar_t *hit = wcschr(tok2, L'b');
                wchar_t *tail = wcsstr(state, L"c");
                printf("%d %d %d %d\n", tok1[0], tok2[0], hit[0], tail[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "97 98 98 99\n");
}

#[test]
fn wcstod_sets_end_pointer() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                wchar_t *end = 0;
                double x = wcstod(L"12.5x", &end);
                printf("%d %d\n", (int)(x * 10.0), *end);
                return 0;
            }
        "#;
    assert_stdout(source, "125 120\n");
}

#[test]
fn mbsrtowcs_and_wcsrtombs_round_trip_and_update_source_pointers() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                const char *src = "hi";
                const char *p = src;
                wchar_t wide[8];
                mbstate_t st = {0};
                size_t wn = mbsrtowcs(wide, &p, 8, &st);

                const wchar_t *wp = wide;
                char bytes[8];
                mbstate_t st2 = {0};
                size_t bn = wcsrtombs(bytes, &wp, 8, &st2);

                printf("%lu %d %lu %d %s\n", wn, p == 0, bn, wp == 0, bytes);
                return 0;
            }
        "#;
    assert_stdout(source, "2 1 2 1 hi\n");
}

#[test]
fn wcscpy_overlap_is_ub() {
    let source = r#"
            #include <wchar.h>

            int main(void) {
                wchar_t text[8] = L"ab";
                wcscpy(&text[1], text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn wide_copy_and_concat_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                wchar_t a[16] = L"ab";
                wchar_t b[16];
                wcscpy(b, a);
                wcsncat(b, L"cd", 1);
                printf("%d %d %lu\n", b[0], b[2], wcslen(b));
                return 0;
            }
        "#;
    assert_stdout(source, "97 99 3\n");
}

#[test]
fn wcsftime_formats_into_wide_buffer() {
    let source = r#"
            #include <stdio.h>
            #include <time.h>
            #include <wchar.h>

            int main(void) {
                time_t t = 0;
                struct tm *tm = gmtime(&t);
                wchar_t buf[32];
                unsigned long n = wcsftime(buf, 32, L"%Y", tm);
                printf("%lu %d %d\n", n, buf[0], buf[3]);
                return 0;
            }
        "#;
    assert_stdout(source, "4 49 48\n");
}

#[test]
fn wide_string_numeric_escapes_use_unsigned_wchar_range() {
    let source = r#"
            #include <wchar.h>
            int main(void) {
                wchar_t text[] = L"\xFFFFFFFF";
                return sizeof text != 2 * sizeof(wchar_t) || text[0] != (wchar_t)-1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn locale_dependent_library_state_is_invalidated() {
    let source = r#"
            #include <locale.h>
            #include <wchar.h>
            int main(void) {
                mbstate_t state = {0};
                (void)mbrlen("A", 1, &state);
                setlocale(LC_CTYPE, "C");
                return (int)mbrlen("A", 1, &state);
            }
        "#;
    let rendered = run_source("test.c", source).unwrap_err().render();
    assert_user_code_library_diag(&rendered, "locale change");
}

#[test]
fn reported_locale_result_pointers_keep_live_referents() {
    let source = r#"
            #include <locale.h>

            int main(void) {
                char *first_name = setlocale(LC_ALL, "C");
                char *second_name = setlocale(LC_ALL, (const char *)0);
                struct lconv *first = localeconv();
                struct lconv *second = localeconv();
                setlocale(LC_CTYPE, "C");
                if (first_name == 0 || second_name == 0 || first == 0 || second == 0)
                    return 1;
                return first->decimal_point[0] != '.';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn reported_localeconv_object_is_read_only() {
    let source = r#"
            #include <locale.h>
            int main(void) {
                localeconv()->frac_digits = 0;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "read-only");
}
