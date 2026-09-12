use super::*;

#[test]
fn host_abi_preserves_interpreted_long_width() {
    assert_exit_status(
        include_str!(
            "../../../tests/standard_examples/host_abi_preserves_interpreted_long_width.c"
        ),
        0,
    );
}

#[test]
fn signed_int_overflow_uses_32_bit_int_model() {
    let source = r#"
            int main(void) {
                int x = 2147483647;
                int y = x + 1;
                return y;
            }
        "#;
    assert_diagnostic_contains(source, "signed integer overflow");
}

#[test]
fn byte_copies_preserve_float_and_long_double_representations_exactly() {
    let source = r#"
            #include <string.h>
            int main(void) {
                unsigned char float_input[sizeof(float)] = {0x45, 0x23, 0xa1, 0x7f};
                unsigned char float_output[sizeof(float)] = {0};
                float floating;
                memcpy(&floating, float_input, sizeof floating);
                memcpy(float_output, &floating, sizeof floating);
                if (memcmp(float_input, float_output, sizeof floating) != 0) return 1;

                unsigned char long_input[sizeof(long double)] = {0};
                unsigned char long_output[sizeof(long double)] = {0};
                long double extended;
                long_input[sizeof long_input - 1] = 42;
                memcpy(&extended, long_input, sizeof extended);
                memcpy(long_output, &extended, sizeof extended);
                return memcmp(long_input, long_output, sizeof extended) != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn corresponding_signed_and_unsigned_types_may_alias() {
    let source = r#"
            #include <limits.h>
            int main(void) {
                int value = -1;
                unsigned int *alias = (unsigned int *)&value;
                if (*alias != UINT_MAX) {
                    return 1;
                }
                *alias = 0x80000000u;
                return value != INT_MIN;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn conditional_operator_and_char_literals_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int y = x ? 'a' : 'b';
                printf("%d\n", y);
                return 0;
            }
        "#;
    assert_stdout(source, "98\n");
}

#[test]
fn bitwise_and_shift_operators_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = (3 << 4) | 2;
                int y = (x & 14) ^ 8;
                printf("%d %d\n", x, y);
                return 0;
            }
        "#;
    assert_stdout(source, "50 10\n");
}

#[test]
fn modifying_string_literal_is_ub() {
    let source = r#"
            int main(void) {
                char *p = "hi";
                p[0] = 'x';
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "read-only");
}

#[test]
fn compound_assignment_updates_integer_object() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 3;
                x += 4;
                x <<= 1;
                x |= 3;
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "15\n");
}

#[test]
fn sizeof_does_not_evaluate_operand() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                printf("%lu %d\n", sizeof i++, i);
                return 0;
            }
        "#;
    assert_stdout(source, "4 0\n");
}

#[test]
fn sizeof_void_is_rejected() {
    let source = r#"
            int main(void) {
                return sizeof(void);
            }
        "#;
    assert_diagnostic_contains(source, "sizeof cannot determine the size of type void");
}

#[test]
fn invalid_constant_shift_is_diagnosed_without_panicking() {
    let source = "enum { BAD = 1 << 128 };\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "shift count");
}

#[test]
fn evaluated_comma_is_not_an_integer_constant_expression() {
    let source = "int main(void) { switch (2) { case (1, 2): return 0; } return 1; }\n";
    assert_diagnostic_contains(source, "comma operator");
}

#[test]
fn switch_rejects_case_values_that_duplicate_after_conversion() {
    let source = r#"
            int main(void) {
                unsigned int control = 0;
                switch (control) {
                    case -1: return 1;
                    case 4294967295U: return 2;
                }
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "duplicate case value");
}

#[test]
fn switch_does_not_conflate_distinct_signed_and_unsigned_case_values() {
    let source = r#"
            int main(void) {
                long control = 0;
                switch (control) {
                    case ~0U: return 1;
                    case -1: return 2;
                    default: return 0;
                }
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn unsigned_constant_arithmetic_wraps_without_host_overflow() {
    let source = r#"
            int main(void) {
                switch (1ULL) {
                    case 18446744073709551615ULL * 18446744073709551615ULL:
                        return 0;
                    default:
                        return 1;
                }
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn case_label_requires_integer_constant_expression() {
    let source = r#"
            int main(void) {
                int x = 1;
                switch (x) {
                    case x:
                        return 0;
                    default:
                        return 1;
                }
            }
        "#;
    assert_diagnostic_contains(source, "integer constant expression");
}

#[test]
fn host_integer_sizes_are_modeled() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%lu %lu %lu %lu %lu %lu\n",
                    sizeof(char),
                    sizeof(short),
                    sizeof(int),
                    sizeof(long),
                    sizeof(long long),
                    sizeof(unsigned long));
                return 0;
            }
        "#;
    assert_stdout(source, "1 2 4 8 8 8\n");
}

#[test]
fn unsigned_and_long_integer_operations_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned int u = (unsigned int)-1;
                unsigned long ul = (unsigned long)-1;
                if (u == -1 && ul > 0) {
                    printf("%u %lu\n", u, ul);
                }
                return 0;
            }
        "#;
    assert_stdout(source, "4294967295 18446744073709551615\n");
}

#[test]
fn unary_integer_operators_preserve_promoted_width_and_signedness() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                long l = 2147483648L;
                unsigned int u = 0U;
                unsigned long ul = 0UL;
                printf("%ld %u %lu %lu %lu\n",
                    -l, ~u, ~ul, sizeof(-l), sizeof(~ul));
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        "-2147483648 4294967295 18446744073709551615 8 8\n"
    );
}

#[test]
fn integer_promotions_determine_expression_types() {
    let source = r#"
            #define IS_INT(value) _Generic((value), int: 1, default: 0)
            enum Choice { NEGATIVE = -1, POSITIVE = 1 };

            int main(void) {
                return ~((const unsigned char)255) != -256
                    || sizeof(+((const unsigned char)255)) != sizeof(int)
                    || !IS_INT((_Bool)1 + (_Bool)1)
                    || !IS_INT((unsigned char)1 * (signed char)2)
                    || !IS_INT((enum Choice)NEGATIVE | (enum Choice)POSITIVE)
                    || !IS_INT(1 ? (short)1 : (short)2);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn out_of_range_unsigned_to_signed_conversions_use_twos_complement() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                signed char byte = (unsigned char)255;
                int word = 4294967295U;
                long long wide = 18446744073709551615ULL;
                long mixed = 1 - 130000UL;
                printf("%d %d %lld %ld\n", byte, word, wide, mixed);
                return 0;
            }
        "#;
    assert_stdout(source, "-1 -1 -1 -129999\n");
}

#[test]
fn integer_literal_suffixes_and_wider_decimal_constants_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                long x = 2147483648;
                unsigned int u = 4000000000U;
                unsigned long ul = 4000000000UL;
                unsigned long long ull = 18446744073709551615ULL;
                printf("%ld %u %lu %llu\n", x, u, ul, ull);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        "2147483648 4000000000 4000000000 18446744073709551615\n"
    );
}

#[test]
fn hexadecimal_and_octal_integer_literals_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                unsigned int u = 0xffffffff;
                int o = 077;
                long h = 0x100000000;
                printf("%u %d %ld\n", u, o, h);
                return 0;
            }
        "#;
    assert_stdout(source, "4294967295 63 4294967296\n");
}

#[test]
fn invalid_octal_integer_literal_is_rejected() {
    let source = r#"
            int main(void) {
                int x = 09;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "invalid octal");
}

#[test]
fn invalid_integer_literal_suffix_order_is_rejected() {
    for literal in ["1lul", "1ulu", "1lll", "1uul"] {
        let source = format!("int main(void) {{ return {literal}; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render().contains("unsupported integer literal suffix"),
            "unexpected diagnostic for {literal}: {}",
            err.render()
        );
    }
}

#[test]
fn unsuffixed_decimal_too_large_for_signed_types_is_rejected() {
    let source = r#"
            int main(void) {
                unsigned long long x = 18446744073709551615;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "out of supported range");
}

#[test]
fn real_and_complex_division_by_zero_are_undefined() {
    for source in [
        "int main(void) { volatile double z = 0.0; return (1.0 / z) != 0.0; }\n",
        "#include <complex.h>\nint main(void) { volatile double complex z = 0.0 + 0.0 * I; return creal((1.0 + I) / z) != 0.0; }\n",
    ] {
        let diagnostic = rendered_diagnostic(source);
        assert!(diagnostic.contains("division by zero"), "{diagnostic}");
        assert!(diagnostic.contains("standard: 6.5.5"), "{diagnostic}");
    }
}

#[test]
fn bool_normalization_promotions_and_sizeof_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                _Bool a = 3;
                _Bool b = 0.0;
                int x = 1;
                _Bool c = &x;
                printf("%d %d %d %d %lu\n", a, b, c, a + c, sizeof(_Bool));
                return 0;
            }
        "#;
    assert_stdout(source, "1 0 1 2 1\n");
}

#[test]
fn floating_point_arithmetic_calls_and_sizes_work() {
    let source = r#"
            #include <stdio.h>
            float sum(float x, float y) { return x + y; }
            long double twice(long double x) { return x * 2.0L; }

            int main(void) {
                float a = 1.25f;
                double b = 2.25;
                long double c = 1.5L;
                printf("%f %f %Lf %d %lu %lu %lu\n",
                       a + b,
                       sum(a, b),
                       twice(c),
                       (_Bool)0.5,
                       sizeof(float),
                       sizeof(double),
                       sizeof(long double));
                return 0;
            }
        "#;
    assert_stdout(source, "3.500000 3.500000 3.000000 1 4 8 8\n");
}

#[test]
fn hexadecimal_floating_literals_parse() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                float f = 0x1p-1f;
                double d = 0x1.8p+1;
                long double l = 0x1p+4L;
                printf("%f %f %Lf\n", f, d, l);
                return 0;
            }
        "#;
    assert_stdout(source, "0.500000 3.000000 16.000000\n");
}

#[test]
fn hexadecimal_floating_literals_require_a_binary_exponent() {
    for literal in ["0x1.2", "0x1.", "0x.8", "0x1.2f"] {
        let source = format!("int main(void) {{ double x = {literal}; return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render()
                .contains("hexadecimal floating literal requires a binary exponent"),
            "unexpected diagnostic for {literal}: {}",
            err.render()
        );
    }
}

#[test]
fn decimal_and_fractional_hex_float_literals_parse() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                double a = .5;
                double b = 1e2;
                double c = 0x1.fp+2;
                double d = .1e+2;
                printf("%f %f %f %f\n", a, b, c, d);
                return 0;
            }
        "#;
    assert_stdout(source, "0.500000 100.000000 7.750000 10.000000\n");
}

#[test]
fn floating_comparisons_and_bool_conversion_handle_nan() {
    let source = r#"
            #include <stdio.h>
            #include <math.h>
            int main(void) {
                double x = NAN;
                printf("%d %d %d\n", x == x, x != x, (_Bool)x);
                return 0;
            }
        "#;
    assert_stdout(source, "0 1 1\n");
}

#[test]
fn float_to_integer_out_of_range_is_ub() {
    let source = r#"
            int main(void) {
                int x = (int)1e100L;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("floating to integer conversion"));
    assert!(rendered.contains("outside the range of the destination type"));
}

#[test]
fn float_to_64_bit_integer_checks_exact_exclusive_upper_bounds() {
    for source in [
        "int main(void) { long x = (long)9223372036854775808.0; return 0; }\n",
        "int main(void) { unsigned long x = (unsigned long)18446744073709551616.0; return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(rendered.contains("floating to integer conversion"));
        assert!(rendered.contains("outside the range of the destination type"));
    }
}

#[test]
fn double_to_float_out_of_range_is_ub() {
    let source = r#"
            int main(void) {
                float x = 1e100;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("floating conversion"));
    assert!(rendered.contains("outside the range of the destination type"));
}

#[test]
fn floating_expression_overflow_is_ub() {
    let source = r#"
            #include <float.h>
            int main(void) {
                volatile double value = DBL_MAX;
                volatile double result = value * 2.0;
                return result != 0.0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("floating arithmetic result"));
    assert!(rendered.contains("outside the range of its type"));
}

#[test]
fn nonmathematical_floating_expression_result_is_ub() {
    let source = r#"
            #include <math.h>
            int main(void) {
                volatile double infinity = INFINITY;
                volatile double result = infinity - infinity;
                return result != 0.0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("floating arithmetic result"));
    assert!(rendered.contains("not mathematically defined"));
}

#[test]
fn complex_expression_overflow_is_ub() {
    let source = r#"
            #include <complex.h>
            #include <float.h>
            int main(void) {
                volatile double complex value = CMPLX(DBL_MAX, DBL_MAX);
                volatile double complex result = value * CMPLX(2.0, 0.0);
                return creal(result) != 0.0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("complex arithmetic result"));
    assert!(rendered.contains("outside the range of its type"));
}

#[test]
fn formatted_output_allows_disjoint_slices_and_zero_length_destinations() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char text[32] = "abc";
                int copied = snprintf(text + 16, 8, "%s", text);
                int measured = snprintf(text, 0, "%s", text);
                printf("%d %d %s\n", copied, measured, text + 16);
                return 0;
            }
        "#;
    assert_stdout(source, "3 3 abc\n");
}

#[test]
fn prefixed_character_constants_work_in_integer_constant_expressions() {
    let source = r#"
            _Static_assert(L'a' == L'a', "wide");
            _Static_assert(u'a' == u'a', "utf16");
            _Static_assert(U'a' == U'a', "utf32");
            #if L'a' != L'a' || u'a' != u'a' || U'a' != U'a'
            #error prefixed character constant mismatch
            #endif
            int main(void) { return 0; }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn unicode_character_numeric_escapes_use_the_integer_type_range() {
    let source = r#"
            int main(void) {
                return u'\xD800' != 0xD800u
                    || U'\xD800' != 0xD800u
                    || U'\x110000' != 0x110000u;
            }
        "#;
    assert_exit_status(source, 0);

    assert!(run_source("test.c", r"int main(void) { return u'\x10000'; }").is_err());
}

#[test]
fn main_must_return_int_and_cannot_be_variadic() {
    for (source, expected) in [
        ("long main(void) { return 0; }\n", "main must return int"),
        (
            "int main(int count, ...) { return count != 1; }\n",
            "main cannot be variadic",
        ),
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains(expected));
    }
}

#[test]
fn common_constraint_diagnostics_describe_the_c_rule_without_lvalue_jargon() {
    for (source, expected) in [
        (
            "int main(void) { int value = 1; return *value; }\n",
            "the * operator requires a pointer, but this expression has type int",
        ),
        (
            "int main(void) { int *pointer = &3; return 0; }\n",
            "the & operator requires an object or function",
        ),
        (
            "int main(void) { int a[2], b[2]; a = b; return 0; }\n",
            "arrays cannot be assigned; assign their elements individually",
        ),
        (
            "int main(void) { const int value = 1; value = 2; return 0; }\n",
            "the left side of = is const and cannot be changed",
        ),
        (
            "struct P { int x; }; int main(void) { struct P p = {1}; return p + 1; }\n",
            "this arithmetic operator requires numeric operands",
        ),
        (
            "int main(void) { int value = 1; return value[0]; }\n",
            "array subscripting requires an array or pointer and an integer index",
        ),
        (
            "int main(void) { (1 + 2)++; return 0; }\n",
            "++ and -- require a changeable arithmetic variable or pointer",
        ),
        (
            "int main(void) { int value = 1 return value; }\n",
            "expected ';', found keyword 'return'",
        ),
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(rendered.contains(expected), "{rendered}");
        assert!(!rendered.contains("lvalue"), "{rendered}");
    }
}

#[test]
fn repeated_compound_literal_evaluation_reuses_one_object_in_an_active_scope() {
    let source = r#"
            int main(void) {
                int count = 0;
                int *first = 0;
                int *current = 0;

            again:
                current = (int[]){ count };
                if (count++ == 0) {
                    first = current;
                    goto again;
                }
                return current != first || *current != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn alignof_accepts_variably_modified_complete_types_without_evaluating_bounds() {
    let source = r#"
            int main(void) {
                int n = 3;
                if (_Alignof(int[n]) != _Alignof(int)) return 1;
                if (_Alignof(int (*)[++n]) != _Alignof(void *)) return 2;
                return n != 3;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn alignof_has_size_t_type() {
    let source = r#"
            int main(void) {
                return _Generic(_Alignof(int), unsigned long: 0, default: 1);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn immediate_floating_constant_cast_is_an_integer_constant_expression() {
    let source = r#"
            enum { VALUE = (int)1.5, TRUTH = (_Bool)0.5 };
            int main(void) {
                return VALUE != 1 || TRUTH != 1;
            }
        "#;
    assert_exit_status(source, 0);

    assert_diagnostic_contains(
        "enum { VALUE = (unsigned char)256.0 };",
        "outside the range of the integer cast type",
    );
}

#[test]
fn multicharacter_constants_are_supported() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%d %d\n", 'ab', 'abc');
                return 0;
            }
        "#;
    assert_stdout(source, "24930 6382179\n");
}

#[test]
fn single_byte_character_constants_follow_signed_plain_char() {
    let source = r#"
            #include <limits.h>
            int main(void) {
                return CHAR_MIN == SCHAR_MIN && '\xFF' == -1 ? 0 : 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn utf16_and_utf32_multicharacter_constants_are_accepted() {
    let source = r#"
            int main(void) {
                return u'ab' != u'ab' || U'abc' != U'abc';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn adjacent_string_literals_are_concatenated() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%s\n", "ab" "cd");
                return 0;
            }
        "#;
    assert_stdout(source, "abcd\n");
}

#[test]
fn mixed_wide_and_narrow_adjacent_string_literals_become_wide() {
    let source = r#"
            #include <stdio.h>
            #include <wchar.h>
            int main(void) {
                wchar_t *text = L"ab" "cd";
                printf("%ls\n", text);
                return 0;
            }
        "#;
    assert_stdout(source, "abcd\n");
}

#[test]
fn adjacent_wide_and_utf8_string_literals_are_rejected() {
    assert_diagnostic_contains(
        r#"int main(void) { return sizeof(L"a" u8"b"); }"#,
        "cannot mix wide and UTF-8 prefixes",
    );
    assert_diagnostic_contains(
        r#"int main(void) { return sizeof(u8"a" U"b"); }"#,
        "cannot mix wide and UTF-8 prefixes",
    );
}

#[test]
fn utf8_and_ordinary_adjacent_string_literals_remain_narrow() {
    let source = r#"
            int main(void) {
                char *a = u8"ab" "cd";
                char *b = "ab" u8"cd";
                return a[3] != 'd' || b[3] != 'd';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn narrow_numeric_escapes_produce_single_bytes() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char text[] = "\xff";
                printf("%zu %d\n", sizeof text, (unsigned char)text[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "2 255\n");
}

#[test]
fn cboxes_expression_eval_identifies_numeric_value_literals() {
    let source = "int main(void) { 0; return 0; }\n";
    let evaluate = |text: &str| {
        crate_run_source_with_options(
            "test.c",
            source,
            &RunOptions {
                expression_eval: Some(RunExpressionEvalRequest {
                    expression: text.to_owned(),
                    event_index: 0,
                }),
                ..RunOptions::default()
            },
        )
        .unwrap()
        .expression
        .expect("expression result should exist")
    };

    let integer = evaluate("-2");
    let integer_literal = integer
        .value_literal
        .expect("signed integer should count as a value literal");
    assert_eq!(integer_literal.kind, "integer");
    assert!(!integer_literal.has_suffix);

    let suffixed_integer = evaluate("2LL");
    let suffixed_integer_literal = suffixed_integer
        .value_literal
        .expect("suffixed integer should count as a value literal");
    assert_eq!(suffixed_integer.ty, "long long");
    assert_eq!(suffixed_integer_literal.kind, "integer");
    assert!(suffixed_integer_literal.has_suffix);

    let double = evaluate("2.0");
    let double_literal = double
        .value_literal
        .expect("double should count as a value literal");
    assert_eq!(double.ty, "double");
    assert_eq!(double_literal.kind, "floating");
    assert!(!double_literal.has_suffix);

    let float = evaluate("2.0f");
    let float_literal = float
        .value_literal
        .expect("float should count as a value literal");
    assert_eq!(float.ty, "float");
    assert_eq!(float_literal.kind, "floating");
    assert!(float_literal.has_suffix);

    assert!(evaluate("1 + 1").value_literal.is_none());
    assert!(evaluate("(2)").value_literal.is_none());
}

#[test]
fn unterminated_literals_and_comments_point_to_the_opening_delimiter() {
    let cases = [
        (
            "int main(void) {\n    char a[] = \"hi\n}\n",
            "unterminated quoted literal",
            "test.c:2:16",
        ),
        (
            "int main(void) {\n    char c = 'x;\n}\n",
            "unterminated quoted literal",
            "test.c:2:14",
        ),
        (
            "int main(void) {\n  /* never ends\n}\n",
            "unterminated block comment",
            "test.c:2:3",
        ),
    ];

    for (source, message, location) in cases {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert!(rendered.contains(message), "{rendered}");
        assert!(rendered.contains(location), "{rendered}");
    }
}

#[test]
fn if_expression_character_constants_support_universal_escapes() {
    let source = r#"
            #include <stdio.h>
            #if '\u0024' == 36
            int main(void) { printf("ok\n"); return 0; }
            #else
            int main(void) { printf("bad\n"); return 0; }
            #endif
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn strdup_duplicates_into_freeable_storage() {
    let source = r#"
            #include <stdlib.h>
            #include <string.h>

            int main(void) {
                char *copy = strdup("abc");
                int ok = copy[0] == 'a' && copy[1] == 'b' && copy[2] == 'c' && copy[3] == '\0';
                free(copy);
                return ok ? 0 : 1;
            }
        "#;
    assert_stdout(source, "");
}

#[test]
fn math_requires_math_header() {
    let source = r#"
            int main(void) {
                return sin(0.0) != 0.0;
            }
        "#;
    assert_diagnostic_contains(source, "undeclared identifier sin");
}

#[test]
fn mixed_math_function_families_work() {
    let source = r#"
            #include <stdio.h>
            #include <math.h>

            int main(void) {
                printf("%d %d %d %d\n",
                    powl(2.0L, 3.0L) == 8.0L,
                    ilogb(8.0) == 3,
                    scalbln(1.5, 2) == 6.0,
                    fma(1.25, 2.0, 1.0) == 3.5);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1\n");
}

#[test]
fn abs_of_int_min_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                abs(-2147483647 - 1);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("signed integer overflow"));
    assert!(rendered.contains("7.20.6.1"));
}

#[test]
fn complex_transcendentals_and_accessors_work() {
    let source = r#"
            #include <stdio.h>
            #include <complex.h>

            int main(void) {
                double complex root = csqrt(CMPLX(-4.0, 0.0));
                double complex power = cpow(I, CMPLX(2.0, 0.0));
                printf("%d %d %d %d\n",
                    cabs(CMPLX(3.0, 4.0)) == 5.0,
                    creal(power) < -0.9 && creal(power) > -1.1,
                    cimag(root) > 1.9 && cimag(root) < 2.1,
                    creal(ccos(CMPLX(0.0, 0.0))) == 1.0);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1\n");
}

#[test]
fn complex_library_requires_complex_header() {
    let source = r#"
            int main(void) {
                return cabs(0.0 + 0.0);
            }
        "#;
    assert_diagnostic_contains(source, "undeclared identifier cabs");
}

#[test]
fn complex_math_preserves_branch_cuts_and_avoids_spurious_intermediate_overflow() {
    assert_exit_status(
        r#"
        #include <complex.h>
        #include <float.h>
        #include <math.h>
        static double complex constant = CMPLX(2.0, 3.0);
        int main(void) {
            double complex z = CMPLX(1e308, 1e308) / CMPLX(1.0, 1.0);
            if (!isfinite(creal(z)) || creal(z) < 9e307 || cimag(z) != 0) return 1;
            z = csqrt(CMPLX(DBL_MAX, DBL_MAX));
            if (!isfinite(creal(z)) || !isfinite(cimag(z)) || creal(z) < 1e154 || cimag(z) < 1e153) return 2;
            z = clog(CMPLX(DBL_MAX, DBL_MAX));
            if (!isfinite(creal(z))) return 3;
            z = cacosh(CMPLX(-2.0, -0.0));
            if (creal(z) < 1.3 || cimag(z) > -3.14) return 4;
            z = casinh(CMPLX(-0.0, 2.0));
            if (creal(z) > -1.3 || cimag(z) < 1.57) return 5;
            z = casin(CMPLX(0.0, 1e-100));
            if (cimag(z) < 0.99e-100 || cimag(z) > 1.01e-100) return 6;
            z = ctan(CMPLX(1.0, 1000.0));
            if (isnan(creal(z)) || cimag(z) != 1.0) return 7;
            z = ctanh(CMPLX(1000.0, 1.0));
            if (creal(z) != 1.0 || isnan(cimag(z))) return 8;
            z = cpow(CMPLX(0.0, 0.0), CMPLX(2.0, 0.0));
            if (creal(z) != 0.0 || cimag(z) != 0.0) return 9;
            return creal(constant) != 2.0 || cimag(constant) != 3.0;
        }
    "#,
        0,
    );
}

#[test]
fn tgmath_header_selects_real_and_complex_variants() {
    let source = r#"
            #include <stdio.h>
            #include <tgmath.h>
            #include <complex.h>

            int main(void) {
                float xf = 4.0f;
                double xd = 9.0;
                double _Complex z = 3.0 + 4.0 * I;
                printf("%lu %lu %lu %lu %lu %d\n",
                    sizeof(sqrt(xf)),
                    sizeof(sqrt(xd)),
                    sizeof(sqrt(z)),
                    sizeof(fabs(z)),
                    sizeof(cabs(xf)),
                    fabs(z) == 5.0);
                return 0;
            }
        "#;
    assert_stdout(source, "4 8 16 8 8 1\n");
}

#[test]
fn constraint_checking_does_not_evaluate_unreachable_expressions() {
    let source = r#"
            int main(void) {
                if (0) {
                    int zero = 0;
                    int value = 1 / zero;
                    int *null = 0;
                    value += *null;
                }
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn c11_unicode_identifiers_literals_and_uchar_conversions_work() {
    let source = r#"
#include <uchar.h>
#include <string.h>
int main(void) {
    int α = 4;
    int a\u0301 = 6;
    char16_t s16[] = u"A🍌";
    char32_t s32[] = U"A🍌";
    char utf8[] = u8"é";
    char16_t converted = 0;
    mbstate_t state = {0};
    if (\u03b1 != 4 || á != 6 || sizeof s16 / sizeof s16[0] != 4) return 1;
    if (sizeof s32 / sizeof s32[0] != 3 || s32[1] != U'🍌') return 2;
    if (strlen(utf8) != 2 || u'A' != 65 || U'A' != 65) return 3;
    if (mbrtoc16(&converted, "A", 1, &state) != 1 || converted != u'A') return 4;
    return 0;
}
"#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
    assert!(run_source("test.c", "int \\u0301invalid;").is_err());
}

#[test]
fn sqlite_varint_subroutines_match_boundary_and_generated_values() {
    let source = sqlite_varint_source();
    let result = run_source("sqlite-varint.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn sqlite_atoi64_subroutine_matches_decimal_boundaries() {
    // Extracted from SQLite src/util.c. The sqlite3Isspace lookup macro and
    // testcase instrumentation are replaced with equivalent small helpers.
    let source = r#"
            #include <assert.h>
            #include <stdint.h>

            typedef unsigned char u8;
            typedef int64_t i64;
            typedef uint64_t u64;

            #define SQLITE_UTF8 1
            #define SQLITE_UTF16LE 2
            #define SQLITE_UTF16BE 3
            #define LARGEST_INT64 ((i64)0x7fffffffffffffffLL)
            #define SMALLEST_INT64 (-LARGEST_INT64-1)

            static int sqlite3Isspace(char c){
              return c==' ' || c=='\t' || c=='\n' || c=='\v' || c=='\f' || c=='\r';
            }

            static int compare2pow63(const char *zNum, int incr){
              int c = 0;
              int i;
              const char *pow63 = "922337203685477580";
              for(i=0; c==0 && i<18; i++){
                c = (zNum[i*incr]-pow63[i])*10;
              }
              if( c==0 ) c = zNum[18*incr] - '8';
              return c;
            }

            int sqlite3Atoi64(const char *zNum, i64 *pNum, int length, u8 enc){
              int incr;
              u64 u = 0;
              int neg = 0;
              int i, j;
              unsigned int c = 0;
              int nonNum = 0;
              int rc;
              const char *zStart;
              const char *zEnd = zNum + length;
              assert( enc==SQLITE_UTF8 || enc==SQLITE_UTF16LE || enc==SQLITE_UTF16BE );
              if( enc==SQLITE_UTF8 ){
                incr = 1;
              }else{
                incr = 2;
                length &= ~1;
                assert( SQLITE_UTF16LE==2 && SQLITE_UTF16BE==3 );
                for(i=3-enc; i<length && zNum[i]==0; i+=2){}
                nonNum = i<length;
                zEnd = &zNum[i^1];
                zNum += (enc&1);
              }
              while( zNum<zEnd && sqlite3Isspace(*zNum) ) zNum+=incr;
              if( zNum<zEnd ){
                if( *zNum=='-' ){
                  neg = 1;
                  zNum+=incr;
                }else if( *zNum=='+' ){
                  zNum+=incr;
                }
              }
              zStart = zNum;
              while( zNum<zEnd && zNum[0]=='0' ){ zNum+=incr; }
              for(i=0; &zNum[i]<zEnd && (c=(unsigned)zNum[i]-'0')<=9; i+=incr){
                u = u*10 + c;
              }
              if( u>LARGEST_INT64 ){
                *pNum = neg ? SMALLEST_INT64 : LARGEST_INT64;
              }else if( neg ){
                *pNum = -(i64)u;
              }else{
                *pNum = (i64)u;
              }
              rc = 0;
              if( i==0 && zStart==zNum ){
                rc = -1;
              }else if( nonNum ){
                rc = 1;
              }else if( &zNum[i]<zEnd ){
                int jj = i;
                do{
                  if( !sqlite3Isspace(zNum[jj]) ){
                    rc = 1;
                    break;
                  }
                  jj += incr;
                }while( &zNum[jj]<zEnd );
              }
              if( i<19*incr ){
                assert( u<=LARGEST_INT64 );
                return rc;
              }else{
                j = i>19*incr ? 1 : compare2pow63(zNum, incr);
                if( j<0 ){
                  assert( u<=LARGEST_INT64 );
                  return rc;
                }else{
                  *pNum = neg ? SMALLEST_INT64 : LARGEST_INT64;
                  if( j>0 ) return 2;
                  assert( u-1==LARGEST_INT64 );
                  return neg ? rc : 3;
                }
              }
            }

            static int check(const char *text, int length, u8 encoding,
                             int expectedRc, i64 expectedValue){
              i64 value = 123;
              int rc = sqlite3Atoi64(text, &value, length, encoding);
              return rc==expectedRc && value==expectedValue;
            }

            int main(void){
              static const char minText[] = "-9223372036854775808";
              static const char maxText[] = "9223372036854775807";
              static const char positiveEdge[] = "9223372036854775808";
              static const char negativeOverflow[] = "-9223372036854775809";
              static const char unsignedMax[] = "18446744073709551615";
              static const char longZeros[] = "00000000000000000000000000000000000042";
              static const char utf16le[] = {'4',0,'2',0};
              /* SQLite forms zNum[length+1] before shifting the big-endian
              ** pointer by one byte, so retain its usual padding byte. */
              static const char utf16be[5] = {0,'4',0,'2',0};

              if( !check("", 0, SQLITE_UTF8, -1, 0) ) return 1;
              if( !check("  +42 ", 6, SQLITE_UTF8, 0, 42) ) return 2;
              if( !check(minText, sizeof(minText)-1, SQLITE_UTF8, 0, SMALLEST_INT64) ) return 3;
              if( !check(maxText, sizeof(maxText)-1, SQLITE_UTF8, 0, LARGEST_INT64) ) return 4;
              if( !check(positiveEdge, sizeof(positiveEdge)-1, SQLITE_UTF8, 3, LARGEST_INT64) ) return 5;
              if( !check(negativeOverflow, sizeof(negativeOverflow)-1, SQLITE_UTF8, 2, SMALLEST_INT64) ) return 6;
              if( !check(unsignedMax, sizeof(unsignedMax)-1, SQLITE_UTF8, 2, LARGEST_INT64) ) return 7;
              if( !check("123abc", 6, SQLITE_UTF8, 1, 123) ) return 8;
              if( !check("abc", 3, SQLITE_UTF8, -1, 0) ) return 9;
              if( !check(longZeros, sizeof(longZeros)-1, SQLITE_UTF8, 0, 42) ) return 10;
              if( !check(utf16le, sizeof(utf16le), SQLITE_UTF16LE, 0, 42) ) return 11;
              if( !check(utf16be, 4, SQLITE_UTF16BE, 0, 42) ) return 12;
              return 0;
            }
        "#;
    let result = run_source("sqlite-atoi64.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn sqlite_small_integer_and_dequote_subroutines_match_edge_cases() {
    // Extracted from SQLite src/util.c. Character-table macros and compiler
    // overflow builtins use the equivalent portable branches below.
    let source = r#"
            #include <stdint.h>
            #include <string.h>

            typedef unsigned char u8;
            typedef uint32_t u32;
            typedef int64_t i64;

            #define LARGEST_INT64 ((i64)0x7fffffffffffffffLL)
            #define SMALLEST_INT64 (-LARGEST_INT64-1)

            static int sqlite3Isxdigit(int c){
              return (c>='0' && c<='9') || (c>='a' && c<='f') || (c>='A' && c<='F');
            }
            static int sqlite3Isdigit(int c){ return c>='0' && c<='9'; }
            static int sqlite3Isquote(int c){ return c=='\'' || c=='"' || c=='`' || c=='['; }

            u8 sqlite3HexToInt(int h){
              h += 9*(1&(h>>6));
              return (u8)(h & 0xf);
            }

            int sqlite3GetInt32(const char *zNum, int *pValue){
              i64 v = 0;
              int i, c;
              int neg = 0;
              if( zNum[0]=='-' ){
                neg = 1;
                zNum++;
              }else if( zNum[0]=='+' ){
                zNum++;
              }else if( zNum[0]=='0'
                    && (zNum[1]=='x' || zNum[1]=='X')
                    && sqlite3Isxdigit(zNum[2])
              ){
                u32 u = 0;
                zNum += 2;
                while( zNum[0]=='0' ) zNum++;
                for(i=0; i<8 && sqlite3Isxdigit(zNum[i]); i++){
                  u = u*16 + sqlite3HexToInt(zNum[i]);
                }
                if( (u&0x80000000)==0 && sqlite3Isxdigit(zNum[i])==0 ){
                  memcpy(pValue, &u, 4);
                  return 1;
                }else{
                  return 0;
                }
              }
              if( !sqlite3Isdigit(zNum[0]) ) return 0;
              while( zNum[0]=='0' ) zNum++;
              for(i=0; i<11 && (c = zNum[i] - '0')>=0 && c<=9; i++){
                v = v*10 + c;
              }
              if( i>10 ) return 0;
              if( v-neg>2147483647 ) return 0;
              if( neg ) v = -v;
              *pValue = (int)v;
              return 1;
            }

            int sqlite3AddInt64(i64 *pA, i64 iB){
              i64 iA = *pA;
              if( iB>=0 ){
                if( iA>0 && LARGEST_INT64 - iA < iB ) return 1;
              }else{
                if( iA<0 && -(iA + LARGEST_INT64) > iB + 1 ) return 1;
              }
              *pA += iB;
              return 0;
            }

            int sqlite3SubInt64(i64 *pA, i64 iB){
              if( iB==SMALLEST_INT64 ){
                if( (*pA)>=0 ) return 1;
                *pA -= iB;
                return 0;
              }else{
                return sqlite3AddInt64(pA, -iB);
              }
            }

            int sqlite3MulInt64(i64 *pA, i64 iB){
              i64 iA = *pA;
              if( iB>0 ){
                if( iA>LARGEST_INT64/iB ) return 1;
                if( iA<SMALLEST_INT64/iB ) return 1;
              }else if( iB<0 ){
                if( iA>0 ){
                  if( iB<SMALLEST_INT64/iA ) return 1;
                }else if( iA<0 ){
                  if( iB==SMALLEST_INT64 ) return 1;
                  if( iA==SMALLEST_INT64 ) return 1;
                  if( -iA>LARGEST_INT64/-iB ) return 1;
                }
              }
              *pA = iA*iB;
              return 0;
            }

            int sqlite3AbsInt32(int x){
              if( x>=0 ) return x;
              if( x==(int)0x80000000 ) return 0x7fffffff;
              return -x;
            }

            void sqlite3Dequote(char *z){
              char quote;
              int i, j;
              if( z==0 ) return;
              quote = z[0];
              if( !sqlite3Isquote(quote) ) return;
              if( quote=='[' ) quote = ']';
              for(i=1, j=0;; i++){
                if( z[i]==quote ){
                  if( z[i+1]==quote ){
                    z[j++] = quote;
                    i++;
                  }else{
                    break;
                  }
                }else{
                  z[j++] = z[i];
                }
              }
              z[j] = 0;
            }

            static int checkInt(const char *text, int expectedOk, int expectedValue){
              int value = 99;
              int ok = sqlite3GetInt32(text, &value);
              return ok==expectedOk && (!ok || value==expectedValue);
            }

            int main(void){
              i64 value;
              char quoted[] = "\"a\"\"b\"";
              char bracketed[] = "[a-b]";

              if( !checkInt("2147483647", 1, 2147483647) ) return 1;
              if( !checkInt("-2147483648", 1, (int)0x80000000) ) return 2;
              if( !checkInt("2147483648", 0, 0) ) return 3;
              if( !checkInt("-2147483649", 0, 0) ) return 4;
              if( !checkInt("123tail", 1, 123) ) return 5;
              if( !checkInt("0x7fffffff", 1, 2147483647) ) return 6;
              if( !checkInt("0x80000000", 0, 0) ) return 7;
              if( !checkInt("0x0000000000002a", 1, 42) ) return 8;

              value = LARGEST_INT64;
              if( sqlite3AddInt64(&value, 1)!=1 || value!=LARGEST_INT64 ) return 9;
              value = SMALLEST_INT64;
              if( sqlite3AddInt64(&value, -1)!=1 || value!=SMALLEST_INT64 ) return 10;
              value = -1;
              if( sqlite3SubInt64(&value, SMALLEST_INT64)!=0 || value!=LARGEST_INT64 ) return 11;
              value = 3037000500LL;
              if( sqlite3MulInt64(&value, 3037000500LL)!=1 || value!=3037000500LL ) return 12;
              value = -7;
              if( sqlite3MulInt64(&value, -6)!=0 || value!=42 ) return 13;
              if( sqlite3AbsInt32((int)0x80000000)!=0x7fffffff ) return 14;

              sqlite3Dequote(quoted);
              sqlite3Dequote(bracketed);
              if( strcmp(quoted, "a\"b")!=0 ) return 15;
              if( strcmp(bracketed, "a-b")!=0 ) return 16;
              return 0;
            }
        "#;
    let result = run_source("sqlite-small-util.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn reported_float_subnormal_is_classified_in_its_semantic_type() {
    let source = r#"
            #include <math.h>
            #include <stdio.h>

            int main(void) {
                float value = 0x1p-149f;
                printf("%d %d %d %d\n",
                    fpclassify(value), isnormal(value), isfinite(value), signbit(value));
                return 0;
            }
        "#;
    assert_stdout(source, "5 0 1 0\n");
}

#[test]
fn reported_complex_finite_components_do_not_spuriously_overflow() {
    let source = r#"
            #include <complex.h>
            #include <math.h>

            int main(void) {
                double p = acos(-1.0) / 2.0;
                double h = 710.5;
                double complex sine = csin(CMPLX(p, h));
                double complex cosine_h = ccosh(CMPLX(h, p));
                double complex sine_h = csinh(CMPLX(h, p));
                float pf = acosf(-1.0f) / 2.0f;
                float complex sine_f = csinf(CMPLXF(pf, 90.0f));
                if (!isinf(creal(sine)) || !isfinite(cimag(sine))) return 1;
                if (!isfinite(creal(cosine_h)) || !isinf(cimag(cosine_h))) return 2;
                if (!isfinite(creal(sine_h)) || !isinf(cimag(sine_h))) return 3;
                if (!isinf(crealf(sine_f)) || !isfinite(cimagf(sine_f))) return 4;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn muse_goto_into_for_scope_materializes_skipped_automatic_objects() {
    let source = r#"
            int main(void) {
                int i = 7;
                goto inside;
                for (int i = 1; ; ) {
                inside:
                    return i;
                }
            }
        "#;
    assert_diagnostic_contains(source, "uninitialized automatic object");
}

#[test]
fn muse_wide_and_utf_character_constants_are_integer_constant_expressions() {
    let source = r#"
            int main(void) {
                void *a = L'\0';
                void *b = u'\0';
                void *c = U'\0';
                switch (L'A') { case L'A': break; default: return 1; }
                switch (u'B') { case u'B': break; default: return 2; }
                switch (U'C') { case U'C': break; default: return 3; }
                return a != 0 || b != 0 || c != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn muse_compound_assignment_checks_the_final_assignment_conversion() {
    assert_diagnostic_contains(
        "int main(void) { int a[2]; int *p=a, *q=a+1; p -= q; return 0; }",
        "cannot implicitly convert",
    );
    assert_diagnostic_contains(
        "int main(void) { int a[1]; int i=0; i += a; return i; }",
        "cannot convert int* to int",
    );
    assert_exit_status(
        "int main(void) { int a[1]; _Bool b=0; b += a; return !b; }",
        0,
    );
}
