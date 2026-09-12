use super::*;

#[test]
fn diagnostic_includes_source_snippet() {
    let source = r#"
            int main(void) {
                int x;
                int y = x;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("--> test.c:"));
    assert!(rendered.contains("int y = x;"));
}

#[test]
fn object_like_macros_expand_but_not_inside_strings() {
    let source = r#"
            #include <stdio.h>
            #define VALUE 7

            int main(void) {
                printf("VALUE %d\n", VALUE);
                return 0;
            }
        "#;
    assert_stdout(source, "VALUE 7\n");
}

#[test]
fn undef_removes_object_like_macro() {
    let source = r#"
            #include <stdio.h>
            #define VALUE 7
            #undef VALUE

            int main(void) {
                int VALUE = 3;
                printf("%d\n", VALUE);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn function_like_macros_expand_and_prescan_arguments() {
    let source = r#"
            #include <stdio.h>
            #define ADD(x, y) ((x) + (y))
            #define TWICE(x) ADD(x, x)

            int main(void) {
                printf("%d\n", TWICE(3 + 1));
                return 0;
            }
        "#;
    assert_stdout(source, "8\n");
}

#[test]
fn preprocessor_constant_expressions_do_not_evaluate_dead_branches() {
    let source = r#"
            #if 1 || (1 / 0)
            #define FIRST 1
            #else
            #error logical-or branch was selected incorrectly
            #endif

            #if 0 && (1 / 0)
            #error logical-and branch was selected incorrectly
            #else
            #define SECOND 2
            #endif

            #if 1 ? 1 : (1 / 0)
            #define THIRD 3
            #else
            #error conditional branch was selected incorrectly
            #endif

            int main(void) { return FIRST + SECOND + THIRD - 6; }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn preprocessor_integer_expressions_preserve_intmax_signedness() {
    let source = r#"
            #if -1 < 1U
            #error signed value was not converted to uintmax_t
            #endif
            #if 18446744073709551615ULL + 1 != 0
            #error uintmax_t arithmetic did not wrap
            #endif
            #if (18446744073709551615ULL >> 63) != 1
            #error uintmax_t right shift was not logical
            #endif
            #if (1 ? -1 : 1U) < 0
            #error conditional operands did not use their common type
            #endif

            int main(void) { return 0; }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    for invalid in [
        "#if 1++1\n#endif\nint main(void) { return 0; }\n",
        "#if 1--1\n#endif\nint main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", invalid).unwrap_err();
        assert!(err.render().contains("not valid in #if expressions"));
    }
}

#[test]
fn preprocessor_still_diagnoses_evaluated_division_by_zero() {
    let source = "#if 1 / 0\nint x;\n#endif\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "division by zero in #if expression");
}

#[test]
fn preprocessor_diagnoses_undefined_signed_left_shifts() {
    for source in [
        "#if (1 << 63)\n#endif\nint main(void) { return 0; }\n",
        "#if ((-1) << 1)\n#endif\nint main(void) { return 0; }\n",
    ] {
        let diagnostic = rendered_diagnostic(source);
        assert!(diagnostic.contains("signed left shift"), "{diagnostic}");
        assert!(diagnostic.contains("standard: 6.5.7"), "{diagnostic}");
    }
}

#[test]
fn member_resolution_cache_distinguishes_macro_operand_record_types() {
    let source = r#"
            #define VALUE(object) ((object).value)
            struct Narrow { int value; };
            struct Wide { long value; };

            int main(void) {
                struct Narrow narrow = { 7 };
                struct Wide wide = { 5000000000L };
                return VALUE(narrow) != 7 || VALUE(wide) != 5000000000L;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn quoted_includes_resolve_relative_to_the_including_file() {
    let project = TestProject::new("include-relative");
    project
        .write(
            "main.c",
            "#include <stdio.h>\n#include \"dir/inner.h\"\nint main(void) { printf(\"%d\\n\", VALUE); return 0; }\n",
        )
        .unwrap();
    project
        .write("dir/inner.h", "#include \"value.h\"\n")
        .unwrap();
    project.write("dir/value.h", "#define VALUE 9\n").unwrap();

    let result = run_file(project.path("main.c")).unwrap();
    assert_eq!(result.stdout, "9\n");
}

#[test]
fn stdout_macro_can_be_passed_to_stream_output_functions() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                fputs("he", stdout);
                fputc('y', stdout);
                return 0;
            }
        "#;
    assert_stdout(source, "hey");
}

#[test]
fn fprintf_writes_through_stdout_macro() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                fprintf(stdout, "%d %s\n", 7, "ok");
                return 0;
            }
        "#;
    assert_stdout(source, "7 ok\n");
}

#[test]
fn hexadecimal_e_sign_sequence_remains_one_preprocessing_number() {
    for source in [
        "int main(void) { return 0x1e+2; }",
        "int main(void) { return 0x1E-foo; }",
    ] {
        assert!(run_source("test.c", source).is_err(), "accepted {source}");
    }
    assert_exit_status("int main(void) { return (0x1f + 2) != 33; }", 0);
    assert_exit_status("int main(void) { return 0x1p+2 != 4; }", 0);
}

#[test]
fn float_header_declares_subnormal_characterization_macros() {
    let source = r#"
            #include <float.h>
            #if FLT_HAS_SUBNORM != 1 || DBL_HAS_SUBNORM != 1 || LDBL_HAS_SUBNORM != 1
            #error unexpected subnormal support
            #endif
            int main(void) {
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn variadic_macros_expand_va_args() {
    let source = r#"
            #include <stdio.h>
            #define LOG(...) printf(__VA_ARGS__)

            int main(void) {
                LOG("%d %d\n", 4, 5);
                return 0;
            }
        "#;
    assert_stdout(source, "4 5\n");
}

#[test]
fn empty_macro_arguments_follow_c11_argument_counting() {
    let source = r#"
            #define IGNORE(value) 7
            #define ZERO() 9
            #define VARIADIC_ONLY(...) 11

            int main(void) {
                return IGNORE() != 7 || ZERO() != 9 || VARIADIC_ONLY() != 11;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    for invalid in [
        "#define F(value, ...) 0\nint main(void) { return F(); }\n",
        "#define F(value, ...) 0\nint main(void) { return F(1); }\n",
    ] {
        assert!(run_source("test.c", invalid).is_err());
    }
}

#[test]
fn macro_stringizing_and_token_pasting_work() {
    let source = r#"
            #include <stdio.h>
            #define STR(x) #x
            #define CAT(a, b) a ## b

            int main(void) {
                int xy = 7;
                printf("%s %d\n", &STR(  a +   b  )[0], CAT(x, y));
                return 0;
            }
        "#;
    assert_stdout(source, "a + b 7\n");

    let err = run_source(
        "test.c",
        "#define BAD(parameter) # not_a_parameter\nint main(void) { return 0; }\n",
    )
    .unwrap_err();
    assert!(
        err.render()
            .contains("# in macro replacement must be followed by a parameter name")
    );
}

#[test]
fn macro_replacements_can_supply_partial_function_invocations() {
    assert_exit_status(
        "#define F(a) 1\n#define G F\n#define H G(~\nint main(void) { return H 5) != 1; }",
        0,
    );
    assert_exit_status(
        "#define F(a) (a)\n#define H() F(1 +\nint main(void) { return H() 2) != 3; }",
        0,
    );
    assert!(
        run_source(
            "test.c",
            "#define F(a) (a)\n#define H F(\nint main(void) { return H 1; }"
        )
        .is_err()
    );
}

#[test]
fn macro_stringizing_preserves_literal_whitespace_commas_ucns_and_digraph_spelling() {
    let source = r#"
            #include <stdio.h>
            #define STR(value) #value
            #define MANY(...) #__VA_ARGS__

            int main(void) {
                printf("%s\n", STR("a  b"));
                printf("%s|%s|%s\n", MANY(a,b), MANY(a, b), MANY(a ,b));
                printf("%s\n", STR(\u03b1));
                printf("%s\n", STR(<:));
                return 0;
            }
        "#;
    assert_stdout(source, "\"a  b\"\na,b|a, b|a ,b\nα\n<:\n");
}

#[test]
fn line_macro_uses_the_argument_token_physical_line() {
    let source = r#"
            #include <stdio.h>
            #define ID(value) value
            int main(void) {
                printf("%d\n", ID(
                    __LINE__));
                return 0;
            }
        "#;
    assert_stdout(source, "6\n");
}

#[test]
fn token_pasting_uses_unexpanded_arguments_on_both_sides() {
    let source = r#"
            #define A x
            #define B y
            #define CAT_RAW(a, b) a ## b
            #define CAT_EXPANDED(a, b) CAT_RAW(a, b)

            int main(void) {
                int Ay = 3;
                int xB = 4;
                int xy = 5;
                return CAT_RAW(A, y) != 3
                    || CAT_RAW(x, B) != 4
                    || CAT_EXPANDED(A, B) != 5;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn macro_redefinitions_must_be_effectively_identical() {
    let equivalent = r#"
            #define VALUE 1  +  2
            #define VALUE 1 + 2
            #define APPLY(value) (value)
            #define APPLY(value) (value)
            int main(void) { return APPLY(VALUE) != 3; }
        "#;
    assert_eq!(run_source("test.c", equivalent).unwrap().exit_status, 0);

    for incompatible in [
        "#define VALUE 1\n#define VALUE 2\nint main(void) { return 0; }\n",
        "#define APPLY(value) value\n#define APPLY(other) other\nint main(void) { return 0; }\n",
        "#define VALUE 1\n#define VALUE() 1\nint main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", incompatible).unwrap_err();
        assert!(err.render().contains("incompatible redefinition of macro"));
    }
}

#[test]
fn macro_rescanning_can_form_a_function_like_invocation() {
    let source = r#"
            #define ALIAS INCREMENT
            #define INCREMENT(value) ((value) + 1)
            #define FUNCTION_NAME(ignored) INCREMENT

            int main(void) {
                return ALIAS(2) != 3 || FUNCTION_NAME(0)(4) != 5;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let recursive = r#"
            int SELF(int value) { return value; }
            #define SELF(value) SELF
            int main(void) { return SELF(0)(6) != 6; }
        "#;
    assert_eq!(run_source("test.c", recursive).unwrap().exit_status, 0);

    let self_referential_argument = r#"
            #define GLOBAL(type, name) name
            #define globalValue GLOBAL(int, globalValue)
            int globalValue = 7;
            int main(void) { return globalValue != 7; }
        "#;
    assert_eq!(
        run_source("test.c", self_referential_argument)
            .unwrap()
            .exit_status,
        0
    );
}

#[test]
fn token_paste_operator_cannot_be_at_a_replacement_list_boundary() {
    for source in [
        "#define BAD ## value\nint main(void) { return 0; }\n",
        "#define BAD value ##\nint main(void) { return 0; }\n",
        "#define BAD(value) ## value\nint main(void) { return 0; }\n",
        "#define BAD(value) value %:%:\nint main(void) { return 0; }\n",
    ] {
        assert_diagnostic_contains(source, "beginning or end");
    }
}

#[test]
fn predefined_file_and_line_macros_and_line_directive_work() {
    let source = r#"
            #include <stdio.h>
            #line 40 "virt.c"
            int main(void) {
                printf("%s %d\n", &__FILE__[0], __LINE__);
                return 0;
            }
        "#;
    assert_stdout(source, "virt.c 41\n");
}

#[test]
fn line_directive_expands_macros_and_accepts_spaces_in_file_names() {
    let source = r#"
            #include <stdio.h>
            #define NEXT_LINE 7
            #line NEXT_LINE "virtual file.c"
            int main(void) {
                printf("%s %d\n", __FILE__, __LINE__);
                return 0;
            }
        "#;
    assert_stdout(source, "virtual file.c 8\n");
}

#[test]
fn line_directive_rejects_out_of_range_numbers_and_trailing_tokens() {
    for source in [
        "#line 0\nint main(void) { return 0; }\n",
        "#line 2147483648\nint main(void) { return 0; }\n",
        "#line 2 \"x\" extra\nint main(void) { return 0; }\n",
    ] {
        assert_diagnostic_contains(source, "#line");
    }
}

#[test]
fn inactive_conditionals_allow_relaxed_nested_directive_syntax() {
    let source = "#if 0\n#ifdef\n#endif\n#ifndef anything extra\n#endif\n#endif\nint main(void) { return 0; }\n";
    assert_exit_status(source, 0);
}

#[test]
fn active_else_and_endif_directives_reject_trailing_tokens() {
    assert_diagnostic_contains(
        "#if 1\nint x;\n#else extra\n#endif\nint main(void) { return 0; }\n",
        "after #else",
    );
    assert_diagnostic_contains(
        "#if 1\nint x;\n#endif extra\nint main(void) { return 0; }\n",
        "after #endif",
    );

    let skipped = "#if 0\n#if 0\n#else extra tokens are relaxed here\n#endif extra\n#endif\nint main(void) { return 0; }\n";
    assert_exit_status(skipped, 0);
}

#[test]
fn unknown_preprocessing_directive_name_is_a_valid_non_directive() {
    let source = "# vendor_extension 123\nint main(void) { return 0; }\n";
    assert_exit_status(source, 0);
}

#[test]
fn utf_environment_macros_match_the_exposed_encodings() {
    let source = r#"
            #include <uchar.h>
            #if __STDC_UTF_16__ != 1 || __STDC_UTF_32__ != 1
            #error UTF environment macros are missing
            #endif
            int main(void) {
                char16_t utf16[] = u"🍌";
                char32_t utf32[] = U"🍌";
                return utf16[0] != 0xd83c || utf16[1] != 0xdf4c
                    || utf32[0] != 0x1f34c;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn backslash_newline_splices_preprocessor_lines() {
    let source = r#"
            #include <stdio.h>
            #define SUM(a, b) ((a) + \
                               (b))

            int main(void) {
                printf("%d\n", SUM(2, 3));
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
}

#[test]
fn line_splicing_precedes_universal_character_name_recognition() {
    let source = "int main(void) {\n    int \\u03\\\nb1 = 7;\n    return \\u03\\\nb1 != 7;\n}\n";
    assert_exit_status(source, 0);
}

#[test]
fn block_comments_are_one_space_and_macro_output_cannot_create_comments() {
    let valid = "#define A 1 /* comment\n*/ + 2\nint main(void) { return A - 3; }\n";
    assert_exit_status(valid, 0);

    let produced_line_comment =
        "#define glue(x,y) x##y\nint main(void) { glue(/,/) hidden(); return 0; }\n";
    assert!(run_source("test.c", produced_line_comment).is_err());

    let produced_block_comment =
        "#define slash /\nint main(void) { int x = 0; if (x) slash**/ return 0; }\n";
    assert!(run_source("test.c", produced_block_comment).is_err());
}

#[test]
fn line_splicing_preserves_physical_line_numbers() {
    let line_macro =
        "#define PLUS_ONE(value) ((value) + \\\n+1)\nint main(void) { return __LINE__ != 3; }\n";
    assert_eq!(run_source("test.c", line_macro).unwrap().exit_status, 0);

    let diagnostic = "#define PLUS_ONE(value) ((value) + \\\n+1)\nint main(void) {\n    int *pointer = 0;\n    return *pointer;\n}\n";
    let rendered = run_source("test.c", diagnostic).unwrap_err().render();
    assert!(rendered.contains("test.c:5:"), "{rendered}");

    let preprocessing_diagnostic = "#define PLUS_ONE(value) ((value) + \\\n+1)\n  #ifdef\n#endif\nint main(void) { return 0; }\n";
    let rendered = run_source("test.c", preprocessing_diagnostic)
        .unwrap_err()
        .render();
    assert!(rendered.contains("test.c:3:3"), "{rendered}");
}

#[test]
fn variadic_macro_diagnostic_counts_the_required_variadic_argument() {
    let source = "#define FIRST(required, ...) required\nint main(void) { return FIRST(1); }\n";
    let rendered = run_source("test.c", source).unwrap_err().render();
    assert!(
        rendered.contains("expects 2+ argument(s), got 1"),
        "{rendered}"
    );
}

#[test]
fn function_like_macro_invocations_can_span_multiple_physical_lines() {
    let source = r#"
            #include <stdio.h>
            #define SUM(a, b) ((a) + (b))

            int main(void) {
                printf("%d\n", SUM(2,
                                   3));
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
}

#[test]
fn comments_are_removed_during_preprocessing() {
    let source = r#"
            #include <stdio.h>
            int/**/main(void) {
                int value = 4; // line comment
                /* block
                   comment */
                printf("%d\n", value);
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn include_operand_can_come_from_a_macro() {
    let source = r#"
            #define IO_HEADER <stdio.h>
            #include IO_HEADER
            int main(void) {
                printf("ok\n");
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn include_operand_macro_can_expand_to_a_local_header_name() {
    let project = TestProject::new("macro-include-local");
    project.write("value.h", "#define VALUE 9\n").unwrap();
    project
        .write(
            "main.c",
            r#"
                #define LOCAL_HEADER "value.h"
                #include LOCAL_HEADER
                #include <stdio.h>
                int main(void) {
                    printf("%d\n", VALUE);
                    return 0;
                }

            "#,
        )
        .unwrap();
    let result = run_file(project.path("main.c")).unwrap();
    assert_eq!(result.stdout, "9\n");
}

#[test]
fn include_search_path_option_finds_headers_outside_source_directory() {
    let project = TestProject::new("include-search-path");
    project
        .write("include/value.h", "#define VALUE 13\n")
        .unwrap();
    project
        .write(
            "src/main.c",
            r#"
                #include <value.h>
                #include <stdio.h>
                int main(void) {
                    printf("%d\n", VALUE);
                    return 0;
                }

            "#,
        )
        .unwrap();
    let options = RunOptions {
        include_dirs: vec![project.path("include")],
        ..RunOptions::default()
    };
    let result = run_files_with_options([project.path("src/main.c")], &options).unwrap();
    assert_eq!(result.stdout, "13\n");
}

#[test]
fn null_preprocessing_directive_is_accepted() {
    let source = r#"
            #
            #   /* still null */
            int main(void) {
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn unterminated_block_comment_is_reported_during_preprocessing() {
    let source = r#"
            int main(void) {
                /* never ends
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unterminated block comment");
}

#[test]
fn preprocessor_diagnostics_do_not_default_to_the_start_of_the_file() {
    let cases = [
        (
            "int x;\n\n  #ifdef\n#endif\nint main(void) { return 0; }\n",
            "expected macro name after #ifdef",
            "test.c:3:3",
        ),
        (
            "int x;\n  #if @\n#endif\nint main(void) { return 0; }\n",
            "invalid token in #if expression",
            "test.c:2:3",
        ),
        (
            "int x;\n  #if 1\nint main(void) { return 0; }\n",
            "unterminated conditional directive",
            "test.c:2:3",
        ),
        (
            "#define F(x) x\nint main(void) {\n  F(1, 2);\n}\n",
            "expects 1 argument(s), got 2",
            "test.c:3:3",
        ),
    ];

    for (source, message, location) in cases {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert!(rendered.contains(message), "{rendered}");
        assert!(rendered.contains(location), "{rendered}");
    }
}

#[test]
fn preprocessor_error_directive_reports_an_error() {
    let source = r#"
            #error stop here
            int main(void) { return 0; }
        "#;
    assert_diagnostic_contains(source, "stop here");
}

#[test]
fn math_macros_constants_and_typedefs_work() {
    let source = r#"
            #include <stdio.h>
            #include <math.h>

            int main(void) {
                printf("%lu %lu %d %d %d %d %d %d %d\n",
                    sizeof(float_t),
                    sizeof(double_t),
                    math_errhandling,
                    fpclassify(0.0),
                    isfinite(1.0),
                    isinf(HUGE_VAL),
                    isnan(NAN),
                    isnormal(1.0),
                    signbit(-0.0));
                return 0;
            }
        "#;
    assert_stdout(source, "4 8 2 3 1 1 1 1 1\n");
}

#[test]
fn stdint_header_typedefs_and_macros_work() {
    let source = r#"
            #include <stdio.h>
            #include <stdint.h>

            int main(void) {
                printf("%lu %lu %lu %lu %lu %lu %d %d %d\n",
                    sizeof(int8_t),
                    sizeof(uint64_t),
                    sizeof(intptr_t),
                    sizeof(uintmax_t),
                    sizeof(int_fast16_t),
                    sizeof(int_least32_t),
                    INT8_MIN < 0,
                    UINT64_MAX > 0,
                    INTMAX_C(7) == 7);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        format!(
            "{} {} {} {} {} {} 1 1 1\n",
            std::mem::size_of::<i8>(),
            std::mem::size_of::<u64>(),
            std::mem::size_of::<isize>(),
            std::mem::size_of::<u64>(),
            std::mem::size_of::<i16>(),
            std::mem::size_of::<i32>(),
        )
    );
}

#[test]
fn inttypes_header_macros_and_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <inttypes.h>

            int main(void) {
                char *end = 0;
                imaxdiv_t d = imaxdiv(-9, 4);
                printf("%s %ld %ld %ld %d\n",
                    PRIdMAX,
                    imaxabs(-7),
                    d.quot,
                    d.rem,
                    (strtoimax("12x", &end, 10) == 12) && *end == 'x');
                return 0;
            }
        "#;
    assert_stdout(source, "ld 7 -2 -1 1\n");
}

#[test]
fn stdlib_header_macros_types_and_abs_family_work() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                div_t d;
                d.quot = 7;
                d.rem = 2;
                printf("%d %d %d %d %d %d\n",
                    EXIT_SUCCESS,
                    EXIT_FAILURE,
                    RAND_MAX > 0,
                    d.quot,
                    d.rem,
                    abs(-7) + (int)labs(-9L) + (int)llabs(-11LL));
                return 0;
            }
        "#;
    assert_stdout(source, "0 1 1 7 2 27\n");
}

#[test]
fn assert_macro_uses_explicit_addresses_for_its_generated_arrays() {
    let source = r#"
            #include <assert.h>
            static void helper(void) {
                assert(2 + 2 == 5);
            }
            int main(void) {
                helper();
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("assertion failed: 2 + 2 == 5"));
    assert!(rendered.contains("test.c"));
    assert!(rendered.contains("helper"));
}

#[test]
fn fenv_header_functions_and_macros_work() {
    let source = r#"
            #include <stdio.h>
            #include <fenv.h>

            int main(void) {
                fexcept_t flag = 0;
                fenv_t env;
                printf("%lu %lu %d %d %d\n",
                    sizeof(fenv_t),
                    sizeof(fexcept_t),
                    FE_ALL_EXCEPT > 0,
                    fegetenv(&env) == 0,
                    fegetexceptflag(&flag, FE_ALL_EXCEPT) == 0);
                fesetenv(FE_DFL_ENV);
                return 0;
            }
        "#;
    assert_stdout(source, "16 2 1 1 1\n");
}

#[test]
fn complex_header_macros_sizes_and_arithmetic_work() {
    let source = r#"
            #include <stdio.h>
            #include <complex.h>

            int main(void) {
                double complex z = CMPLX(1.5, 2.5);
                double complex w = 2.0 + 1.0 * I;
                double complex p = z * w;
                printf("%lu %lu %lu %d %d %d %d\n",
                    sizeof(float complex),
                    sizeof(double complex),
                    sizeof(long double complex),
                    creal(z) > 1.4 && creal(z) < 1.6,
                    cimag(z) > 2.4 && cimag(z) < 2.6,
                    creal(p) > 0.4 && creal(p) < 0.6,
                    cimag(conj(z)) > -2.6 && cimag(conj(z)) < -2.4);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        format!(
            "{} {} {} 1 1 1 1\n",
            8,
            16,
            crate::types::HOST_LONG_DOUBLE_SIZE * 2
        )
    );
}

#[test]
fn pragma_operator_is_accepted_after_macro_expansion() {
    let source = r#"
            #define DO_PRAGMA(x) _Pragma(#x)
            DO_PRAGMA(message("ignored"))
            _Pragma(L"wide ignored")
            _Pragma
            (
                "split ignored"
            )

            int main(void) {
                DO_PRAGMA(message("still ignored"))
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();

    assert!(
        run_source(
            "test.c",
            "int main(void) { return 1_Pragma(\"ignored\"); }\n",
        )
        .is_err()
    );
}

#[test]
fn c11_alternative_tokens_predefined_macros_and_old_style_definitions_work() {
    let source = r#"
??=include <stddef.h>
int add(int, int);
int add(a, b)
int a;
char b;
<%
    int values<:2:> = <% a, b %>;
    return values<:0:> + values<:1:>;
%>
int main(void) {
    /* Unicode is allowed in comments too: 🍌 */
#if !defined(__DATE__) || !defined(__TIME__) || !defined(__STDC_NO_THREADS__) || !defined(__STDC_NO_ATOMICS__)
#error missing predefined macro
#endif
    return add(2, 3) - 5;
}
"#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn reported_stdint_limits_and_constant_macro_types_are_correct() {
    let source = r#"
            #include <stdint.h>
            #include <signal.h>
            #include <wchar.h>
            #include <stdio.h>

            #if !defined(SIG_ATOMIC_MIN) || !defined(SIG_ATOMIC_MAX)
            #error missing sig_atomic_t limits
            #endif
            #if !defined(WINT_MIN) || !defined(WINT_MAX)
            #error missing wint_t limits
            #endif

            int main(void) {
                printf("%d %d %d %d %d %d\n",
                    SIG_ATOMIC_MIN <= -127, SIG_ATOMIC_MAX >= 127,
                    WINT_MIN <= -32767, WINT_MAX >= 32767,
                    _Generic(UINT8_C(1), int: 1, default: 0),
                    _Generic(UINT16_C(1), int: 1, default: 0));
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1 1 1\n");
}

#[test]
fn reported_inttypes_format_macros_match_the_corresponding_types() {
    let source = r#"
            #include <inttypes.h>
            #include <stdio.h>

            int main(void) {
                int8_t small = 0;
                int64_t exact = 0;
                int_least64_t least = 0;
                int_fast64_t fast = 0;
                int count = sscanf("-12 1234567890123 44 55",
                    "%" SCNd8 " %" SCNd64 " %" SCNdLEAST64 " %" SCNdFAST64,
                    &small, &exact, &least, &fast);
                printf("%d %d %" PRId64 " %" PRIdLEAST64 " %" PRIdFAST64 "\n",
                    count, small, exact, least, fast);
                return 0;
            }
        "#;
    assert_stdout(source, "4 -12 1234567890123 44 55\n");
}

#[test]
fn muse_whole_aggregate_copy_may_include_volatile_members() {
    let source = r#"
            struct S { volatile int member; int other; };
            union U { volatile int member; long other; };
            int main(void) {
                struct S a = {1, 2}, b = {0, 0};
                union U x = {.member = 3}, y = {.other = 0};
                b = a;
                y = x;
                return b.member != 1 || b.other != 2 || y.member != 3;
            }
        "#;
    assert_exit_status(source, 0);

    let hidden_volatile = r#"
            struct S { int member; };
            int main(void) {
                volatile struct S source = {1};
                struct S *nonvolatile = (struct S *)&source;
                struct S copy = *nonvolatile;
                return copy.member;
            }
        "#;
    assert_diagnostic_contains(hidden_volatile, "volatile-qualified object");
}
