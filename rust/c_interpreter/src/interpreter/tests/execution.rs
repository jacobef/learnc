use super::*;

#[test]
fn temporary_audit_public_api_conditional_inclusion() {
    use crate::{NativeExecutionOptions, run_native_source};
    let options = NativeExecutionOptions::default();
    let cases = [
        (
            "#if 0\n#ifdef\n#endif\n#endif\nint main(void) { return 0; }\n",
            "skipped nested #ifdef",
        ),
        (
            "#if 1\nint main(void) { return 0; }\n#else extra\n#endif\n",
            "#else extra",
        ),
        (
            "#if 1\nint main(void) { return 0; }\n#endif extra\n",
            "#endif extra",
        ),
        (
            "#if L'a' == 97\nint main(void) { return 0; }\n#else\nint main(void) { return 1; }\n#endif\n",
            "wide character in #if",
        ),
    ];
    for (source, label) in cases {
        let result = run_native_source("audit.c", source, &options);
        println!("{label}: {result:?}");
    }
}

#[test]
fn else_branch_executes() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                if (x) {
                    printf("then\n");
                } else {
                    printf("else\n");
                }
                return 0;
            }
        "#;
    assert_stdout(source, "else\n");
}

#[test]
fn runaway_recursion_reports_a_diagnostic_before_overflowing_the_host_stack() {
    let source = r#"
            int recurse(int n) {
                return recurse(n + 1);
            }

            int main(void) {
                return recurse(0);
            }
        "#;
    let rendered = run_source("test.c", source).unwrap_err().render();
    assert!(rendered.contains("function call depth exceeded the interpreter limit of 80"));
    assert!(rendered.contains("check for recursion that does not reach its base case"));
}

#[test]
fn deep_finite_recursion_is_independent_of_the_host_build_profile() {
    let source = r#"
            int descend(int n) {
                return n == 0 ? 0 : 1 + descend(n - 1);
            }

            int main(void) {
                return descend(63) != 63;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn ordinary_recursive_fibonacci_stays_below_the_call_depth_guard() {
    let source = r#"
            int fibonacci(int n) {
                return n < 2 ? n : fibonacci(n - 1) + fibonacci(n - 2);
            }

            int main(void) {
                return fibonacci(10) != 55;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn falling_off_nonvoid_function_is_allowed_if_result_is_ignored() {
    let source = r#"
            int f(void) {
            }

            int main(void) {
                f();
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn simple_mode_rejects_falling_off_nonvoid_function_when_result_is_ignored() {
    let source = r#"
            int f(void) {
            }

            int main(void) {
                f();
                return 0;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("control reached the end of non-void function f"));
    assert!(rendered.contains("caller uses the missing return value"));
}

#[test]
fn simple_mode_still_allows_implicit_return_from_main() {
    let source = r#"
            int main(void) {
            }
        "#;
    let result = run_source_with_options("test.c", source, &simple_ub_options()).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn using_result_of_nonvoid_function_that_falls_off_end_is_ub() {
    let source = r#"
            int f(void) {
            }

            int main(void) {
                int x = f();
                return x;
            }
        "#;
    assert_diagnostic_contains(source, "reached the end of a non-void function");
}

#[test]
fn short_circuit_and_avoids_dead_ub() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x;
                if (0 && x) {
                    printf("bad\n");
                }
                printf("ok\n");
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn short_circuit_or_avoids_dead_ub() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x;
                if (1 || x) {
                    printf("ok\n");
                }
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn comma_operator_sequences_side_effects() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                printf("%d\n", (x = 1, x));
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn pre_and_post_increment_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 4;
                int a = x++;
                int b = ++x;
                printf("%d %d %d\n", a, b, x);
                return 0;
            }
        "#;
    assert_stdout(source, "4 6 6\n");
}

#[test]
fn assignment_read_for_same_store_is_allowed() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                i = i + 1;
                printf("%d\n", i);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn comma_sequence_makes_simple_assignment_well_defined_but_not_compound_assignment() {
    let valid = "int main(void) { int i = 0; i = (i++, i); return i != 1; }";
    assert_eq!(run_source("test.c", valid).unwrap().exit_status, 0);

    let invalid = "int main(void) { int i = 0; i += (i++, 0); return i; }";
    assert_diagnostic_contains(invalid, "unsequenced");
}

#[test]
fn dereference_after_function_return_is_ub() {
    let source = r#"
            #include <stdio.h>
            int *leak(void) {
                int x = 1;
                return &x;
            }

            int main(void) {
                int *p = leak();
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "indeterminate pointer");
}

#[test]
fn switch_body_may_be_any_statement() {
    let source = r#"
            int classify(int value) {
                switch (value)
                    case 2: return 20;
                return 0;
            }

            int main(void) {
                return classify(2) != 20 || classify(3) != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn byte_copies_preserve_partial_scalar_representations() {
    let source = r#"
            #include <string.h>
            int main(void) {
                int value;
                unsigned char input = 42;
                unsigned char output = 0;
                memcpy(&value, &input, 1);
                memcpy(&output, &value, 1);
                return output != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn do_while_executes_body_before_test() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                do {
                    ++i;
                } while (i < 3);
                printf("%d\n", i);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn switch_case_default_and_fallthrough_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 2;
                switch (x) {
                    case 1:
                        printf("a");
                        break;
                    case 2:
                    case 3:
                        printf("b");
                        break;
                    default:
                        printf("c");
                }
                printf("\n");
                return 0;
            }
        "#;
    assert_stdout(source, "b\n");
}

#[test]
fn switch_dispatch_enters_nested_case_labels() {
    for source in [
        r#"
                int main(void) {
                    switch (1) {
                        if (0) { case 1: return 0; }
                    }
                    return 1;
                }
            "#,
        r#"
                int main(void) {
                    int x = 0;
                    switch (1) {
                        { x = 9; case 1: x++; }
                    }
                    return x != 1;
                }
            "#,
        r#"
                int main(void) {
                    int x = 0;
                    switch (1) {
                        while (x < 2) { case 1: x++; }
                    }
                    return x != 2;
                }
            "#,
    ] {
        let result = run_source("test.c", source).unwrap();
        assert_eq!(result.exit_status, 0);
    }
}

#[test]
fn nested_case_labels_are_transparent_during_normal_fallthrough() {
    let source = r#"
            int main(void) {
                switch (1) {
                    case 1:
                        if (1) { case 2: return 0; }
                }
                return 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn switch_cannot_enter_scope_of_variably_modified_object() {
    let source = r#"
            int main(void) {
                int n = 2;
                switch (1) {
                    int values[n];
                    case 1: return 0;
                }
                return 1;
            }
        "#;
    assert_diagnostic_contains(source, "variably modified type");
}

#[test]
fn duplicate_switch_labels_are_rejected() {
    for source in [
        "int main(void) { switch (0) { case 1: return 1; case 1: return 2; } return 0; }\n",
        "int main(void) { switch (0) { default: return 1; default: return 2; } }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("duplicate case value")
                || rendered.contains("multiple default labels"),
            "unexpected diagnostic: {rendered}"
        );
    }
}

#[test]
fn switch_converts_case_values_to_the_promoted_control_type() {
    let source = r#"
            int main(void) {
                unsigned int control = 4294967295U;
                switch (control) {
                    case -1: break;
                    default: return 1;
                }
                unsigned long wide_control = ~0UL;
                switch (wide_control) {
                    case ~0UL: return 0;
                    default: return 2;
                }
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn switch_constant_expressions_short_circuit_and_keep_conditional_types() {
    let source = r#"
            int main(void) {
                switch (1) {
                    case 1 || (1 / 0): break;
                    default: return 1;
                }

                switch (4) {
                    case sizeof(int): break;
                    default: return 2;
                }

                switch (4294967295U) {
                    case 1 ? -1 : 0U: return 0;
                    default: return 3;
                }
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn nested_switches_have_independent_case_and_default_labels() {
    let source = r#"
            int main(void) {
                switch (0) {
                    case 1:
                        switch (0) {
                            case 1: return 1;
                            default: break;
                        }
                        break;
                    default: break;
                }
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn switch_uses_default_when_no_case_matches() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                switch (7) {
                    case 1:
                        printf("bad");
                        break;
                    default:
                        printf("ok\n");
                }
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn negating_minimum_long_long_is_ub() {
    let source = r#"
            int main(void) {
                long long minimum = -9223372036854775807LL - 1LL;
                return -minimum;
            }
        "#;
    assert_diagnostic_contains(source, "signed integer overflow");
}

#[test]
fn conditional_compilation_with_defined_and_elif_works() {
    let source = r#"
            #include <stdio.h>
            #define VALUE 3

            #if defined(VALUE) && VALUE == 3
            #define RESULT 7
            #elif VALUE == 4
            #define RESULT 9
            #else
            #define RESULT 11
            #endif

            int main(void) {
                printf("%d\n", RESULT);
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
}

#[test]
fn duplicate_or_mismatched_tags_in_one_scope_are_rejected() {
    for source in [
        "struct A { int x; }; struct A { int y; }; int main(void) { return 0; }\n",
        "union A { int x; }; union A { int y; }; int main(void) { return 0; }\n",
        "enum A { x }; enum A { y }; int main(void) { return 0; }\n",
        "struct A; union A; int main(void) { return 0; }\n",
        "enum A { x }; struct A; int main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("already complete")
                || rendered.contains("previously declared with a different kind"),
            "unexpected diagnostic: {rendered}"
        );
    }
}

#[test]
fn multiple_input_files_are_linked_together() {
    let project = TestProject::new("multi-input");
    project.write("main.c",
            "#include <stdio.h>\nint helper(void);\nint main(void) { printf(\"%d\\n\", helper()); return 0; }\n",
        )
        .unwrap();
    project
        .write("helper.c", "int helper(void) { return 7; }\n")
        .unwrap();

    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "7\n");
}

#[test]
fn local_static_object_persists_across_calls() {
    let source = r#"
            #include <stdio.h>
            int f(void) {
                static int x;
                x++;
                return x;
            }

            int main(void) {
                printf("%d %d\n", f(), f());
                return 0;
            }
        "#;
    assert_stdout(source, "1 2\n");
}

#[test]
fn static_functions_with_same_name_in_different_files_do_not_conflict() {
    let project = TestProject::new("static-functions");
    project
        .write(
            "main.c",
            r#"
                #include <stdio.h>

                int f(void);

                static int helper(void) {
                    return 2;
                }

                int main(void) {
                    printf("%d %d\n", f(), helper());
                    return 0;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                static int helper(void) {
                    return 1;
                }

                int f(void) {
                    return helper();
                }

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "1 2\n");
}

#[test]
fn static_globals_with_same_name_in_different_files_do_not_conflict() {
    let project = TestProject::new("static-globals");
    project
        .write(
            "main.c",
            r#"
                #include <stdio.h>

                int f(void);
                static int x = 2;

                int g(void) {
                    return x;
                }

                int main(void) {
                    printf("%d %d\n", f(), g());
                    return 0;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                static int x = 1;

                int f(void) {
                    return x;
                }

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "1 2\n");
}

#[test]
fn duplicate_type_specifiers_are_rejected_but_long_long_is_allowed() {
    for specifiers in [
        "void void",
        "_Bool _Bool",
        "char char",
        "float float",
        "double double",
        "int int",
        "short short",
        "signed signed",
        "unsigned unsigned",
    ] {
        let source = format!("int main(void) {{ {specifiers} value; return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render().contains("duplicate type specifier"),
            "unexpected diagnostic for {specifiers}: {}",
            err.render()
        );
    }

    run_source(
        "test.c",
        "int main(void) { long long value = 0; return (int)value; }\n",
    )
    .unwrap();

    let err = run_source(
        "test.c",
        "int main(void) { _Bool _Complex value = 0; return 0; }\n",
    )
    .unwrap_err();
    assert!(
        err.render()
            .contains("_Bool cannot be combined with other type specifiers")
    );

    let err = run_source(
        "test.c",
        "int main(void) { _Complex value = 0; return 0; }\n",
    )
    .unwrap_err();
    assert!(err.render().contains("_Complex requires float or double"));
}

#[test]
fn duplicate_storage_class_specifiers_are_rejected() {
    for specifiers in [
        "auto auto int",
        "extern extern int",
        "register register int",
        "static static int",
        "typedef typedef int",
    ] {
        let source = format!("int main(void) {{ {specifiers} value; return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render()
                .contains("multiple storage class specifiers are not allowed"),
            "unexpected diagnostic for {specifiers}: {}",
            err.render()
        );
    }
}

#[test]
fn block_extern_object_shadows_a_local_and_refers_to_the_external_object() {
    let source = r#"
            int value = 9;

            int main(void) {
                int value = 2;
                {
                    extern int value;
                    return value - 9;
                }
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn goto_forward_jump_reaches_later_label() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                goto out;
                printf("bad\n");
            out:
                printf("ok\n");
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn goto_backward_jump_reaches_earlier_label() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
            loop:
                i++;
                if (i < 3) {
                    goto loop;
                }
                printf("%d\n", i);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn handled_goto_does_not_resume_after_the_original_goto_statement() {
    let source = r#"
            int main(void) {
                int x = 0;
                {
                    goto label;
                    x = 100;
                label:
                    x++;
                }
                return x != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn goto_cannot_enter_scope_of_variably_modified_object() {
    let source = r#"
            int main(void) {
                int n = 2;
                goto label;
                int values[n];
            label:
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "variably modified type");
}

#[test]
fn goto_can_enter_a_nested_if_branch() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int jumped = 0;
                if (0) {
                label:
                    x = 5;
                }
                if (!jumped) {
                    jumped = 1;
                    goto label;
                }
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
}

#[test]
fn goto_can_enter_a_while_body() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                int jumped = 0;
                while (i < 3) {
                label:
                    i++;
                    if (i < 3) {
                        continue;
                    }
                    break;
                }
                if (!jumped) {
                    jumped = 1;
                    goto label;
                }
                printf("%d\n", i);
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn duplicate_labels_are_rejected() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
            label:
                printf("a\n");
            label:
                printf("b\n");
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "duplicate label label");
}

#[test]
fn atomic_qualified_objects_support_single_threaded_c11_operations() {
    let source = r#"
            struct Pair { int first; int second; };

            int main(void) {
                _Atomic int value = 0;
                int assigned = (value = 3);
                value += 2;
                ++value;
                _Atomic(int) other = value;
                _Atomic struct Pair pair = {1, 2};
                struct Pair copy = pair;
                copy.second = 9;
                pair = copy;
                copy = pair;
                return assigned != 3 || value != 6 || other != 6
                    || copy.first != 1 || copy.second != 9
                    || sizeof pair.first != sizeof(int);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn atomic_and_imaginary_spellings_are_keywords() {
    for source in [
        "int main(void) { int _Atomic = 0; return _Atomic; }\n",
        "int main(void) { int _Imaginary = 0; return _Imaginary; }\n",
    ] {
        assert!(run_source("test.c", source).is_err());
    }
    assert!(
        run_source(
            "test.c",
            "_Atomic(void) bad; int main(void) { return 0; }\n"
        )
        .is_err()
    );
}

#[test]
fn string_and_char_escapes_support_hex_octal_and_universal_forms() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char text[] = "\x41\101\n";
                printf("%d %d %d %u\n", text[0], text[1], text[2], U'\u03a9');
                return 0;
            }
        "#;
    assert_stdout(source, "65 65 10 937\n");
}

#[test]
fn invalid_escape_sequences_are_rejected() {
    for literal in [r#""\8""#, r#""\400""#, r#"'\8'"#, r#"'\400'"#] {
        let source = format!("int main(void) {{ (void)({literal}); return 0; }}\n");
        assert!(run_source("test.c", &source).is_err(), "accepted {literal}");
    }

    for literal in [r#""\u0061""#, r#"'\U00000061'"#] {
        let source = format!("int main(void) {{ (void)({literal}); return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(err.render().contains("invalid universal character name"));
    }

    for condition in [r#"'\400'"#, r#"'\u0061'"#] {
        let source = format!(
            "#if {condition}\nint main(void) {{ return 0; }}\n#else\nint main(void) {{ return 1; }}\n#endif\n"
        );
        assert!(
            run_source("test.c", &source).is_err(),
            "accepted #if {condition}"
        );
    }
}

#[test]
fn variadic_function_with_stdarg_utilities_works() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>

            int sum(int count, ...) {
                va_list ap;
                va_start(ap, count);
                int total = 0;
                while (count) {
                    total += va_arg(ap, int);
                    count--;
                }
                va_end(ap);
                return total;
            }

            int main(void) {
                printf("%d\n", sum(4, 1, 2, 3, 4));
                return 0;
            }
        "#;
    assert_stdout(source, "10\n");
}

#[test]
fn va_copy_clones_the_cursor() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>

            int test(int count, ...) {
                va_list ap;
                va_list copy;
                va_start(ap, count);
                va_copy(copy, ap);
                int a = va_arg(ap, int);
                int b = va_arg(copy, int);
                va_end(copy);
                va_end(ap);
                return a + b;
            }

            int main(void) {
                printf("%d\n", test(2, 7, 11));
                return 0;
            }
        "#;
    assert_stdout(source, "14\n");
}

#[test]
fn va_arg_type_mismatch_is_ub() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>

            int bad(int count, ...) {
                va_list ap;
                va_start(ap, count);
                return va_arg(ap, char);
            }

            int main(void) {
                printf("%d\n", bad(1, 7));
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "va_arg requested char");

    let qualified = r#"
            #include <stdarg.h>
            int bad(int count, ...) {
                va_list ap;
                va_start(ap, count);
                return va_arg(ap, const int);
            }
            int main(void) { return bad(1, 7); }
        "#;
    assert_diagnostic_contains(qualified, "va_arg requested const int");
}

#[test]
fn va_end_rejects_an_already_ended_list() {
    let source = r#"
            #include <stdarg.h>
            int bad(int last, ...) {
                va_list ap;
                va_start(ap, last);
                va_end(ap);
                va_end(ap);
                return 0;
            }
            int main(void) { return bad(1, 2); }
        "#;
    assert_diagnostic_contains(source, "va_list that is not active");
}

#[test]
fn va_copy_rejects_an_ended_source_and_active_destination() {
    for source in [
        r#"
                #include <stdarg.h>
                int bad(int last, ...) {
                    va_list source;
                    va_list destination;
                    va_start(source, last);
                    va_end(source);
                    va_copy(destination, source);
                    return 0;
                }
                int main(void) { return bad(1, 2); }
            "#,
        r#"
                #include <stdarg.h>
                int bad(int last, ...) {
                    va_list source;
                    va_list destination;
                    va_start(source, last);
                    va_copy(destination, source);
                    va_copy(destination, source);
                    return 0;
                }
                int main(void) { return bad(1, 2); }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(rendered.contains("ended source") || rendered.contains("active destination"));
    }
}

#[test]
fn returning_without_va_end_is_ub() {
    let source = r#"
            #include <stdarg.h>
            int bad(int last, ...) {
                va_list ap;
                va_start(ap, last);
                return va_arg(ap, int);
            }
            int main(void) { return bad(1, 2); }
        "#;
    assert_diagnostic_contains(source, "without a matching va_end");
}

#[test]
fn va_end_must_run_in_the_function_that_started_or_copied_the_list() {
    let source = r#"
            #include <stdarg.h>
            void finish(va_list *list) { va_end(*list); }
            int bad(int last, ...) {
                va_list list;
                va_start(list, last);
                finish(&list);
                return 0;
            }
            int main(void) { return bad(1, 2); }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(err.render().contains("same function"), "{}", err.render());
}

#[test]
fn va_arg_allows_the_standard_compatible_type_exceptions() {
    let source = r#"
            #include <stdarg.h>
            int signed_exception(int last, ...) {
                va_list ap;
                va_start(ap, last);
                int value = va_arg(ap, int);
                va_end(ap);
                return value;
            }
            int pointer_exception(int last, ...) {
                va_list ap;
                va_start(ap, last);
                char *value = va_arg(ap, char *);
                va_end(ap);
                return *value;
            }
            int main(void) {
                char ch = 9;
                void *pointer = &ch;
                return signed_exception(0, 7U) + pointer_exception(0, pointer) - 16;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn va_arg_signedness_exception_requires_a_representable_value() {
    let source = r#"
            #include <stdarg.h>
            int bad(int last, ...) {
                va_list ap;
                va_start(ap, last);
                unsigned int value = va_arg(ap, unsigned int);
                va_end(ap);
                return value;
            }
            int main(void) { return bad(0, -1); }
        "#;
    assert_diagnostic_contains(source, "va_arg requested unsigned int");
}

#[test]
fn quoted_standard_header_falls_back_to_angle_header_lookup() {
    let source = r#"
            #include "stdio.h"
            int main(void) {
                printf("ok\n");
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn defined_cannot_be_defined_or_undefined() {
    assert_diagnostic_contains(
        "#define defined 1\nint main(void) { return 0; }\n",
        "defined",
    );
    assert_diagnostic_contains("#undef defined\nint main(void) { return 0; }\n", "defined");
}

#[test]
fn run_result_preserves_main_return_status() {
    let source = r#"
            int main(void) {
                return 7;
            }
        "#;
    assert_exit_status(source, 7);
}

#[test]
fn cboxes_expression_eval_uses_trace_state() {
    let source = r#"
            int main(void) {
                int x;
                x = 7;
                return 0;
            }
        "#;
    let mut normalized = source.to_owned();
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    let baseline =
        crate_run_source_with_options("test.c", normalized.clone(), &RunOptions::default())
            .unwrap();
    let event_index = baseline
        .trace
        .iter()
        .position(|event| {
            event
                .state
                .iter()
                .any(|item| item.name == "x" && item.value == "7")
        })
        .expect("assignment trace event should exist");
    let result = crate_run_source_with_options(
        "test.c",
        normalized,
        &RunOptions {
            expression_eval: Some(RunExpressionEvalRequest {
                expression: "x + 1".to_owned(),
                event_index,
            }),
            ..RunOptions::default()
        },
    )
    .unwrap();
    let expression = result.expression.expect("expression result should exist");
    assert_eq!(expression.kind, "rvalue");
    assert_eq!(expression.ty, "int");
    assert_eq!(expression.value, "8");
}

#[test]
fn stdio_header_exposes_ssize_t_like_the_host_headers() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                ssize_t value = 4;
                return value - 4;
            }
        "#;
    assert_stdout(source, "");
}

#[test]
fn external_object_and_function_with_same_name_conflict() {
    let project = TestProject::new("object-function-conflict");
    project
        .write(
            "main.c",
            r#"
                int f;

                int main(void) {
                    return 0;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                int f(void) {
                    return 0;
                }

            "#,
        )
        .unwrap();
    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("both a object and an function"));
}

#[test]
fn string_search_and_span_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char text[] = "banana";
                void *m = memchr(text, 'n', 6);
                char *first_n = (char *)m;
                char *term = strchr(text, 0);
                char *last_a = strrchr(text, 'a');
                char *set = strpbrk(text, "xyzn");
                char *needle = strstr(text, "ana");
                printf("%c %d %c %c %s %lu %lu\n",
                    *first_n,
                    term[0] == 0,
                    *last_a,
                    *set,
                    needle,
                    strspn("aaab", "ab"),
                    strcspn("abc123", "123"));
                return 0;
            }
        "#;
    assert_stdout(source, "n 1 a n anana 4 3\n");
}

#[test]
fn string_concatenation_and_comparison_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                char text[16] = "ab";
                strcat(text, "cd");
                strncat(text, "wxyz", 2);
                printf("%s %d %d\n",
                    &text[0],
                    strncmp(text, "abcdwx", 6) == 0,
                    strncmp(text, "abcdwy", 6) == 0);
                return 0;
            }
        "#;
    assert_stdout(source, "abcdwx 1 0\n");
}

#[test]
fn lgamma_updates_signgam_binding() {
    let source = r#"
            #include <stdio.h>
            #include <math.h>

            int main(void) {
                lgamma(-0.5);
                printf("%d\n", signgam);
                return 0;
            }
        "#;
    assert_stdout(source, "-1\n");
}

#[test]
fn stddef_header_types_and_offsetof_work() {
    #[repr(C)]
    struct HostLayout {
        head: libc::c_char,
        value: libc::c_int,
        tail: [libc::c_ushort; 3],
    }

    let uninit = std::mem::MaybeUninit::<HostLayout>::uninit();
    let base = uninit.as_ptr();
    let tail_offset = unsafe { std::ptr::addr_of!((*base).tail[2]) as usize - base as usize };

    let source = r#"
            #include <stdio.h>
            #include <stddef.h>

            struct S {
                char head;
                int value;
                unsigned short tail[3];
            };

            int main(void) {
                printf("%lu %lu %lu %lu %d\n",
                    sizeof(ptrdiff_t),
                    sizeof(size_t),
                    sizeof(wchar_t),
                    offsetof(struct S, tail[2]),
                    NULL == (void *)0);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(
        result.stdout,
        format!(
            "{} {} {} {} 1\n",
            std::mem::size_of::<isize>(),
            std::mem::size_of::<libc::size_t>(),
            std::mem::size_of::<libc::wchar_t>(),
            tail_offset
        )
    );
}

#[test]
fn errno_is_zero_at_the_start_of_each_execution() {
    assert_exit_status(
        r#"
            #include <errno.h>
            #include <limits.h>
            #include <stdlib.h>
            int main(void) {
                (void)strtol("999999999999999999999999999999", 0, 10);
                return errno != ERANGE;
            }
        "#,
        0,
    );
    assert_exit_status(
        r#"
            #include <errno.h>
            int main(void) {
                return errno != 0;
            }
        "#,
        0,
    );
}

#[test]
fn repeated_assert_header_inclusion_tracks_ndebug() {
    let disabled = r#"
            #include <assert.h>
            #define NDEBUG
            #include <assert.h>
            int main(void) {
                assert(0);
                return 0;
            }
        "#;
    assert_exit_status(disabled, 0);

    let enabled = r#"
            #define NDEBUG
            #include <assert.h>
            #undef NDEBUG
            #include <assert.h>
            int main(void) {
                assert(0);
                return 0;
            }
        "#;
    assert_diagnostic_contains(enabled, "assertion failed");
}

#[test]
fn errno_is_a_persistent_modifiable_lvalue() {
    let source = r#"
            #include <errno.h>
            int main(void) {
                errno = 17;
                errno += 1;
                return errno != 18;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn invalid_wctrans_descriptor_is_ub() {
    let source = r#"
            #include <wctype.h>

            int main(void) {
                wctrans_t lower = wctrans("definitely_not_valid");
                return towctrans('A', lower);
            }
        "#;
    assert_diagnostic_contains(source, "invalid or locale-stale");
}

#[test]
fn ctime_keeps_prior_localtime_result_live() {
    let source = r#"
            #include <time.h>

            int main(void) {
                time_t t = time(0);
                struct tm *tm = localtime(&t);
                ctime(&t);
                (void)tm->tm_sec;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn localtime_keeps_prior_gmtime_result_live() {
    let source = r#"
            #include <time.h>

            int main(void) {
                time_t t = time(0);
                struct tm *utc = gmtime(&t);
                localtime(&t);
                (void)utc->tm_sec;
                utc->tm_sec = 0;
                return utc->tm_sec;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn asctime_keeps_prior_results_live_and_writable() {
    let source = r#"
            #include <time.h>

            int main(void) {
                time_t t = time(0);
                struct tm *tm = localtime(&t);
                char *first = asctime(tm);
                asctime(tm);
                first[0] = 'X';
                return first[0] != 'X';
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn fe_all_except_is_exactly_the_defined_exception_mask() {
    let source = r#"
            #include <fenv.h>
            int main(void) {
                return FE_ALL_EXCEPT != (FE_DIVBYZERO | FE_INEXACT | FE_INVALID |
                                         FE_OVERFLOW | FE_UNDERFLOW);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn fe_dfl_env_restores_the_environment_from_program_startup() {
    let source = r#"
            #include <fenv.h>
            int main(void) {
                int initial = fegetround();
                if (fesetround(FE_UPWARD) != 0) return 1;
                if (fegetround() != FE_UPWARD) return 2;
                if (fesetenv(FE_DFL_ENV) != 0) return 3;
                return fegetround() != initial;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn fenv_rejects_invalid_exception_mask_bits() {
    let source = r#"
            #include <fenv.h>

            int main(void) {
                feclearexcept(0x80);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unsupported bits");
}

#[test]
fn wide_formatted_output_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <stdarg.h>
            #include <wchar.h>

            int render(wchar_t *buf, unsigned long n, const wchar_t *fmt, ...) {
                va_list ap;
                va_start(ap, fmt);
                int out = vswprintf(buf, n, fmt, ap);
                va_end(ap);
                return out;
            }

            int main(void) {
                wchar_t buf[16];
                int count = render(buf, 16, L"%lc%ls %d", L'X', L"ok", 7);
                wprintf(L"%ls\n", L"hi");
                printf("%d %d %d %d\n", count, buf[0], buf[2], buf[4]);
                return 0;
            }
        "#;
    assert_stdout(source, "hi\n5 88 107 55\n");
}

#[test]
fn nonempty_source_file_without_trailing_newline_is_rejected() {
    let err = crate_run_source("test.c", "int main(void) { return 0; }").unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("must end in a newline character"));
    assert!(rendered.contains("test.c:1:29"), "{rendered}");
}

#[test]
fn volatile_object_access_through_nonvolatile_lvalue_is_ub() {
    let source = r#"
            int main(void) {
                volatile int x = 0;
                int *p = (int *)&x;
                return *p;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(
        rendered.contains("volatile-qualified object or subobject through a non-volatile lvalue"),
        "{rendered}"
    );
}

#[test]
fn zero_count_wide_writes_do_not_modify_the_destination() {
    for call in [
        "wmemcpy((wchar_t *)&constant, &source, 0)",
        "wmemmove((wchar_t *)&constant, &source, 0)",
        "wmemset((wchar_t *)&constant, 0, 0)",
        "wcsncpy((wchar_t *)&constant, &source, 0)",
    ] {
        let source = format!(
            r#"
                    #include <wchar.h>
                    int main(void) {{
                        const wchar_t constant = 7;
                        wchar_t source = 0;
                        {call};
                    }}
                "#
        );
        assert_eq!(run_source("test.c", &source).unwrap().exit_status, 0);
    }
}

#[test]
fn constraints_are_checked_in_unreachable_and_uncalled_code() {
    let sources = [
        "int main(void) { if (0) { int *p = 1; } return 0; }",
        "int bad(void) { int value = 0; value(); return 0; } int main(void) { return 0; }",
        "int bad(void) { int value; return &value; } int main(void) { return 0; }",
        "int main(void) { if (0) { break; } return 0; }",
        "int main(void) { if (0) { struct S { int x; } a, b; return a == b; } return 0; }",
        "int main(void) { if (0) { void *p = 0; p + 1; } return 0; }",
        "int main(void) { if (0) { struct S { int x; } a, b; (struct S)a; } return 0; }",
        "int main(void) { if (0) { int values[2] = { \"wrong\" }; } return 0; }",
        "int main(void) { if (0) { struct S { int value; } s = { \"wrong\" }; } return 0; }",
        "int main(void) { goto target; int n = 2; int values[n]; target: return 0; }",
        "int main(void) { switch (0) { int n = 2; int values[n]; case 0: return 0; } }",
        "int values[2] = {}; int main(void) { return 0; }",
        "int values[0]; int main(void) { return 0; }",
        "struct S { int count; int values[]; }; int main(void) { if (0) { struct S s = { 1, { 2 } }; } return 0; }",
        "struct S; int main(void) { if (0) { struct S value; } return 0; }",
        "int main(void) { if (0) { void value; } return 0; }",
        "struct S; int f(struct S value); int main(void) { return 0; }",
        "struct S; struct S f(void) { } int main(void) { return 0; }",
        "int f(void value); int main(void) { return 0; }",
        "extern void value; int main(void) { return 0; }",
        "struct S; extern struct S values[2]; int main(void) { return 0; }",
    ];
    for source in sources {
        assert!(run_source("test.c", source).is_err(), "accepted {source}");
    }
}

#[test]
fn function_arguments_require_complete_object_types() {
    let source = "void accept(int fixed, ...) {} int main(void) { accept(7, (void)0); }";
    assert_diagnostic_contains(source, "function arguments must have complete object type");
}

#[test]
fn c11_library_additions_work() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
void finish(void) { _Exit(7); }
int main(void) {
    FILE *first = fopen("exclusive", "wx");
    FILE *second = fopen("exclusive", "wx");
    struct timespec ts;
    void *memory = aligned_alloc(64, 128);
    void *small = aligned_alloc(4, 4);
    if (!first || second || timespec_get(&ts, TIME_UTC) != TIME_UTC) return 1;
    if (!memory || !small || (unsigned long)memory % 64 || (unsigned long)small % 4 || ts.tv_nsec < 0 || ts.tv_nsec >= 1000000000L) return 2;
    free(memory);
    free(small);
    if (at_quick_exit(finish)) return 3;
    quick_exit(4);
}
"#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 7);
}

#[test]
fn saved_library_state_objects_require_valid_provenance() {
    let cases = [
        r#"
                #include <fenv.h>
                int main(void) {
                    fenv_t environment = {0};
                    return fesetenv(&environment);
                }
            "#,
        r#"
                #include <stdio.h>
                int main(void) {
                    FILE *first = tmpfile();
                    FILE *second = tmpfile();
                    fpos_t position;
                    fgetpos(first, &position);
                    return fsetpos(second, &position);
                }
            "#,
        r#"
                #include <setjmp.h>
                #include <string.h>
                int main(void) {
                    jmp_buf original, copy;
                    if (setjmp(original) == 0) {
                        memcpy(&copy[0], &original[0], sizeof original);
                        longjmp(copy, 1);
                    }
                    return 0;
                }
            "#,
    ];
    for source in cases {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert_user_code_library_diag(&rendered, "requires");
    }
}

#[test]
fn descriptor_and_format_preconditions_are_enforced() {
    let cases = [
        r#"
                #include <wctype.h>
                int main(void) { return iswctype(L'a', (wctype_t)1); }
            "#,
        r#"
                #include <time.h>
                int main(void) {
                    char output[8];
                    struct tm value = {0};
                    return (int)strftime(output, sizeof output, "%Q", &value);
                }
            "#,
        r#"
                #include <time.h>
                int main(void) {
                    struct tm value = {0};
                    value.tm_sec = 61;
                    return asctime(&value) == 0;
                }
            "#,
    ];
    for source in cases {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert!(rendered.contains("undefined behavior"), "{rendered}");
    }
}

#[test]
fn sort_and_search_comparator_preconditions_are_enforced() {
    let cyclic = r#"
            #include <stdlib.h>
            int compare(const void *left, const void *right) {
                int a = *(const int *)left;
                int b = *(const int *)right;
                if (a == b) return 0;
                return (a + 1) % 3 == b ? -1 : 1;
            }
            int main(void) {
                int values[3] = {0, 1, 2};
                qsort(values, 3, sizeof values[0], compare);
            }
        "#;
    let rendered = run_source("test.c", cyclic).unwrap_err().render();
    assert_user_code_library_diag(&rendered, "cyclic");

    let unsorted = r#"
            #include <stdlib.h>
            int compare(const void *left, const void *right) {
                int a = *(const int *)left;
                int b = *(const int *)right;
                return (a > b) - (a < b);
            }
            int main(void) {
                int key = 3;
                int values[3] = {1, 3, 2};
                return bsearch(&key, values, 3, sizeof values[0], compare) == 0;
            }
        "#;
    let rendered = run_source("test.c", unsorted).unwrap_err().render();
    assert_user_code_library_diag(&rendered, "bsearch array");
}

#[test]
fn sqlite_utf8_and_hash_subroutines_match_edge_cases() {
    // Extracted from SQLite src/utf.c and src/hash.c. The SQLite typedefs
    // and EBCDIC build switch are reduced to their portable C11 forms.
    let source = r#"
            #include <stdint.h>

            typedef unsigned char u8;
            typedef uint32_t u32;

            static const unsigned char sqlite3Utf8Trans1[] = {
              0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
              0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
              0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
              0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
              0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
              0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
              0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
              0x00, 0x01, 0x02, 0x03, 0x00, 0x01, 0x00, 0x00,
            };

            u32 sqlite3Utf8Read(const unsigned char **pz){
              unsigned int c;
              c = *((*pz)++);
              if( c>=0xc0 ){
                c = sqlite3Utf8Trans1[c-0xc0];
                while( (*(*pz) & 0xc0)==0x80 ){
                  c = (c<<6) + (0x3f & *((*pz)++));
                }
                if( c<0x80
                    || (c&0xFFFFF800)==0xD800
                    || (c&0xFFFFFFFE)==0xFFFE ){  c = 0xFFFD; }
              }
              return c;
            }

            static unsigned int strHash(const char *z){
              unsigned int h = 0;
              while( z[0] ){
                h += 0xdf & (unsigned char)*(z++);
                h *= 0x9e3779b1;
              }
              return h;
            }

            static int checkUtf8(const unsigned char *input, u32 expected,
                                 unsigned long expectedBytes){
              const unsigned char *next = input;
              u32 value = sqlite3Utf8Read(&next);
              return value==expected && (unsigned long)(next-input)==expectedBytes;
            }

            int main(void){
              static const unsigned char ascii[] = {'A',0};
              static const unsigned char cent[] = {0xc2,0xa2,0};
              static const unsigned char euro[] = {0xe2,0x82,0xac,0};
              static const unsigned char grin[] = {0xf0,0x9f,0x98,0x80,0};
              static const unsigned char overlong[] = {0xc0,0x80,0};
              static const unsigned char surrogate[] = {0xed,0xa0,0x80,0};
              static const unsigned char noncharacter[] = {0xef,0xbf,0xbe,0};
              static const unsigned char continuation[] = {0x80,0};

              if( !checkUtf8(ascii, 0x41, 1) ) return 1;
              if( !checkUtf8(cent, 0xa2, 2) ) return 2;
              if( !checkUtf8(euro, 0x20ac, 3) ) return 3;
              if( !checkUtf8(grin, 0x1f600, 4) ) return 4;
              if( !checkUtf8(overlong, 0xfffd, 2) ) return 5;
              if( !checkUtf8(surrogate, 0xfffd, 3) ) return 6;
              if( !checkUtf8(noncharacter, 0xfffd, 3) ) return 7;
              if( !checkUtf8(continuation, 0x80, 1) ) return 8;

              if( strHash("SQLite") != strHash("sqlite") ) return 9;
              if( strHash("SQLite") == strHash("sqlite3") ) return 10;
              if( strHash("") != 0 ) return 11;
              return 0;
            }
        "#;
    let result = run_source("sqlite-utf8-hash.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn reported_invalid_stdc_pragma_is_diagnosed() {
    let source = "#pragma STDC FP_CONTRACT MAYBE\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "invalid #pragma STDC directive");
    let operator = "_Pragma(\"STDC FP_CONTRACT MAYBE\")\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(operator, "invalid #pragma STDC directive");
}

#[test]
fn reported_mixed_case_long_long_suffix_is_rejected() {
    assert_diagnostic_contains(
        "int main(void) { return 1lL == 1; }\n",
        "integer literal suffix",
    );
    assert_diagnostic_contains(
        "int main(void) { return 1Ll == 1; }\n",
        "integer literal suffix",
    );
    assert_exit_status(
        "int main(void) { return !(1ll == 1LL && 1uLL == 1ULL && 1LLu == 1LLU); }\n",
        0,
    );
}

#[test]
fn reported_duplicate_converted_case_is_diagnosed_when_function_is_uncalled() {
    let source = r#"
            int bad(void) {
                switch (0U) {
                    case -1: ;
                    case 4294967295U: ;
                }
                return 0;
            }
            int main(void) { return 0; }
        "#;
    assert_diagnostic_contains(source, "duplicate case value");
}

#[test]
fn reported_va_arg_compatible_types_are_accepted() {
    let source = r#"
            #include <stdarg.h>

            static int target(int value) { return value + 1; }

            static int qualified(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                int (*fn)(const int) = va_arg(ap, int (*)(const int));
                va_end(ap);
                return fn(4) != 5;
            }

            static int old_style(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                int (*fn)() = va_arg(ap, int (*)());
                va_end(ap);
                return fn(4) != 5;
            }

            static void consume(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                (void)va_arg(ap, va_list);
                va_end(ap);
            }

            static void produce(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                consume(0, ap);
                va_end(ap);
            }

            int main(void) {
                produce(0, 42);
                return qualified(0, target) || old_style(0, target);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn muse_goto_within_for_scope_preserves_the_existing_for_object() {
    let source = r#"
            int main(void) {
                for (int value = 7; ; )
                    if (value) goto inside;
                    else inside: return value;
            }
        "#;
    assert_exit_status(source, 7);
}
