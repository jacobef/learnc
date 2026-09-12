use super::*;

#[test]
fn printf_rejects_indeterminate_int_even_after_address_taken() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a;
                &a;
                printf("%d\n", a);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("indeterminate"));
    assert!(rendered.contains("DR 451"));
}

#[test]
fn printf_percent_p_requires_void_pointer() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int *p = &x;
                printf("%p\n", p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "%p");
}

#[test]
fn printf_percent_p_accepts_void_pointer_after_cast() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                printf("%p\n", (void *)&x);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert!(result.stdout.starts_with("0x"));
}

#[test]
fn unsigned_comparisons_and_printf_lengths_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned int u = (unsigned int)-1;
                long l = 2147483647;
                l = l + 1;
                unsigned long ul = (unsigned long)-1;
                if (u == (unsigned int)-1 && u > 0 && l > 2147483647) {
                    printf("%u %ld %lu\n", u, l, ul);
                }
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        "4294967295 2147483648 18446744073709551615\n"
    );
}

#[test]
fn printf_rejects_sizeof_with_percent_d() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%d\n", sizeof(int));
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "%d");
}

#[test]
fn printf_percent_f_requires_double_after_default_argument_promotions() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%f\n", 1);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "requires an argument of type double");
}

#[test]
fn printf_percent_lf_requires_long_double() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%Lf\n", 1.0);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "requires an argument of type long double");
}

#[test]
fn printf_percent_p_uses_synthetic_addresses_not_object_ids() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a;
                int b;
                printf("%p %p\n", (void *)&a, (void *)&b);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    let parts = result.stdout.trim().split(' ').collect::<Vec<_>>();
    assert_eq!(parts.len(), 2);
    let first = u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).unwrap();
    let second = u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap();
    assert!(first >= 0x1000);
    assert_eq!(second - first, 8);
}

#[test]
fn printf_percent_p_reflects_subobject_offsets_within_an_array() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a[2];
                printf("%p %p\n", (void *)&a[0], (void *)&a[1]);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    let parts = result.stdout.trim().split(' ').collect::<Vec<_>>();
    assert_eq!(parts.len(), 2);
    let first = u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).unwrap();
    let second = u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap();
    assert_eq!(second - first, 4);
}

#[test]
fn printf_percent_s_accepts_const_char_pointers() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                const char *msg = "ok";
                printf("%s\n", msg);
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn printf_percent_s_accepts_all_character_types_and_const_qualification() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char plain[] = "plain";
                const signed char signed_text[] = {'s', 'i', 'g', 'n', 'e', 'd', 0};
                const unsigned char unsigned_text[] = {
                    'u', 'n', 's', 'i', 'g', 'n', 'e', 'd', 0
                };
                printf("%s %s %s\n", plain, signed_text, unsigned_text);
                return 0;
            }
        "#;
    assert_stdout(source, "plain signed unsigned\n");
}

#[test]
fn printf_percent_s_rejects_pointers_to_non_character_arrays() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned short text[] = {'n', 'o', 0};
                printf("%s\n", text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "requires a pointer to a character type");
}

#[test]
fn printf_percent_s_rejects_character_pointer_casts_into_non_character_objects() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int storage = 0;
                printf("%.1s\n", (unsigned char *)&storage);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not point into an array of character type");
}

#[test]
fn printf_percent_ls_accepts_qualified_wchar_pointers_only() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>
            int main(void) {
                const wchar_t *text = L"wide";
                printf("%ls\n", text);
                return 0;
            }
        "#;
    assert_stdout(source, "wide\n");

    let wrong_type = r#"
            #include <stdio.h>
            int main(void) {
                const unsigned int text[] = {'n', 'o', 0};
                printf("%ls\n", text);
                return 0;
            }
        "#;
    assert_diagnostic_contains(wrong_type, "requires an argument of type wchar_t *");
}

#[test]
fn snprintf_truncates_but_returns_full_length() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char buf[4];
                int n = snprintf(buf, sizeof(buf), "%d%d", 12, 34);
                printf("%d %s\n", n, &buf[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "4 123\n");
}

#[test]
fn formatted_output_rejects_overlapping_inputs_and_percent_n_outputs() {
    for source in [
        r#"
                #include <stdio.h>
                int main(void) {
                    char text[16] = "abc";
                    sprintf(text, "%s", text);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                int main(void) {
                    char text[16] = "x";
                    sprintf(text, text);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                int main(void) {
                    int words[8] = {0};
                    sprintf((char *)words, "abc%n", &words[0]);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                int main(void) {
                    int format_words[8] = {0};
                    char *format = (char *)format_words;
                    char output[8];
                    format[0] = 'x';
                    format[1] = '%';
                    format[2] = 'n';
                    format[3] = 0;
                    sprintf(output, format, &format_words[0]);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                #include <wchar.h>
                int main(void) {
                    wchar_t text[16] = L"abc";
                    swprintf(text, 16, L"%ls", text);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                #include <stdarg.h>
                int render(char *dest, unsigned long n, const char *format, ...) {
                    va_list args;
                    va_start(args, format);
                    int result = vsnprintf(dest, n, format, args);
                    va_end(args);
                    return result;
                }
                int main(void) {
                    char text[16] = "abc";
                    render(text, sizeof text, "%s", text);
                    return 0;
                }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains("overlaps"), "{}", err.render());
    }
}

#[test]
fn wide_printf_precision_stops_at_a_terminator_before_the_object_bound() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>
            int main(void) {
                wprintf(L"%.10ls\n", L"x");
                return 0;
            }
        "#;
    assert_stdout(source, "x\n");
}

#[test]
fn tmpfile_round_trips_text_through_fputs_rewind_and_fgets() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                FILE *f = tmpfile();
                char buf[8];
                fputs("abc\n", f);
                rewind(f);
                fgets(buf, sizeof(buf), f);
                printf("%s", &buf[0]);
                fclose(f);
                return 0;
            }
        "#;
    assert_stdout(source, "abc\n");
}

#[test]
fn getc_and_ungetc_work_on_tmpfile_streams() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                FILE *f = tmpfile();
                fputs("ab", f);
                rewind(f);
                int a = getc(f);
                ungetc(a, f);
                printf("%c%c\n", getc(f), getc(f));
                fclose(f);
                return 0;
            }
        "#;
    assert_stdout(source, "ab\n");
}

#[test]
fn perror_writes_to_captured_stderr() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                perror("tag");
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert!(result.stderr.starts_with("tag: "));
    assert!(result.stderr.ends_with('\n'));
}

#[test]
fn vprintf_consumes_interpreter_va_lists() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>
            int log_line(const char *fmt, ...) {
                va_list ap;
                va_start(ap, fmt);
                int n = vprintf(fmt, ap);
                va_end(ap);
                return n;
            }
            int main(void) {
                log_line("%d %s\n", 5, "ok");
                return 0;
            }
        "#;
    assert_stdout(source, "5 ok\n");
}

#[test]
fn vsnprintf_formats_through_interpreter_va_lists() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>
            int build(char *buf, unsigned long n, const char *fmt, ...) {
                va_list ap;
                va_start(ap, fmt);
                int out = vsnprintf(buf, n, fmt, ap);
                va_end(ap);
                return out;
            }
            int main(void) {
                char buf[5];
                printf("%d %s\n", build(buf, sizeof(buf), "%d%s", 12, "xy"), buf);
                return 0;
            }
        "#;
    assert_stdout(source, "4 12xy\n");
}

#[test]
fn printf_supports_standard_integer_length_modifiers() {
    let source = r#"
            #include <stdio.h>
            #include <stddef.h>
            #include <stdint.h>
            int main(void) {
                size_t z = 3;
                intmax_t j = -4;
                ptrdiff_t t = 5;
                signed char hh = -6;
                unsigned short h = 7;
                printf("%zu %jd %td %hhd %ho\n", z, j, t, hh, h);
                return 0;
            }
        "#;
    assert_stdout(source, "3 -4 5 -6 7\n");
}

#[test]
fn printf_allows_representable_corresponding_signed_and_unsigned_arguments() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned char byte = 0xab;
                unsigned int unsigned_value = 42;
                printf("%02X %u %d\n", byte, byte, unsigned_value);
                return 0;
            }
        "#;
    assert_stdout(source, "AB 171 42\n");
}

#[test]
fn printf_rejects_unrepresentable_corresponding_signed_and_unsigned_arguments() {
    assert_diagnostic_contains(
        r#"
            #include <limits.h>
            #include <stdio.h>
            int main(void) {
                unsigned int value = UINT_MAX;
                printf("%d\n", value);
                return 0;
            }
        "#,
        "requires an argument of type int",
    );
    assert_diagnostic_contains(
        r#"
            #include <stdio.h>
            int main(void) {
                int value = -1;
                printf("%u\n", value);
                return 0;
            }
        "#,
        "requires an argument of type unsigned int",
    );
}

#[test]
fn printf_rejects_wrong_type_for_size_t_length_modifier() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned int x = 3;
                printf("%zu\n", x);
                return 0;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "printf %zu requires an argument of type unsigned long",
    );
}

#[test]
fn printf_supports_flags_width_precision_and_percent_n() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = -1;
                printf("[%05d][%-6.3s][%*.*d][%nX]\n", 12, "zebra", 6, 4, 7, &n);
                printf("%d\n", n);
                return 0;
            }
        "#;
    assert_stdout(source, "[00012][zeb   ][  0007][X]\n24\n");
}

#[test]
fn printf_allows_extra_variadic_arguments() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%d\n", 1, 2, 3);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn scanf_allows_extra_variadic_arguments() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int y = 0;
                sscanf("7", "%d", &x, &y);
                printf("%d %d\n", x, y);
                return 0;
            }
        "#;
    assert_stdout(source, "7 0\n");
}

#[test]
fn printf_precision_can_bound_a_non_terminated_char_array() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char text[3] = {'a', 'b', 'c'};
                printf("%.2s\n", &text[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "ab\n");
}

#[test]
fn printf_supports_wide_character_and_string_output() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>
            int main(void) {
                wchar_t text[] = L"hi";
                printf("%lc %ls\n", L'Z', &text[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "Z hi\n");
}

#[test]
fn printf_rejects_field_width_with_percent_n() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 0;
                printf("%5n\n", &n);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "uses invalid flags, width, or precision");
}

#[test]
fn explicitly_declared_printf_works_without_including_stdio() {
    let source = r#"
            extern int printf(const char *, ...);
            int main(void) {
                printf("ok\n");
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn printf_can_be_called_through_a_function_pointer() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int (*fp)(const char *fmt, ...) = printf;
                fp("%d %d\n", 7, 9);
                return 0;
            }
        "#;
    assert_stdout(source, "7 9\n");
}

#[test]
fn printf_ub_diagnostics_point_to_user_code() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                printf("%d\n", 1.5);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert_user_code_library_diag(&rendered, "printf %d requires an argument of type int");
}

#[test]
fn printf_requires_stdio_header() {
    let source = r#"
            int main(void) {
                printf("%d\n", 1);
                return 0;
            }

        "#;
    assert_diagnostic_contains(source, "use of undeclared identifier printf");
}

#[test]
fn puts_appends_a_trailing_newline() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                puts("hello");
                return 0;
            }
        "#;
    assert_stdout(source, "hello\n");
}

#[test]
fn wide_stream_io_round_trips() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                FILE *f = tmpfile();
                fputwc(L'A', f);
                fputws(L"BC\n", f);
                rewind(f);
                wchar_t line[8];
                int first = fgetwc(f);
                fgetws(line, 8, f);
                printf("%d %d %d\n", first, line[0], line[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "65 66 10\n");
}

#[test]
fn swscanf_parses_wide_integer_string_and_n() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                int x = 0;
                int n = 0;
                wchar_t word[8];
                int count = swscanf(L"42 zebra", L"%d %ls%n", &x, word, &n);
                printf("%d %d %d %d\n", count, x, word[0], n);
                return 0;
            }
        "#;
    assert_stdout(source, "2 42 122 8\n");
}

#[test]
fn fwscanf_reads_wide_data_from_stream() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                FILE *f = tmpfile();
                fputws(L"7 Q\n", f);
                rewind(f);
                int x = 0;
                wchar_t ch = 0;
                int count = fwscanf(f, L"%d %lc", &x, &ch);
                printf("%d %d %d\n", count, x, ch);
                return 0;
            }
        "#;
    assert_stdout(source, "2 7 81\n");
}

#[test]
fn fwscanf_restores_unread_stream_input_in_source_order() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                FILE *f = tmpfile();
                fputws(L"123abc\n", f);
                rewind(f);
                int value = 0;
                int count = fwscanf(f, L"%d", &value);
                wint_t first = fgetwc(f);
                wint_t second = fgetwc(f);
                printf("%d %d %d %d\n", count, value, (int) first, (int) second);
                return 0;
            }
        "#;
    assert_stdout(source, "1 123 97 98\n");
}

#[test]
fn vswscanf_consumes_interpreter_va_lists() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>
            #include <wchar.h>

            int parse(const wchar_t *src, const wchar_t *fmt, ...) {
                va_list ap;
                va_start(ap, fmt);
                int out = vswscanf(src, fmt, ap);
                va_end(ap);
                return out;
            }

            int main(void) {
                int x = 0;
                wchar_t word[8];
                int count = parse(L"15 ok", L"%d %ls", &x, word);
                printf("%d %d %d\n", count, x, word[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "2 15 107\n");
}

#[test]
fn swscanf_matching_failure_returns_zero() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                int x = 123;
                int count = swscanf(L"x", L"%d", &x);
                printf("%d %d\n", count, x);
                return 0;
            }
        "#;
    assert_stdout(source, "0 123\n");
}

#[test]
fn sscanf_parses_integer_string_and_n() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                int x = 0;
                int n = 0;
                char word[8];
                int count = sscanf("42 zebra", "%d %s%n", &x, word, &n);
                printf("%d %d %c %d\n", count, x, word[0], n);
                return 0;
            }
        "#;
    assert_stdout(source, "2 42 z 8\n");
}

#[test]
fn fscanf_reads_from_stream() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                fputs("7 Q\n", f);
                rewind(f);
                int x = 0;
                char ch = 0;
                int count = fscanf(f, "%d %c", &x, &ch);
                printf("%d %d %d\n", count, x, ch);
                return 0;
            }
        "#;
    assert_stdout(source, "2 7 81\n");
}

#[test]
fn fscanf_restores_unread_stream_input_in_source_order() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                fputs("123abc\n", f);
                rewind(f);
                int value = 0;
                int count = fscanf(f, "%d", &value);
                int first = fgetc(f);
                int second = fgetc(f);
                printf("%d %d %d %d\n", count, value, first, second);
                return 0;
            }
        "#;
    assert_stdout(source, "1 123 97 98\n");
}

#[test]
fn scanf_character_conversions_accept_all_character_types() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                unsigned char word[8] = {0};
                signed char byte[2] = {0};
                unsigned char set[8] = {0};
                int a = sscanf("hello", "%s", word);
                int b = sscanf("Q", "%c", byte);
                int c = sscanf("abc!", "%[abc]", set);
                printf("%d %d %d %s %d %s\n", a, b, c, word, byte[0], set);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 hello 81 abc\n");
}

#[test]
fn scanf_incomplete_floating_input_item_is_a_matching_failure() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                fputs("100er", f);
                rewind(f);
                float value = 7.0f;
                int count = fscanf(f, "%f", &value);
                printf("%d %.0f %c\n", count, value, fgetc(f));
                return 0;
            }
        "#;
    assert_stdout(source, "0 7 r\n");
}

#[test]
fn scanf_incomplete_integer_input_item_is_a_matching_failure() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *hex = tmpfile();
                fputs("0xg", hex);
                rewind(hex);
                int a = 7;
                int first = fscanf(hex, "%i", &a);

                FILE *decimal = tmpfile();
                fputs("+q", decimal);
                rewind(decimal);
                int b = 8;
                int second = fscanf(decimal, "%d", &b);

                printf("%d %d %c %d %d %c\n",
                       first, a, fgetc(hex), second, b, fgetc(decimal));
                return 0;
            }
        "#;
    assert_stdout(source, "0 7 g 0 8 q\n");
}

#[test]
fn wide_scanf_incomplete_floating_input_item_is_a_matching_failure() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                FILE *f = tmpfile();
                fputws(L"100er", f);
                rewind(f);
                float value = 7.0f;
                int count = fwscanf(f, L"%f", &value);
                printf("%d %.0f %d\n", count, value, (int)fgetwc(f));
                return 0;
            }
        "#;
    assert_stdout(source, "0 7 114\n");
}

#[test]
fn scanf_accepts_complete_standard_floating_input_forms() {
    let source = r#"
            #include <math.h>
            #include <stdio.h>

            int main(void) {
                double a, b, c, d, e, f, g;
                int count = sscanf("-1.25e+2 .5 1. 0x1.8p+1 0x1.8 infinity nan(payload)",
                                   "%lf %lf %lf %la %la %lf %lf",
                                   &a, &b, &c, &d, &e, &f, &g);
                printf("%d %.0f %.1f %.0f %.0f %.1f %d %d\n",
                       count, a, b, c, d, e, isinf(f), isnan(g));
                return 0;
            }
        "#;
    assert_stdout(source, "7 -125 0.5 1 3 1.5 1 1\n");
}

#[test]
fn sscanf_converts_multibyte_input_to_wide_outputs() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>

            int main(void) {
                wchar_t ch = 0;
                wchar_t word[8];
                int count = sscanf("A ok", "%lc %ls", &ch, word);
                printf("%d %d %d\n", count, ch, word[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "2 65 107\n");
}

#[test]
fn vsscanf_consumes_interpreter_va_lists() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>

            int parse(const char *src, const char *fmt, ...) {
                va_list ap;
                va_start(ap, fmt);
                int out = vsscanf(src, fmt, ap);
                va_end(ap);
                return out;
            }

            int main(void) {
                int x = 0;
                char word[8];
                int count = parse("15 ok", "%d %s", &x, word);
                printf("%d %d %c\n", count, x, word[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "2 15 k\n");
}

#[test]
fn fflush_on_input_stream_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                fflush(stdin);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "fflush is undefined for an input stream");
}

#[test]
fn update_stream_output_then_input_without_intervening_sync_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                fputc('x', f);
                return fgetc(f);
            }
        "#;
    assert_diagnostic_contains(
        source,
        "input on an update stream requires an intervening fflush or file-positioning call after output",
    );
}

#[test]
fn update_stream_input_then_output_without_intervening_positioning_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                fputc('x', f);
                rewind(f);
                fgetc(f);
                fputc('y', f);
                return 0;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "output on an update stream requires an intervening file-positioning call after input",
    );
}

#[test]
fn setbuf_supplied_buffer_contents_become_ub_to_use() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                char buf[BUFSIZ];
                setbuf(f, buf);
                return buf[0];
            }
        "#;
    assert_diagnostic_contains(source, "array supplied to setbuf or setvbuf");
}

#[test]
fn setvbuf_validates_the_requested_size_instead_of_bufsiz() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                char byte[1];
                int configured = setvbuf(f, byte, _IOFBF, sizeof byte);
                int closed = fclose(f);
                printf("%d %d\n", configured, closed);
                return 0;
            }
        "#;
    assert_stdout(source, "0 0\n");
}

#[test]
fn unsuccessful_setvbuf_does_not_inspect_or_reserve_the_buffer() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                char byte[1];
                int configured = setvbuf(f, byte, 12345, BUFSIZ);
                byte[0] = 'x';
                fclose(f);
                printf("%d %c\n", configured, byte[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "1 x\n");
}

#[test]
fn setvbuf_after_another_stream_operation_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                char byte[1];
                fputc('x', f);
                setvbuf(f, byte, _IOFBF, sizeof byte);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "before any other operation");
}

#[test]
fn a_second_successful_stream_buffer_configuration_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = tmpfile();
                setvbuf(f, 0, _IONBF, 0);
                setvbuf(f, 0, _IONBF, 0);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "before any other operation");
}

#[test]
fn a_supplied_stream_buffer_must_outlive_the_stream() {
    let source = r#"
            #include <stdio.h>

            FILE *stream;

            void configure(void) {
                char buffer[BUFSIZ];
                setbuf(stream, buffer);
            }

            int main(void) {
                stream = tmpfile();
                configure();
                fclose(stream);
                return 0;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("supplied buffer's lifetime has ended")
    );
}

#[test]
fn ended_stream_buffer_lifetime_is_detected_without_another_stream_call() {
    let source = r#"
            #include <stdio.h>

            FILE *stream;

            void configure(void) {
                char buffer[BUFSIZ];
                setbuf(stream, buffer);
            }

            int main(void) {
                stream = tmpfile();
                configure();
                return 0;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("supplied buffer's lifetime has ended")
    );
}

#[test]
fn freeing_a_supplied_stream_buffer_is_ub_without_a_later_stream_call() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                FILE *stream = tmpfile();
                char *buffer = malloc(BUFSIZ);
                setbuf(stream, buffer);
                free(buffer);
                return 0;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("supplied buffer's lifetime has ended")
    );
}

#[test]
fn stream_buffer_reservation_does_not_poison_sibling_subobjects() {
    let source = r#"
            #include <stdio.h>

            struct Holder {
                char buffer[BUFSIZ];
                int sibling;
            };

            int main(void) {
                FILE *f = tmpfile();
                struct Holder holder = {{0}, 0};
                setbuf(f, holder.buffer);
                holder.sibling = 37;
                printf("%d\n", holder.sibling);
                fclose(f);
                return 0;
            }
        "#;
    assert_stdout(source, "37\n");
}

#[test]
fn virtual_filesystem_supports_nested_files_rename_append_and_remove() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = fopen("/notes/week/one.txt", "w");
                fputs("ab", f);
                fclose(f);

                rename("/notes/week/one.txt", "/notes/week/two.txt");
                f = fopen("/notes/week/two.txt", "a+");
                fputs("c", f);
                rewind(f);

                char text[4] = {0};
                fread(text, 1, 3, f);
                fclose(f);

                int removed_file = remove("/notes/week/two.txt");
                int removed_week = remove("/notes/week");
                int removed_notes = remove("/notes");
                printf("%s %d %d %d\n", text, removed_file, removed_week, removed_notes);
            }
        "#;
    assert_stdout(source, "abc 0 0 0\n");
}

#[test]
fn zero_sized_fread_and_fwrite_return_zero() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char byte = 0;
                return fread(&byte, 0, 7, stdin) != 0
                    || fwrite(&byte, 0, 7, stdout) != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn fread_makes_a_partial_final_element_indeterminate() {
    let source = r#"
            #include <stdio.h>
            struct Value {
                _Bool flag;
                unsigned char rest[3];
            };

            int main(void) {
                FILE *file = tmpfile();
                unsigned char input[2] = {0, 0};
                fwrite(input, 1, 2, file);
                rewind(file);
                struct Value value = {0};
                if (fread(&value, sizeof value, 1, file) != 0) {
                    return 1;
                }
                return value.flag;
            }
        "#;
    assert_diagnostic_contains(source, "indeterminate _Bool");
}

#[test]
fn removing_an_open_virtual_file_does_not_invalidate_the_stream() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                FILE *f = fopen("open.txt", "w+");
                fputs("ok", f);
                rewind(f);
                remove("open.txt");
                int first = fgetc(f);
                int second = fgetc(f);
                printf("%c%c %d\n", first, second, fopen("open.txt", "r") == 0);
                fclose(f);
            }
        "#;
    assert_stdout(source, "ok 1\n");
}

#[test]
fn configured_stdin_is_consumed_by_standard_input_functions() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
            int value = 0;
            char word[8] = {0};
            char tail = 0;
            scanf("%d %7s %c", &value, word, &tail);
            printf("%d %s %c\n", value, word, tail);
            }
        "#;
    let result = run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            stdin: "42 hello !".to_owned(),
            ..RunOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.stdout, "42 hello !\n");
}

#[test]
fn exhausted_live_stdin_blocks_at_the_input_call() {
    let source = "#include <stdio.h>\nint main(void) {\n  int before = 7;\n  printf(\"ready\");\n  int ch = getchar();\n}\n";
    let result = run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            stdin: String::new(),
            ..RunOptions::default()
        },
    )
    .unwrap();
    let blocked = result.blocked.as_ref().expect("getchar should block");

    assert_eq!(result.stdout, "ready");
    assert_eq!(blocked.file, "test.c");
    assert_eq!(blocked.start_line, 4);
    assert_eq!(blocked.function, "getchar");
    assert!(blocked.state.iter().any(|item| item.name == "before"));
    assert!(blocked.state.iter().all(|item| item.name != "ch"));
}

#[test]
fn ctrl_d_marker_closes_stdin_and_produces_eof() {
    let source = "#include <stdio.h>\nint main(void) {\n  int ch = getchar();\n  printf(\"%d\\n\", ch);\n}\n";
    let result = run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            stdin: "\u{2404}".to_owned(),
            ..RunOptions::default()
        },
    )
    .unwrap();

    assert!(result.blocked.is_none());
    assert_eq!(result.stdout, "-1\n");
}

#[test]
fn stdin_readers_block_but_regular_files_still_reach_eof() {
    let blocking_sources = [
        (
            "fgets",
            "#include <stdio.h>\nint main(void) { char text[4]; fgets(text, 4, stdin); }\n",
        ),
        (
            "fread",
            "#include <stdio.h>\nint main(void) { char text[4]; fread(text, 1, 4, stdin); }\n",
        ),
        (
            "scanf",
            "#include <stdio.h>\nint main(void) { int value; scanf(\"%d\", &value); }\n",
        ),
        (
            "getwchar",
            "#include <stdio.h>\n#include <wchar.h>\nint main(void) { getwchar(); }\n",
        ),
    ];
    for (expected_function, source) in blocking_sources {
        let result = run_source_with_options(
            "test.c",
            source,
            &RunOptions {
                stdin: String::new(),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            result
                .blocked
                .as_ref()
                .map(|blocked| blocked.function.as_str()),
            Some(expected_function)
        );
    }

    let file_source = "#include <stdio.h>\nint main(void) {\n  FILE *file = tmpfile();\n  int ch = fgetc(file);\n  printf(\"%d\\n\", ch);\n}\n";
    let result = run_source("test.c", file_source).unwrap();
    assert!(result.blocked.is_none());
    assert_eq!(result.stdout, "-1\n");
}

#[test]
fn fopen_invalid_mode_string_is_ub() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                fopen("missing.txt", "rx");
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(
        rendered
            .contains("fopen mode string must exactly match one of the standard mode sequences")
    );
}

#[test]
fn sscanf_percent_p_requires_a_pointer_value_from_the_same_execution() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                void *p = 0;
                sscanf("0x1234", "%p", &p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "%p input must be a pointer value previously produced",
    );
}

#[test]
fn snprintf_retains_its_explicit_null_zero_size_exception() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                return snprintf((char *)0, 0, "%s", "okay") != 4;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn scanf_rejects_unrepresentable_conversion_results() {
    for source in [
        r#"
                #include <stdio.h>
                int main(void) {
                    signed char value;
                    return sscanf("128", "%hhd", &value);
                }
            "#,
        r#"
                #include <wchar.h>
                int main(void) {
                    int value;
                    return swscanf(L"2147483648", L"%d", &value);
                }
            "#,
        r#"
                #include <stdio.h>
                int main(void) {
                    long long value;
                    return sscanf("999999999999999999999999999", "%lld", &value);
                }
            "#,
    ] {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert_user_code_library_diag(&rendered, "not representable");
    }
}

#[test]
fn printf_percent_s_accepts_all_three_character_types() {
    let source = r#"
            #include <stdio.h>

            int main(void) {
                char plain[] = "plain";
                signed char signed_text[] = {'s', 'i', 'g', 'n', 'e', 'd', 0};
                unsigned char unsigned_text[] = {'u', 'n', 's', 'i', 'g', 'n', 'e', 'd', 0};
                printf("%s %s %s\n", plain, signed_text, unsigned_text);
                return 0;
            }
        "#;
    assert_stdout(source, "plain signed unsigned\n");
}
