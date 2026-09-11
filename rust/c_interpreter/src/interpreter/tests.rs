use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    RunExpressionEvalRequest, RunOptions, RunResult, UbDetectionMode, diag::Diagnostic,
    run_file as crate_run_file, run_files as crate_run_files,
    run_files_with_options as crate_run_files_with_options, run_source as crate_run_source,
    run_source_with_options as crate_run_source_with_options,
};

fn run_source(path: &str, source: &str) -> Result<RunResult, Diagnostic> {
    let mut normalized = source.to_owned();
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    crate_run_source(path, &normalized)
}

fn run_source_with_options(
    path: &str,
    source: &str,
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    let mut normalized = source.to_owned();
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    crate_run_source_with_options(path, &normalized, options)
}

fn assert_stdout(source: &str, expected: &str) {
    assert_eq!(run_source("test.c", source).unwrap().stdout, expected);
}

fn assert_exit_status(source: &str, expected: i32) {
    assert_eq!(run_source("test.c", source).unwrap().exit_status, expected);
}

fn rendered_diagnostic(source: &str) -> String {
    run_source("test.c", source).unwrap_err().render()
}

fn assert_diagnostic_contains(source: &str, expected: &str) {
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains(expected), "{rendered}");
}

#[test]
fn standard_example_mktime_ignores_output_fields() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/mktime_ignores_output_fields.c"),
        0,
    );
}

#[test]
fn standard_example_long_double_math_uses_the_interpreted_format() {
    assert_exit_status(
        include_str!(
            "../../tests/standard_examples/long_double_math_uses_the_interpreted_format.c"
        ),
        0,
    );
}

#[test]
fn host_abi_preserves_interpreted_long_width() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/host_abi_preserves_interpreted_long_width.c"),
        0,
    );
}

#[test]
fn standard_example_vla_parameters_retain_inner_dimensions() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/vla_parameters_retain_inner_dimensions.c"),
        0,
    );
}

#[test]
fn standard_example_transform_sizing_null_exception_is_narrow() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/transform_sizing_null_exception_is_narrow.c"),
        0,
    );
    for call in [
        "strxfrm(NULL, \"x\", 1)",
        "strxfrm(NULL, NULL, 0)",
        "wcsxfrm(NULL, L\"x\", 1)",
        "wcsxfrm(NULL, NULL, 0)",
        "strncpy(NULL, \"x\", 0)",
    ] {
        assert_diagnostic_contains(
            &format!("#include <string.h>\n#include <wchar.h>\nint main(void) {{ {call}; }}\n"),
            "undefined behavior",
        );
    }
}

#[test]
fn standard_example_scanf_percent_skips_only_leading_whitespace() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/scanf_percent_skips_only_leading_whitespace.c"),
        0,
    );
}

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

fn simple_ub_options() -> RunOptions {
    RunOptions {
        ub_detection_mode: UbDetectionMode::Simple,
        ..RunOptions::default()
    }
}

fn normalize_test_file(path: &Path) {
    let mut text = fs::read_to_string(path).unwrap();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
        fs::write(path, text).unwrap();
    }
}

fn run_file(path: PathBuf) -> Result<RunResult, Diagnostic> {
    normalize_test_file(&path);
    crate_run_file(path)
}

fn run_files<I>(paths: I) -> Result<RunResult, Diagnostic>
where
    I: IntoIterator<Item = PathBuf>,
{
    let collected = paths.into_iter().collect::<Vec<_>>();
    for path in &collected {
        normalize_test_file(path);
    }
    crate_run_files(collected)
}

fn run_files_with_options<I>(paths: I, options: &RunOptions) -> Result<RunResult, Diagnostic>
where
    I: IntoIterator<Item = PathBuf>,
{
    let collected = paths.into_iter().collect::<Vec<_>>();
    for path in &collected {
        normalize_test_file(path);
    }
    crate_run_files_with_options(collected, options)
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let root = temp_test_dir(label);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, path: &str, source: &str) -> std::io::Result<()> {
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }

    fn run<const N: usize>(&self, sources: [&str; N]) -> Result<RunResult, Diagnostic> {
        run_files(sources.map(|source| self.root.join(source)))
    }

    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_user_code_library_diag(rendered: &str, expected: &str) {
    assert!(rendered.contains("test.c"), "{rendered}");
    assert!(rendered.contains(expected), "{rendered}");
    assert!(!rendered.contains("__codex"), "{rendered}");
}

fn state_address(result: &RunResult, name: &str) -> u64 {
    result
        .state
        .iter()
        .find(|item| item.name == name)
        .and_then(|item| item.address)
        .unwrap_or_else(|| panic!("missing address for {name}"))
}

#[test]
fn cboxes_state_addresses_follow_x86_64_alignment_and_keep_objects_disjoint() {
    let source = r#"
            int main(void) {
                int *a;
                int *b;
                int x;
                int y;
                char c;
                int z;
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();

    let a = state_address(&result, "a");
    let b = state_address(&result, "b");
    let x = state_address(&result, "x");
    let y = state_address(&result, "y");
    let c = state_address(&result, "c");
    let z = state_address(&result, "z");

    assert_eq!(b - a, 16);
    assert_eq!(y - x, 8);
    assert_eq!(z - c, 4);
    assert_eq!(a % 8, 0);
    assert_eq!(b % 8, 0);
    assert_eq!(x % 4, 0);
    assert_eq!(y % 4, 0);
    assert_eq!(z % 4, 0);
}

#[test]
fn ub_for_uninitialized_int_without_address_taken() {
    let source = r#"
            int main(void) {
                int x;
                int y = x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "undefined behavior");
}

#[test]
fn reading_uninitialized_int_after_address_taken_is_allowed() {
    let source = r#"
            int main(void) {
                int x;
                int *p = &x;
                int y = x;
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

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
fn ub_for_uninitialized_char_without_address_taken() {
    let source = r#"
            int main(void) {
                char x;
                char y = x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "undefined behavior");
}

#[test]
fn reading_uninitialized_char_after_address_taken_is_allowed() {
    let source = r#"
            int main(void) {
                char x;
                char *p = &x;
                char y = x;
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn simple_mode_rejects_uninitialized_char_after_address_taken() {
    let source = r#"
            int main(void) {
                char a;
                char *p = &a;
                char b = a;
                return 0;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("read of uninitialized automatic object char"));
    assert!(rendered.contains("intentional conservative rejection"));
    assert!(rendered.contains("address was taken"));
}

#[test]
fn simple_mode_allows_addressing_uninitialized_object_and_one_past_pointer() {
    let source = r#"
            int main(void) {
                char untouched;
                char *pointer = &untouched;
                int values[3] = {1, 2, 3};
                int *end = &values[3];
                int *same_end = &values[3];
                return pointer != &untouched || end != same_end;
            }
        "#;
    let result = run_source_with_options("test.c", source, &simple_ub_options()).unwrap();
    assert_eq!(result.exit_status, 0);
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
fn pointer_write_side_effect_updates_object() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                volatile int *p = &x;
                *p = 7;
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
}

#[test]
fn multidimensional_array_zero_index_assignment_preserves_rows() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char a[3][4];
                a[0][0] = 0;
                a[0][1] = 1;
                a[1][0] = 2;
                printf("%d %d %d\n", a[0][0], a[0][1], a[1][0]);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(result.stdout, "0 1 2\n");
    let final_state = &result.trace.last().unwrap().state;
    let root = final_state
        .iter()
        .find(|box_state| box_state.name == "a")
        .unwrap();
    assert_eq!(root.ty, "char[3][4]");
    assert_eq!(
        final_state
            .iter()
            .filter(|box_state| box_state.array_root.as_deref() == Some("a"))
            .count(),
        12
    );
    assert_eq!(
        final_state
            .iter()
            .find(|box_state| box_state.name == "a[0][0]")
            .unwrap()
            .value,
        "0"
    );
    assert_eq!(
        final_state
            .iter()
            .find(|box_state| box_state.name == "a[0][1]")
            .unwrap()
            .value,
        "1"
    );
    assert_eq!(
        final_state
            .iter()
            .find(|box_state| box_state.name == "a[1][0]")
            .unwrap()
            .value,
        "2"
    );
}

#[test]
fn null_pointer_constant_compares_equal_to_pointer() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int *p = 0;
                if (p == 0) {
                    printf("null\n");
                }
                return 0;
            }
        "#;
    assert_stdout(source, "null\n");
}

#[test]
fn void_pointer_null_constant_converts_to_function_pointer() {
    let source = r#"
            #include <stddef.h>

            typedef int (*Callback)(void);
            static int answer(void) { return 42; }
            static const Callback callbacks[] = { answer, NULL };

            int main(void) {
                Callback selected = 0 ? NULL : callbacks[0];
                return callbacks[1] != NULL || selected() != 42;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn non_constant_zero_is_not_a_null_pointer_constant_in_initializer() {
    let source = r#"
            int main(void) {
                int a = 0;
                int *p = a;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("null pointer constant"));
    assert!(rendered.contains("type int to pointer type int*"));
}

#[test]
fn non_constant_zero_is_not_a_null_pointer_constant_in_assignment() {
    let source = r#"
            int main(void) {
                int a = 0;
                int *p = 0;
                p = a;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "null pointer constant");
}

#[test]
fn file_scope_initializer_cannot_call_function() {
    let source = r#"
            int f(void) {
                return 0;
            }
            int a = f();
            int main(void) {
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("static storage duration"));
    assert!(rendered.contains("compile-time constant"));
}

#[test]
fn block_scope_static_initializer_cannot_call_function() {
    let source = r#"
            int f(void) {
                return 0;
            }
            int main(void) {
                static int a = f();
                return a;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("static storage duration"));
    assert!(rendered.contains("compile-time constant"));
}

#[test]
fn static_pointer_initializer_accepts_address_of_array_element() {
    let source = r#"
            static int values[] = { 19, 23, 42 };
            static int *answer = &values[2];

            int main(void) {
                return sizeof(values) / sizeof(values[0]) != 3 || *answer != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn non_constant_zero_is_not_a_null_pointer_constant_for_pointer_parameter() {
    let source = r#"
            void takes_ptr(int *p) {
            }

            int main(void) {
                int a = 0;
                takes_ptr(a);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "null pointer constant");
}

#[test]
fn non_constant_zero_is_not_a_null_pointer_constant_in_pointer_comparison() {
    let source = r#"
            int main(void) {
                int *p = 0;
                int a = 0;
                if (p == a) {
                    return 0;
                }
                return 1;
            }
        "#;
    assert_diagnostic_contains(source, "invalid operands");
}

#[test]
fn conditional_operator_accepts_literal_null_pointer_constant() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int value = 7;
                int *p = 1 ? &value : 0;
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
}

#[test]
fn discarding_const_qualifier_in_pointer_initializer_is_rejected() {
    let source = r#"
            int main(void) {
                const int a = 0;
                int *p = &a;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn nested_pointer_conversion_cannot_create_a_const_qualification_hole() {
    let source = r#"
            int main(void) {
                int **source = 0;
                const int **destination = source;
                return destination != 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn writing_through_cast_pointer_to_const_object_is_ub() {
    let source = r#"
            int main(void) {
                const int a = 0;
                const int *p = &a;
                *(int*)p = 1;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "const-qualified");
}

#[test]
fn writing_through_cast_pointer_to_uninitialized_const_object_is_ub() {
    let source = r#"
            int main(void) {
                const int a;
                *(int*)&a = 1;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "const-qualified");
}

#[test]
fn casting_away_const_from_pointer_to_nonconst_object_can_modify_object() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                const int *p = &x;
                *(int*)p = 1;
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn const_qualified_pointer_object_is_not_assignable() {
    let source = r#"
            int main(void) {
                int x = 0;
                int y = 0;
                int *const p = &x;
                p = &y;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "is const and cannot be changed");
}

#[test]
fn const_qualified_pointer_object_can_be_dereferenced() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int *const p = &x;
                *p = 4;
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn pointer_qualification_conversion_preserves_array_element_provenance() {
    let source = r#"
            #include <stdio.h>
            struct S { int x; };

            int main(void) {
                struct S arr[3] = { {1}, {2}, {3} };
                const struct S *p = &arr[2];
                printf("%d\n", p->x);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn function_return_pointer_conversion_preserves_array_element_provenance() {
    let source = r#"
            #include <stdio.h>
            struct S { int x; };

            const struct S *pick(struct S *p) {
                return p;
            }

            int main(void) {
                struct S arr[3] = { {1}, {2}, {3} };
                const struct S *p = pick(&arr[2]);
                printf("%d\n", p->x);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

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
fn function_prototype_is_accepted() {
    let source = r#"
            #include <stdio.h>
            int add_one(int x);

            int add_one(int x) {
                return x + 1;
            }

            int main(void) {
                printf("%d\n", add_one(6));
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
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
fn call_before_function_declaration_is_rejected() {
    let source = r#"
            int main(void) {
                f();
                return 0;
            }

            int f(void) {
                return 1;
            }
        "#;
    assert_diagnostic_contains(source, "undeclared identifier f");
}

#[test]
fn function_pointer_declaration_and_calls_work() {
    let source = r#"
            #include <stdio.h>
            int add_one(int x) {
                return x + 1;
            }

            int main(void) {
                int (*fp)(int) = &add_one;
                printf("%d %d\n", fp(6), (*fp)(7));
                return 0;
            }
        "#;
    assert_stdout(source, "7 8\n");
}

#[test]
fn explicit_function_pointer_casts_work_but_incompatible_calls_are_ub() {
    let valid = r#"
            int identity(int value) { return value; }
            int main(void) {
                int (*pointer)(int) = (int (*)(int))identity;
                return pointer(7) != 7;
            }
        "#;
    assert_eq!(run_source("test.c", valid).unwrap().exit_status, 0);

    let invalid = r#"
            int identity(int value) { return value; }
            int main(void) {
                double (*pointer)(double) = (double (*)(double))identity;
                return pointer(1.0) != 1.0;
            }
        "#;
    let err = run_source("test.c", invalid).unwrap_err();
    assert!(
        err.render()
            .contains("is incompatible with definition of identity")
    );
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
fn for_loop_with_declaration_break_and_continue_works() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int sum = 0;
                for (int i = 0; i < 6; ++i) {
                    if (i == 2) {
                        continue;
                    }
                    if (i == 5) {
                        break;
                    }
                    sum = sum + i;
                }
                printf("%d\n", sum);
                return 0;
            }
        "#;
    assert_stdout(source, "8\n");
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
fn assignment_target_survives_function_call_sequence_points() {
    let source = r#"
            static unsigned int rotate(unsigned int value, int amount) {
                return (value >> amount) | (value << (32 - amount));
            }

            int main(void) {
                unsigned int value = 0x12345678U;
                value = rotate(value, 7) ^ rotate(value, 18) ^ (value >> 3);
                return value != 0xe7fce6eeU;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn unsequenced_modification_is_reported() {
    let source = r#"
            int main(void) {
                int i = 0;
                i = i++ + 1;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unsequenced");
}

#[test]
fn unsequenced_function_arguments_are_reported() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                printf("%d %d\n", i++, i++);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unsequenced");
}

#[test]
fn logical_and_sequence_point_allows_following_read() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                if ((i = 1) && i) {
                    printf("%d\n", i);
                }
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn nested_sequence_points_do_not_hide_unsequenced_sibling_modifications() {
    for expression in [
        "i++ + (1 ? i++ : 0)",
        "i++ + (1 && i++)",
        "i++ + (0, i++)",
        "(1 && i++) + i++",
    ] {
        let source =
            format!("int main(void) {{ int i = 0; int value = {expression}; return value; }}");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(err.render().contains("unsequenced"), "{expression}");
    }
}

#[test]
fn comma_sequence_makes_simple_assignment_well_defined_but_not_compound_assignment() {
    let valid = "int main(void) { int i = 0; i = (i++, i); return i != 1; }";
    assert_eq!(run_source("test.c", valid).unwrap().exit_status, 0);

    let invalid = "int main(void) { int i = 0; i += (i++, 0); return i; }";
    assert_diagnostic_contains(invalid, "unsequenced");
}

#[test]
fn initializer_list_expressions_are_indeterminately_sequenced() {
    let source = r#"
            struct Pair { int first; int second; };
            int main(void) {
                int i = 0;
                int values[2] = {i++, i++};
                struct Pair pair = {i++, i++};
                return i != 4 || values[0] != 0 || values[1] != 1
                    || pair.first != 2 || pair.second != 3;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn pointer_argument_can_modify_caller_object() {
    let source = r#"
            #include <stdio.h>
            void set_to_seven(int *p) {
                *p = 7;
            }

            int main(void) {
                int x = 0;
                set_to_seven(&x);
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
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
fn copying_pointer_value_after_referent_lifetime_ends_is_ub() {
    let source = r#"
            int main(void) {
                int *p;
                {
                    int a;
                    p = &a;
                }
                int *q = p;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn copying_pointer_returned_from_dead_stack_object_is_ub() {
    let source = r#"
            int *leak(void) {
                int x;
                return &x;
            }

            int main(void) {
                int *p = leak();
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "indeterminate pointer");
}

#[test]
fn pointers_nested_in_returned_aggregates_become_indeterminate() {
    for source in [
        r#"
                struct Carrier { int *pointer; };
                struct Carrier leak(void) {
                    int local = 1;
                    return (struct Carrier){&local};
                }
                int main(void) {
                    int *copy = leak().pointer;
                    return copy != 0;
                }
            "#,
        r#"
                union Carrier { int *pointer; long bits; };
                union Carrier leak(void) {
                    int local = 1;
                    union Carrier result = {.pointer = &local};
                    return result;
                }
                int main(void) {
                    int *copy = leak().pointer;
                    return copy != 0;
                }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("indeterminate") || rendered.contains("lifetime has ended"),
            "{rendered}"
        );
    }
}

#[test]
fn copying_an_aggregate_with_an_ended_pointer_subobject_is_ub() {
    let source = r#"
            struct Carrier { int *pointer; };
            int main(void) {
                struct Carrier source;
                {
                    int local = 1;
                    source.pointer = &local;
                }
                struct Carrier copy = source;
                return copy.pointer != 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("aggregate value containing a pointer"));
    assert!(rendered.contains("lifetime has ended"));
}

#[test]
fn globals_are_zero_initialized_and_mutable() {
    let source = r#"
            #include <stdio.h>
            int g;

            int main(void) {
                printf("%d ", g);
                g = 3;
                printf("%d\n", g);
                return 0;
            }
        "#;
    assert_stdout(source, "0 3\n");
}

#[test]
fn array_subscript_and_pointer_arithmetic_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a[3];
                a[0] = 4;
                a[1] = 5;
                int *p = a;
                printf("%d %d\n", a[0], *(p + 1));
                return 0;
            }
        "#;
    assert_stdout(source, "4 5\n");
}

#[test]
fn arrays_cannot_be_initialized_or_assigned_from_array_values() {
    let source = r#"
            int main(void) {
                int original[3] = {1, 2, 3};
                int copy[3] = original;
                int inferred[] = original;
                copy[0] = 9;
                original = copy;
                copy[1] = 8;
                return sizeof inferred != sizeof original
                    || inferred[2] != 3
                    || original[0] != 9
                    || original[1] != 2
                    || copy[1] != 8;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("an array cannot be initialized by copying another array")
    );
}

#[test]
fn array_compound_literals_decay_to_element_pointers() {
    let source = r#"
            int main(void) {
                int *values = (int[3]){4, 5, 6};
                return values[0] != 4 || values[1] != 5 || values[2] != 6;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn file_scope_array_compound_literals_have_static_storage_duration() {
    assert_exit_status(
        "int *p = (int []){2, 4}; int main(void) { return p[0] != 2 || p[1] != 4; }",
        0,
    );
}

#[test]
fn braced_string_literals_initialize_character_array_compound_literals() {
    assert_exit_status(
        r#"
            int main(void) {
                char *p = (char []){"abc"};
                p[0] = 'x';
                return p[0] != 'x' || p[1] != 'b' || p[3] != 0;
            }
        "#,
        0,
    );
}

#[test]
fn multidimensional_compound_literals_complete_the_outer_array_bound() {
    assert_exit_status(
        r#"
            int main(void) {
                int (*p)[2] = (int [][2]){{1, 2}, {3, 4}};
                return p[1][0] != 3
                    || sizeof((int [][2]){{1, 2}, {3, 4}}) != 16;
            }
        "#,
        0,
    );
}

#[test]
fn functions_cannot_return_arrays() {
    let source = r#"
            int make_values(void)[3] {
                int local[3] = {10, 20, 30};
                return local;
            }

            int main(void) {
                int values[3] = make_values();
                return values[0] != 10 || make_values()[1] != 20;
            }
        "#;
    assert_diagnostic_contains(source, "cannot return an array type");
}

#[test]
fn functions_cannot_return_function_types() {
    let source = "int function(void)(void); int main(void) { return 0; }";
    assert_diagnostic_contains(source, "cannot return a function type");
}

#[test]
fn function_pointers_cannot_have_array_return_types() {
    let source = r#"
            int make_values(void)[2] {
                return (int[2]){4, 7};
            }
            int main(void) {
                int (*factory)(void)[2] = make_values;
                int values[2] = factory();
                return values[0] != 4 || values[1] != 7;
            }
        "#;
    assert_diagnostic_contains(source, "cannot return an array type");
}

#[test]
fn function_definition_cannot_return_an_incomplete_array() {
    let source = r#"
            int make_values(void)[] {
                return (int[2]){1, 2};
            }
            int main(void) { return 0; }
        "#;
    assert_diagnostic_contains(source, "cannot return an array type");
}

#[test]
fn nested_arrays_are_not_assignable_by_value() {
    let source = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                matrix[0] = matrix[1];
                int copy[2][3] = matrix;
                matrix[1][0] = 9;
                return copy[0][2] != 6
                    || copy[1][0] != 4
                    || matrix[0][0] != 4
                    || matrix[1][0] != 9;
            }
        "#;
    assert_diagnostic_contains(source, "arrays cannot be assigned");
}

#[test]
fn nested_incomplete_array_bounds_are_rejected() {
    let source = r#"
            int main(void) {
                int row[] = {1, 2, 3};
                int matrix[][] = {row, row, row};
                int from_lists[][] = {{4, 5}, {6, 7}};
                int original[2][3] = {{8, 9, 10}, {11, 12, 13}};
                int copied[][] = original;
                int compound[][] = (int[][]){{14, 15}, {16, 17}};
                return sizeof matrix != 3 * sizeof row
                    || matrix[2][1] != 2
                    || sizeof from_lists != 4 * sizeof(int)
                    || from_lists[1][0] != 6
                    || sizeof copied != sizeof original
                    || copied[1][2] != 13
                    || compound[1][1] != 17;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("array element has incomplete type int[]"),
        "{}",
        err.render()
    );
}

#[test]
fn nested_incomplete_array_bounds_must_agree_across_elements() {
    let source = r#"
            int main(void) {
                int short_row[2] = {1, 2};
                int long_row[3] = {3, 4, 5};
                int matrix[][] = {short_row, long_row};
                return matrix[0][0];
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("array element has incomplete type int[]"),
        "{}",
        err.render()
    );
}

#[test]
fn array_parameters_adjust_to_pointers() {
    let source = r#"
            int mutate_copy(int values[3]) {
                values[0] = 99;
                return values[0];
            }

            int main(void) {
                int values[3] = {1, 2, 3};
                return mutate_copy(values) != 99 || values[0] != 99;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn incomplete_array_parameters_adjust_to_pointers() {
    let source = r#"
            unsigned long copied_size(int values[]) {
                values[0] = 99;
                return sizeof values;
            }
            int main(void) {
                int values[4] = {1, 2, 3, 4};
                return copied_size(values) != sizeof(int *) || values[0] != 99;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn array_parameter_qualifiers_and_prototype_vla_star_are_accepted() {
    assert_exit_status(
        r#"
            int f(int a[const 3]) { return a[0]; }
            int declared(int a[*]);
            int main(void) { int a[3] = {7, 0, 0}; return f(a) != 7; }
        "#,
        0,
    );
}

#[test]
fn prototype_vla_star_is_rejected_in_a_function_definition() {
    assert_diagnostic_contains(
        "int f(int a[*]) { return 0; } int main(void) { return 0; }",
        "only valid in a function declaration with prototype scope",
    );
}

#[test]
fn pointer_to_incomplete_array_can_point_to_a_completed_array() {
    let source = r#"
            int main(void) {
                int row[3] = {4, 5, 6};
                int (*pointer)[] = &row;
                return (*pointer)[1] - 5;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn size_dependent_operations_reject_pointers_to_incomplete_arrays() {
    for (source, expected) in [
        (
            "int main(void) { int row[3]; int (*p)[] = &row; return sizeof *p; }\n",
            "sizeof cannot determine the size of type int[]",
        ),
        (
            "int main(void) { int row[3]; int (*p)[] = &row; ++p; return 0; }\n",
            "pointer arithmetic cannot be performed because pointed-to type int[] is incomplete",
        ),
        (
            "int main(void) { void *p = 0; ++p; return 0; }\n",
            "pointer arithmetic cannot be performed on void*",
        ),
        (
            "void f(void) {} int main(void) { void (*p)(void) = f; ++p; return 0; }\n",
            "pointer arithmetic cannot be performed on a function pointer",
        ),
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains(expected), "{}", err.render());
    }
}

#[test]
fn array_expressions_decay_to_element_pointers() {
    let source = r#"
            int read_first(int *values) { return values[0]; }

            int main(void) {
                int values[2] = {1, 2};
                return read_first(values);
            }
        "#;
    assert_exit_status(source, 1);
}

#[test]
fn pointer_to_array_does_not_implicitly_convert_to_element_pointer() {
    let source = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int (*row)[3] = &matrix[1];
                int *first = row;
                first[0] = 40;
                return matrix[1][0] != 40 || matrix[0][1] != 2;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn pointer_to_multidimensional_array_does_not_implicitly_flatten() {
    let source = r#"
            int main(void) {
                int cube[2][2][3] = {
                    {{1, 2, 3}, {4, 5, 6}},
                    {{7, 8, 9}, {10, 11, 12}}
                };
                int (*planes)[2][3] = &cube;
                int (*rows)[3] = &cube;
                int *elements = &cube;
                if (planes[1][0][2] != 9) return 1;
                if (rows[0][2] != 3) return 2;
                elements[1] = 20;
                if (cube[0][0][1] != 20) return 3;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn pointer_subtraction_does_not_implicitly_flatten_array_pointers() {
    let source = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int (*row)[3] = &matrix[0];
                int *element = &matrix[0][0];
                return row - element;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("pointers to compatible complete object types"),
        "{}",
        err.render()
    );

    let explicitly_cast = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int (*row)[3] = &matrix[0];
                int *element = &matrix[0][0];
                return (int *)row - element;
            }
        "#;
    let result = run_source("test.c", explicitly_cast).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn pointer_subtraction_does_not_treat_void_pointer_conversion_as_compatibility() {
    for expression in ["numbers - erased", "erased - numbers"] {
        let source = format!(
            r#"
                    int main(void) {{
                        int values[2] = {{0, 0}};
                        int *numbers = values;
                        void *erased = numbers;
                        return {expression};
                    }}
                "#
        );
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render()
                .contains("pointers to compatible complete object types"),
            "unexpected diagnostic for {expression}: {}",
            err.render()
        );
    }
}

#[test]
fn pointer_subtraction_requires_complete_pointed_to_types() {
    let source = r#"
            struct Opaque;
            long distance(struct Opaque *left, struct Opaque *right) {
                return left - right;
            }
            int main(void) { return 0; }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("pointers to compatible complete object types"),
        "{}",
        err.render()
    );
}

#[test]
fn pointer_subtraction_respects_nested_array_boundaries() {
    let valid = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int (*first)[3] = &matrix[0];
                int (*second)[3] = &matrix[1];
                int *row = *second;
                return second - first != 1 || &row[3] - row != 3;
            }
        "#;
    let result = run_source("test.c", valid).unwrap();
    assert_eq!(result.exit_status, 0);

    let invalid = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int *first = &matrix[0][0];
                int *second = &matrix[1][0];
                return second - first;
            }
        "#;
    assert_diagnostic_contains(invalid, "same array object");
}

#[test]
fn relational_pointer_comparisons_respect_array_provenance() {
    let valid = r#"
            int main(void) {
                int values[3] = {1, 2, 3};
                return !(&values[0] < &values[2])
                    || !(&values[2] <= &values[3])
                    || !(&values[3] > &values[0]);
            }
        "#;
    let result = run_source("test.c", valid).unwrap();
    assert_eq!(result.exit_status, 0);

    let invalid = r#"
            int main(void) {
                int matrix[2][2] = {{1, 2}, {3, 4}};
                return &matrix[0][0] < &matrix[1][0];
            }
        "#;
    assert_diagnostic_contains(invalid, "same array object");
}

#[test]
fn relational_pointer_constraints_require_compatible_object_types() {
    for expression in [
        "numbers < erased",
        "erased < numbers",
        "erased < other_erased",
    ] {
        let source = format!(
            r#"
                    int main(void) {{
                        int value = 0;
                        int *numbers = &value;
                        void *erased = numbers;
                        void *other_erased = numbers;
                        if (0) return {expression};
                        return 0;
                    }}
                "#
        );
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render().contains("pointers to compatible object types"),
            "unexpected diagnostic for {expression}: {}",
            err.render()
        );
    }
}

#[test]
fn relational_pointer_constraints_allow_compatible_incomplete_object_types() {
    let source = r#"
            struct Opaque;
            int ordered(struct Opaque *left, const struct Opaque *right) {
                return left < right;
            }
            int main(void) { return 0; }
        "#;
    assert_exit_status(source, 0);
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
fn pointers_to_members_of_one_struct_follow_declaration_order() {
    let source = r#"
            struct Pair { int first; int second; };
            int main(void) {
                struct Pair pair = {0, 0};
                return &pair.first < &pair.second ? 0 : 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn array_members_are_not_assignable_from_conditional_expressions() {
    let source = r#"
            struct Box { int values[2]; };

            int main(void) {
                int left[2] = {1, 2};
                int right[2] = {3, 4};
                struct Box box = {{0, 0}};
                box.values = 1 ? left : right;
                return box.values[0] != 1 || box.values[1] != 2;
            }
        "#;
    assert_diagnostic_contains(source, "arrays cannot be assigned");
}

#[test]
fn array_assignment_rejects_const_elements() {
    let source = r#"
            int main(void) {
                const int destination[2] = {1, 2};
                int source[2] = {3, 4};
                destination = source;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "arrays cannot be assigned");
}

#[test]
fn array_values_cannot_be_copied_to_drop_qualifiers() {
    let source = r#"
            int main(void) {
                const int source[2] = {1, 2};
                int mutable_copy[2] = source;
                int other[2] = {3, 4};
                const int const_copy[2] = other;
                mutable_copy[0] = 9;
                return mutable_copy[0] != 9
                    || source[0] != 1
                    || const_copy[1] != 4;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("an array cannot be initialized by copying another array")
    );
}

#[test]
fn aggregate_assignment_rejects_nested_const_subobjects() {
    let source = r#"
            struct Box { const int values[2]; };
            int main(void) {
                struct Box destination = {{1, 2}};
                struct Box source = {{3, 4}};
                destination = source;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "is const and cannot be changed");
}

#[test]
fn memcpy_cannot_bypass_nested_const_subobjects() {
    let source = r#"
            #include <string.h>
            struct Box { const int values[2]; };
            int main(void) {
                struct Box destination = {{1, 2}};
                struct Box source = {{3, 4}};
                memcpy(&destination, &source, sizeof destination);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "const-qualified");
}

#[test]
fn nonconst_sibling_of_const_member_remains_modifiable() {
    let source = r#"
            #include <string.h>
            struct Value { const int fixed; int mutable; };
            int main(void) {
                struct Value value = {1, 2};
                value.mutable = 3;
                int replacement = 4;
                memcpy(&value.mutable, &replacement, sizeof replacement);
                return value.fixed != 1 || value.mutable != 4;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn cast_cannot_hide_a_volatile_member_access() {
    let source = r#"
            struct Value { volatile int member; };
            int main(void) {
                struct Value value = {1};
                int *pointer = (int *)&value.member;
                return *pointer;
            }
        "#;
    assert_diagnostic_contains(source, "volatile-qualified");
}

#[test]
fn memcpy_can_copy_partly_indeterminate_character_arrays() {
    let source = r#"
            #include <string.h>
            int main(void) {
                char source[4];
                char destination[4];
                source[0] = 42;
                memcpy(destination, source, sizeof source);
                return destination[0] != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn memmove_can_move_overlapping_partly_indeterminate_bytes() {
    let source = r#"
            #include <string.h>
            int main(void) {
                char bytes[4];
                bytes[0] = 42;
                memmove(bytes + 1, bytes, 3);
                return bytes[1] != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn memset_can_create_an_invalid_pointer_representation_for_character_inspection() {
    let source = r#"
            #include <string.h>
            int main(void) {
                int *pointer = 0;
                memset(&pointer, 0xff, sizeof pointer);
                unsigned char *bytes = (unsigned char *)&pointer;
                return bytes[0] != 255;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn invalid_pointer_representation_is_ub_only_when_read_as_a_pointer() {
    let source = r#"
            #include <string.h>
            int main(void) {
                int *pointer = 0;
                memset(&pointer, 0xff, sizeof pointer);
                return pointer == 0;
            }
        "#;
    assert_diagnostic_contains(source, "invalid object representation");
}

#[test]
fn memcpy_preserves_invalid_bool_representation_for_character_inspection() {
    let source = r#"
            #include <string.h>
            int main(void) {
                unsigned char byte = 2;
                _Bool value = 0;
                memcpy(&value, &byte, 1);
                return *(unsigned char *)&value != 2;
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
fn byte_copies_preserve_structure_padding() {
    let source = r#"
            #include <string.h>
            #include <stdlib.h>
            struct Value { char character; int integer; };
            union Number { long integer; double number; };
            union Node {
                struct {
                    union Number first;
                    unsigned char first_tag;
                    unsigned char second_tag;
                    int next;
                    union Number second;
                } key;
                struct { union Number value; unsigned char tag; } value;
            };
            int main(void) {
                unsigned char input[sizeof(struct Value)] = {1, 2, 3, 4, 5, 6, 7, 8};
                unsigned char output[sizeof(struct Value)] = {0};
                struct Value value;
                memcpy(&value, input, sizeof value);
                memcpy(output, &value, sizeof value);
                if (memcmp(input, output, sizeof value) != 0) return 1;

                struct Value initialized_by_members;
                initialized_by_members.character = 9;
                initialized_by_members.integer = 42;
                struct Value copy = initialized_by_members;
                if (copy.character != 9 || copy.integer != 42) return 2;

                union Node *nodes = malloc(4 * sizeof *nodes);
                for (int i = 0; i < 4; ++i) {
                    nodes[i].key.first.integer = i + 10;
                    nodes[i].key.first_tag = 1;
                    nodes[i].key.second_tag = 2;
                    nodes[i].key.next = 0;
                    nodes[i].key.second.integer = i + 20;
                }
                nodes[1] = nodes[3];
                int bad = nodes[1].key.first.integer != 13
                    || nodes[1].key.second.integer != 23;
                free(nodes);
                return bad;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn byte_or_string_library_access_cannot_escape_a_member_subobject() {
    for source in [
        r#"
                #include <string.h>
                struct Pair { char first; char second; };
                int main(void) {
                    struct Pair pair = {0, 0};
                    char source[2] = {1, 2};
                    memcpy(&pair.first, source, 2);
                    return 0;
                }
            "#,
        r#"
                #include <stdio.h>
                struct Buffers { char small[1]; char sibling[8]; };
                int main(void) {
                    struct Buffers buffers = {{0}, {0}};
                    sprintf(buffers.small, "abc");
                    return 0;
                }
            "#,
        r#"
                #include <string.h>
                struct Text { char first[1]; char following[2]; };
                int main(void) {
                    struct Text text = {{'x'}, {0, 0}};
                    return strlen(text.first);
                }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("subobject") || rendered.contains("not terminated"),
            "{rendered}"
        );
    }
}

#[test]
fn library_writes_respect_nested_const_and_volatile_qualifiers() {
    for qualifier in ["const", "volatile"] {
        let source = format!(
            r#"
                    #include <stdio.h>
                    struct Buffers {{ {qualifier} char protected_bytes[4]; char writable[4]; }};
                    int main(void) {{
                        struct Buffers buffers = {{{{0}}, {{0}}}};
                        snprintf((char *)buffers.protected_bytes, 4, "x");
                        return 0;
                    }}
                "#
        );
        let err = run_source("test.c", &source).unwrap_err();
        assert!(err.render().contains(qualifier), "{}", err.render());
    }

    let allowed = r#"
            #include <stdio.h>
            struct Buffers { const char protected_bytes[4]; char writable[4]; };
            int main(void) {
                struct Buffers buffers = {{0}, {0}};
                snprintf(buffers.writable, 4, "ok");
                return buffers.writable[1] != 'k';
            }
        "#;
    let result = run_source("test.c", allowed).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn address_of_dereference_does_not_access_the_pointed_to_object() {
    let source = r#"
            int main(void) {
                int *null_pointer = 0;
                int *same_null_pointer = &*null_pointer;
                int values[3] = { 1, 2, 3 };
                int *one_past = &values[3];
                return same_null_pointer != null_pointer || one_past - &values[0] != 3;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn pointer_subtraction_within_the_same_character_array_works() {
    let source = r#"
            int main(void) {
                char text[] = "abc.txt";
                char *dot = &text[3];
                return (int)(dot - &text[0]) - 3;
            }
        "#;
    assert_stdout(source, "");
}

#[test]
fn pointer_to_array_declaration_and_dereference_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a[3];
                int (*b)[3] = &a;
                (*b)[1] = 5;
                printf("%d\n", a[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
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
fn char_array_from_string_literal_initializes_and_prints() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char s[4] = "hey";
                printf("%s %d\n", &s[0], s[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "hey 101\n");
}

#[test]
fn pointer_arithmetic_out_of_bounds_is_ub() {
    let source = r#"
            int main(void) {
                int a[2];
                int *p = &a[3];
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "outside the bounds");
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
fn conditional_struct_and_union_operands_undergo_lvalue_conversion() {
    let source = r#"
            struct Record { int value; };
            union Choice { int value; };

            int main(void) {
                const struct Record const_record = {11};
                struct Record record = {12};
                const union Choice const_choice = {21};
                union Choice choice = {22};
                return (1 ? const_record : record).value != 11
                    || (0 ? const_choice : choice).value != 22;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn generic_selection_distinguishes_qualified_types() {
    let source = r#"
            int main(void) {
                int value = 3;
                const int *pointer = &value;
                return _Generic(1, int: 0, const int: 1)
                    || _Generic(pointer, int *: 2, const int *: 0);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn generic_association_types_must_be_valid_and_unique() {
    for source in [
        "int main(void) { return _Generic(1, int: 0, float: 1, float: 2); }\n",
        "struct S;\nint main(void) { return _Generic(1, int: 0, struct S: 1); }\n",
        "int main(void) { return _Generic(1, int: 0, void: 1); }\n",
        "int main(void) { return _Generic(1, int: 0, int (*)(int): 1, int (*)(const int): 2); }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("complete object type")
                || rendered.contains("compatible types more than once"),
            "{rendered}"
        );
    }
}

#[test]
fn equality_rejects_incompatible_object_pointer_types() {
    let source = r#"
            int main(void) {
                int *p = 0;
                char *q = 0;
                return p == q;
            }
        "#;
    assert_diagnostic_contains(source, "compatible pointer operand types");
}

#[test]
fn conditional_rejects_incompatible_object_pointer_types() {
    let source = r#"
            int main(void) {
                int *p = 0;
                char *q = 0;
                void *r = 1 ? p : q;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "compatible pointer operand types");
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
fn void_pointer_accepts_object_pointer_in_conditional() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                void *vp = 0;
                int *ip = &x;
                void *r = 1 ? vp : ip;
                printf("%p\n", r);
                return 0;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert!(result.stdout == "(nil)\n" || result.stdout.starts_with("0x"));
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
fn incomplete_char_array_from_string_literal_gets_deduced_size() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                char s[] = "ok";
                printf("%s %d\n", s, s[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "ok 0\n");
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
fn comma_separated_declarations_work() {
    let source = r#"
            #include <stdio.h>
            union Value { int integer; };
            typedef union Value *ValuePointer;
            int g = 1, h = 2;

            int main(void) {
                union Value values[2] = {{19}, {23}};
                ValuePointer first, second;
                first = values;
                second = first + 1;
                int a = 3, b = 4;
                for (int i = 0, j = 1; i < 1; ++i) {
                    printf("%d %d %d %d %d %d\n",
                           g, h, a, b, j, first->integer + second->integer);
                }
                return 0;
            }
        "#;
    assert_stdout(source, "1 2 3 4 1 42\n");
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
fn pointer_compound_assignment_and_increment_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a[3];
                int *p = a;
                p += 2;
                *p = 9;
                --p;
                *p = 4;
                printf("%d %d\n", a[1], a[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "4 9\n");
}

#[test]
fn compound_assignment_preserves_unsequenced_ub() {
    let source = r#"
            int main(void) {
                int i = 0;
                i += i++;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unsequenced");
}

#[test]
fn unsequenced_tracking_distinguishes_disjoint_scalar_subobjects() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int first; int second; };

            int main(void) {
                int values[2] = { 1, 2 };
                values[0] = values[1]++;

                struct Pair pair = { 3, 4 };
                pair.first = pair.second++;

                printf("%d %d %d %d\n",
                    values[0], values[1], pair.first, pair.second);
                return 0;
            }
        "#;
    assert_stdout(source, "2 3 4 5\n");
}

#[test]
fn unsequenced_tracking_still_detects_aliases_of_the_same_subobject() {
    let source = r#"
            int main(void) {
                int values[2] = { 1, 2 };
                int *alias = &values[0];
                values[0] = (*alias)++;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "unsequenced");
}

#[test]
fn unsequenced_tracking_detects_overlapping_aggregate_and_member_accesses() {
    for expression in [
        "(s = (struct Pair){1, 2}).first + s.first",
        "(t = s).first + (s.first = 1)",
    ] {
        let source = format!(
            r#"
                    struct Pair {{ int first; int second; }};
                    int main(void) {{
                        struct Pair s = {{0, 0}};
                        struct Pair t = {{0, 0}};
                        return {expression};
                    }}
                "#
        );
        let err = run_source("test.c", &source).unwrap_err();
        assert!(err.render().contains("unsequenced"), "{expression}");
    }
}

#[test]
fn adjacent_bit_fields_are_distinct_scalar_objects_for_sequencing() {
    let source = r#"
            struct Bits { unsigned int low : 4; unsigned int high : 4; };
            int main(void) {
                struct Bits bits = {1, 2};
                int sum = bits.low++ + bits.high++;
                return sum != 3 || bits.low != 2 || bits.high != 3;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn sizeof_handles_types_arrays_and_strings() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int a[3];
                printf("%lu %lu %lu %lu\n", sizeof(int), sizeof a, sizeof(&a), sizeof "hey");
                return 0;
            }
        "#;
    assert_stdout(source, "4 12 8 4\n");
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
fn optimizing_precomputations_preserve_switch_and_union_behavior() {
    let source = r#"
            union value { int integer; unsigned char bytes[4]; };
            struct state { union value value; int total; };

            int main(void) {
                struct state state = { { 0 }, 0 };
                for (int i = 0; i < 8; ++i) {
                    state.value.integer = i & 3;
                    switch (state.value.integer) {
                        case 0: state.total += 1; break;
                        case 1: state.total += 2; break;
                        case 2: state.total += 4; break;
                        default: state.total += 8; break;
                    }
                }
                return state.total != 30;
            }
        "#;
    let options = RunOptions {
        optimizing_precomputations: true,
        ..RunOptions::default()
    };
    let result = run_source_with_options("test.c", source, &options).unwrap();
    assert_eq!(result.exit_status, 0);
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
fn switch_past_fixed_declaration_uses_the_inner_object() {
    let source = r#"
            int main(void) {
                int x = 0;
                switch (1) {
                    int x = 99;
                    case 1: x = 5; break;
                }
                return x;
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
fn automatic_object_without_initializer_becomes_indeterminate_each_reach() {
    let source = r#"
            int main(void) {
                int pass = 0;
            again:
                ;
                int value;
                if (pass++ == 0) {
                    value = 7;
                    goto again;
                }
                return value;
            }
        "#;
    assert_diagnostic_contains(source, "uninitialized automatic object");
}

#[test]
fn nested_designated_initializers_continue_from_the_selected_subobject() {
    assert_exit_status(
        r#"
        struct S { int a[2]; int b; };
        union U { struct S s; int other; };
        int main(void) {
            int a[3][2] = { [0][0] = 1, 2, 3 };
            struct S s[2] = { [0].a[0] = 1, 2, 3, 4, 5, 6 };
            union U u = { .s.a[0] = 7, 8, 9 };
            int b[3][2] = { [0][0] = 1, 2, [2][0] = 3, 4 };
            return a[0][0] != 1 || a[0][1] != 2 || a[1][0] != 3 || a[1][1] != 0
                || s[0].a[1] != 2 || s[0].b != 3 || s[1].a[0] != 4 || s[1].b != 6
                || u.s.a[1] != 8 || u.s.b != 9 || b[1][0] != 0 || b[2][1] != 4;
        }
    "#,
        0,
    );
}

#[test]
fn pointers_to_members_promoted_from_anonymous_aggregates_have_defined_order() {
    let source = r#"
            struct S {
                union {
                    struct { int x; };
                };
                int y;
            };
            int main(void) {
                struct S s;
                return !(&s.x < &s.y);
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn pointer_arithmetic_accepts_qualification_within_nested_array_types() {
    let source = r#"
            int main(void) {
                int values[2][3] = {{1, 2, 3}, {4, 5, 6}};
                const int (*p)[3] = values;
                int (*q)[3] = values + 1;
                return (p + 1)[0][0] != 4 || q - p != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn declaratorless_tagged_record_member_is_rejected() {
    let source = "struct S { struct T { int x; }; int y; };\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "declarator-less record member");
}

#[test]
fn adjacent_bit_fields_share_a_storage_unit_when_space_remains() {
    let source = r#"
            #include <string.h>
            struct S { _Bool a : 1; unsigned int b : 1; char x; };
            int main(void) {
                struct S value = {0};
                unsigned char bytes[sizeof value];
                value.a = 1;
                value.b = 1;
                memcpy(bytes, &value, sizeof value);
                return bytes[0] != 3;
            }
        "#;
    assert_exit_status(source, 0);
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
fn sizeof_compound_literal_with_incomplete_array_type_uses_completed_bound() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                printf("%lu %lu\n",
                    sizeof((int[]){ 1, 2, 3 }),
                    sizeof((char[]){ 'a', 'b' }));
                return 0;
            }
        "#;
    assert_stdout(source, "12 2\n");
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
fn narrow_unsigned_bit_field_promotions_determine_binary_and_conditional_types() {
    let source = r#"
            #define IS_INT(value) _Generic((value), int: 1, default: 0)
            struct Bits { unsigned int narrow : 3; };

            int main(void) {
                struct Bits bits = { 7 };
                return !IS_INT(bits.narrow + 1)
                    || !IS_INT(bits.narrow & 1)
                    || !IS_INT(0 ? bits.narrow : -1)
                    || !((0 ? bits.narrow : -1) < 0);
            }
        "#;
    assert_exit_status(source, 0);
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
fn object_pointer_contexts_reject_function_pointers() {
    let source = r#"
            int f(void) { return 0; }

            int main(void) {
                void *p = f;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn equality_rejects_void_pointer_and_function_pointer() {
    let source = r#"
            int f(void) { return 0; }

            int main(void) {
                void *p = 0;
                return p == f;
            }
        "#;
    assert_diagnostic_contains(source, "compatible pointer operand types");
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
fn incompatible_overlapping_assignment_is_undefined() {
    let source = r#"
            union Value { int signed_value; unsigned int unsigned_value; };
            int main(void) {
                union Value value = { .signed_value = 7 };
                value.signed_value = value.unsigned_value;
                return 0;
            }
        "#;
    let diagnostic = rendered_diagnostic(source);
    assert!(diagnostic.contains("overlapping object"), "{diagnostic}");
    assert!(diagnostic.contains("standard: 6.5.16.1"), "{diagnostic}");

    assert_exit_status(
        "union V { int a; int b; }; int main(void) { union V v = { .a = 3 }; v.a = v.b; return v.a != 3; }\n",
        0,
    );
}

#[test]
fn restricted_pointer_assignment_and_const_based_write_are_undefined() {
    for source in [
        "int main(void) { int x, y; int *restrict p = &x; int *restrict q = &y; *p = 1; p = q; return 0; }\n",
        "int main(void) { int x = 0; const int *restrict p = &x; *((int *)p) = 1; return 0; }\n",
    ] {
        let diagnostic = rendered_diagnostic(source);
        assert!(diagnostic.contains("standard: 6.7.3.1"), "{diagnostic}");
    }

    assert_exit_status(
        "int main(void) { int x = 0; int *restrict outer = &x; { int *restrict inner = outer; *inner = 1; } return x != 1; }\n",
        0,
    );
}

#[test]
fn standalone_struct_definition_member_access_and_sizeof_work() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair p;
                p.x = 3;
                p.y = 4;
                printf("%d %d %lu\n", p.x, p.y, sizeof(struct Pair));
                return 0;
            }
        "#;
    assert_stdout(source, "3 4 8\n");
}

#[test]
fn taking_address_of_struct_member_and_writing_through_it_works() {
    let source = r#"
            #include <stdio.h>
            #include <stdint.h>
            #include <stdlib.h>
            #include <string.h>
            struct Pair { int x; int y; };
            struct Header { struct Header *next; int tag; };
            struct Table { struct Header *next; int tag; int value; };
            union Objects {
                struct Header header;
                struct Table table;
                unsigned char largest_member[128];
            };
            struct WrappedObjects { int prefix; union Objects objects; };

            int main(void) {
                struct Pair p;
                int *px = &p.x;
                *px = 9;
                struct Header *object = malloc(sizeof(struct Table));
                object->next = 0;
                object->tag = 7;
                struct Table *table = &((union Objects *)object)->table;
                table->value = 42;
                struct Header *header = &((union Objects *)table)->header;
                struct Header *saved;
                memcpy(&saved, &object, sizeof saved);
                uintptr_t object_address = (uintptr_t)object;
                uintptr_t first_member_address = (uintptr_t)&object->next;
                struct Header *loaded;
                memcpy(&loaded, &saved, sizeof loaded);
                struct Table *round_trip = &((union Objects *)loaded)->table;
                struct WrappedObjects wrapped = {0};
                struct Table *nested_table = &wrapped.objects.table;
                union Objects *nested_owner = (union Objects *)nested_table;
                nested_owner->header.tag = 11;
                union Objects *allocation = malloc(sizeof *allocation);
                struct Header *allocation_header = &allocation->header;
                printf("%d %d %d %d %d\n", p.x, table->value, header->tag,
                       object_address == first_member_address && round_trip->value == 42,
                       wrapped.objects.header.tag);
                free(object);
                free(allocation_header);
                return 0;
            }
        "#;
    assert_stdout(source, "9 42 7 1 11\n");
}

#[test]
fn array_of_structs_and_arrow_member_access_work() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair items[2];
                items[1].y = 7;
                struct Pair *p = items;
                printf("%d\n", (p + 1)->y);
                return 0;
            }
        "#;
    assert_stdout(source, "7\n");
}

#[test]
fn struct_return_and_assignment_by_value_work() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int x; int y; };

            struct Pair make(int x) {
                struct Pair p;
                p.x = x;
                p.y = x + 1;
                return p;
            }

            int main(void) {
                struct Pair a = make(4);
                struct Pair b = a;
                printf("%d %d %d %d\n", a.x, a.y, b.x, b.y);
                return 0;
            }
        "#;
    assert_stdout(source, "4 5 4 5\n");
}

#[test]
fn enum_constants_and_sizeof_work() {
    let source = r#"
            #include <stdio.h>
            enum Color { RED = 1, GREEN, BLUE = 7 };

            int main(void) {
                enum Color c = GREEN;
                printf("%d %lu\n", BLUE + c, sizeof(enum Color));
                return 0;
            }
        "#;
    assert_stdout(source, "9 4\n");
}

#[test]
fn negative_enum_constants_keep_int_type_when_used() {
    let source = r#"
            enum Direction {
                MINIMUM = -2147483647 - 1,
                LEFT = -1,
                ALSO_LEFT = LEFT,
                RIGHT = 1
            };

            int main(void) {
                enum Direction direction = LEFT;
                return direction != -1
                    || ALSO_LEFT != -1
                    || MINIMUM != (-2147483647 - 1)
                    || RIGHT != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn enumerator_values_must_fit_in_int() {
    for source in [
        "enum Number { TOO_BIG = 2147483648 };\nint main(void) { return 0; }\n",
        "enum Number { LAST = 2147483647, TOO_BIG };\nint main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(
            err.render()
                .contains("enumerator value must be representable as int")
        );
    }
}

#[test]
fn incomplete_struct_object_declaration_is_rejected() {
    let source = r#"
            struct S;

            int main(void) {
                struct S value;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "object definition has incomplete type struct S");
}

#[test]
fn self_referential_struct_with_pointer_member_is_allowed() {
    let source = r#"
            #include <stdio.h>
            struct Node;
            struct Node { int value; struct Node *next; };

            int main(void) {
                struct Node n;
                n.value = 3;
                n.next = &n;
                printf("%d %d\n", n.value, n.next == &n);
                return 0;
            }
        "#;
    assert_stdout(source, "3 1\n");
}

#[test]
fn member_of_const_struct_is_not_modifiable() {
    let source = r#"
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair tmp;
                tmp.x = 1;
                tmp.y = 2;
                const struct Pair p = tmp;
                p.x = 3;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "is const and cannot be changed");
}

#[test]
fn uninitialized_struct_member_without_address_taken_is_ub() {
    let source = r#"
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair p;
                int y = p.x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "undefined behavior");
}

#[test]
fn reading_uninitialized_struct_member_after_address_taken_is_allowed() {
    let source = r#"
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair p;
                struct Pair *q = &p;
                int y = p.x;
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn simple_mode_rejects_uninitialized_member_of_addressed_struct() {
    let source = r#"
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair pair;
                struct Pair *pointer = &pair;
                pair.x = 1;
                return pair.y;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    assert!(
        err.render()
            .contains("read of uninitialized automatic object int")
    );
}

#[test]
fn duplicate_struct_member_names_are_rejected() {
    let source = r#"
            struct Bad { int x; int x; };

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "duplicate member declaration");
}

#[test]
fn inner_scopes_can_shadow_record_enum_tags_and_enumerators() {
    let source = r#"
            struct Item { int outer; };
            union Value { int outer; };
            enum Shade { color = 1 };

            int main(void) {
                struct Item { double inner; };
                union Value { double inner; };
                enum Shade { color = 2 };
                struct Item item;
                union Value value;
                item.inner = color;
                value.inner = item.inner;
                return value.inner == 2.0 ? 0 : 1;
            }
        "#;
    run_source("test.c", source).unwrap();
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
fn standalone_inner_tag_declarations_hide_outer_tags() {
    let incomplete_shadow = r#"
            struct Item { int outer; };
            int main(void) {
                struct Item;
                struct Item value;
                return 0;
            }
        "#;
    let err = run_source("test.c", incomplete_shadow).unwrap_err();
    assert!(
        err.render()
            .contains("object definition has incomplete type struct Item")
    );

    let different_kind_shadow = r#"
            union Item { int outer; };
            int main(void) {
                struct Item;
                struct Item { double inner; };
                struct Item value;
                value.inner = 1.0;
                return 0;
            }
        "#;
    run_source("test.c", different_kind_shadow).unwrap();
}

#[test]
fn function_definition_parameter_tags_are_visible_in_the_body_only() {
    let definition = r#"
            int read(struct Item { int value; } item) {
                struct Item copy = item;
                return copy.value;
            }
            int main(void) { return 0; }
        "#;
    run_source("test.c", definition).unwrap();

    let prototype = r#"
            int read(struct Hidden *item);
            int main(void) {
                struct Hidden value;
                return 0;
            }
        "#;
    let err = run_source("test.c", prototype).unwrap_err();
    assert!(
        err.render()
            .contains("object definition has incomplete type struct Hidden")
    );
}

#[test]
fn empty_struct_is_rejected() {
    let source = r#"
            struct Empty { };

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "at least one member");
}

#[test]
fn record_type_cannot_silently_consume_a_following_builtin_type_specifier() {
    let err = run_source(
        "test.c",
        "struct Point { int x; }\nint main(void) { return 0; }\n",
    )
    .unwrap_err();
    assert!(
        err.render()
            .contains("struct Point cannot be combined with a built-in type specifier")
    );
}

#[test]
fn record_with_only_unnamed_bit_fields_is_ub() {
    let source = "struct Empty { unsigned int : 3; };\nint main(void) { return 0; }\n";
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("contains no named members"));
    assert!(rendered.contains("6.7.2.1p8"));
}

#[test]
fn incomplete_member_type_is_rejected() {
    let source = r#"
            struct Bad { struct Bad child; };

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "complete type");
}

#[test]
fn array_of_completed_typedef_record_has_complete_type() {
    let source = r#"
            typedef struct Db Db;
            struct Db { int value; };
            struct Connection { Db static_databases[2]; };

            int main(void) {
                struct Connection connection = {{{19}, {23}}};
                return connection.static_databases[0].value
                     + connection.static_databases[1].value != 42;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn union_active_member_access_and_sizeof_work() {
    let source = r#"
            #include <stdio.h>
            union Value { int i; int j; long l; };

            int main(void) {
                union Value v;
                v.l = 9;
                union Value overlap = { .i = 7 };
                overlap.j = overlap.i;
                printf("%ld %lu %d\n", v.l, sizeof(union Value), overlap.j);
                return 0;
            }
        "#;
    assert_stdout(source, "9 8 7\n");
}

#[test]
fn inactive_union_member_access_reinterprets_object_representation() {
    let source = r#"
            #include <stdio.h>
            union Value { int i; long l; };

            int main(void) {
                union Value v = {0};
                v.i = 1;
                printf("%ld\n", v.l);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn simple_mode_rejects_inactive_union_member_read() {
    let source = r#"
            union Value { unsigned int bits; float number; };

            int main(void) {
                union Value value;
                value.bits = 0;
                return value.number == 0.0f ? 0 : 1;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("read of inactive union member number"));
    assert!(rendered.contains("member bits is active"));
    assert!(rendered.contains("representation-level technique"));
}

#[test]
fn simple_mode_preserves_strict_union_representation_ub() {
    let source = r#"
            union Value { unsigned char raw; _Bool flag; };

            int main(void) {
                union Value value;
                value.raw = 2;
                return value.flag;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("potentially invalid object representation"));
    assert!(!rendered.contains("intentional conservative rejection"));
}

#[test]
fn simple_mode_rejects_inactive_member_of_union_rvalue() {
    let source = r#"
            union Value { unsigned int bits; float number; };

            union Value zero_bits(void) {
                union Value value;
                value.bits = 0;
                return value;
            }

            int main(void) {
                return zero_bits().number == 0.0f ? 0 : 1;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(
        rendered.contains("read of inactive union member number"),
        "{rendered}"
    );
}

#[test]
fn simple_mode_rejects_inactive_union_member_in_array() {
    let source = r#"
            union Value { unsigned int bits; float number; };

            int main(void) {
                union Value values[2];
                values[1].bits = 0;
                return values[1].number == 0.0f ? 0 : 1;
            }
        "#;
    let err = run_source_with_options("test.c", source, &simple_ub_options()).unwrap_err();
    let rendered = err.render();
    assert!(
        rendered.contains("read of inactive union member number"),
        "{rendered}"
    );
}

#[test]
fn simple_mode_allows_union_common_initial_sequence_read() {
    let source = r#"
            struct First { int tag; int first; };
            struct Second { int tag; double second; };
            union Tagged { struct First first; struct Second second; };

            int main(void) {
                union Tagged value = { .first = { 7, 9 } };
                return value.second.tag != 7;
            }
        "#;
    let result = run_source_with_options("test.c", source, &simple_ub_options()).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn simple_mode_allows_reading_the_active_union_member() {
    let source = r#"
            union Value { int integer; float number; };

            int main(void) {
                union Value value;
                value.integer = 7;
                return value.integer != 7;
            }
        "#;
    let result = run_source_with_options("test.c", source, &simple_ub_options()).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn distinct_struct_pointer_types_are_incompatible() {
    let source = r#"
            struct A { int x; };
            struct B { int x; };

            int main(void) {
                struct A a;
                struct B *p = &a;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert");
}

#[test]
fn nested_struct_member_paths_work() {
    let source = r#"
            #include <stdio.h>
            struct Inner { int value; };
            struct Outer { struct Inner inner; };

            int main(void) {
                struct Outer o;
                o.inner.value = 12;
                printf("%d\n", o.inner.value);
                return 0;
            }
        "#;
    assert_stdout(source, "12\n");
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
fn qualifiers_propagate_through_member_address() {
    let source = r#"
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair tmp;
                tmp.x = 1;
                tmp.y = 2;
                const struct Pair p = tmp;
                int *q = &p.x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot convert const int* to int*");
}

#[test]
fn array_members_in_records_work() {
    let source = r#"
            #include <stdio.h>
            struct Good { int values[2]; };

            int main(void) {
                struct Good g;
                g.values[0] = 4;
                g.values[1] = 9;
                printf("%d %d %lu\n", g.values[0], g.values[1], sizeof(struct Good));
                return 0;
            }
        "#;
    assert_stdout(source, "4 9 8\n");
}

#[test]
fn anonymous_struct_and_union_members_are_visible() {
    let source = r#"
            #include <stdio.h>
            struct Value {
                int tag;
                struct {
                    int x;
                    int y;
                };
                union {
                    int z;
                    unsigned char bytes[4];
                };
            };

            int main(void) {
                struct Value v;
                v.tag = 1;
                v.x = 7;
                v.y = 8;
                v.z = 9;
                printf("%d %d %d %d\n", v.tag, v.x, v.y, v.bytes[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "1 7 8 9\n");
}

#[test]
fn bit_fields_pack_store_and_read_back() {
    let source = r#"
            #include <stdio.h>
            struct Bits {
                unsigned int a : 3;
                signed int b : 5;
                unsigned int : 0;
                unsigned int c : 4;
            };

            int main(void) {
                struct Bits bits;
                bits.a = 9;
                bits.b = -3;
                bits.c = 15;
                printf("%d %d %d %lu\n", bits.a, bits.b, bits.c, sizeof(struct Bits));
                return 0;
            }
        "#;
    assert_stdout(source, "1 -3 15 8\n");
}

#[test]
fn bit_field_serialization_merges_fields_in_the_same_storage_unit() {
    let source = r#"
            #include <string.h>
            struct Bits {
                unsigned int low : 4;
                unsigned int high : 4;
            };
            int main(void) {
                struct Bits bits = {1, 2};
                unsigned char byte = 0;
                memcpy(&byte, &bits, 1);
                return byte != 0x21;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn bit_field_initializers_are_truncated_to_field_width() {
    let source = r#"
            #include <stdio.h>
            struct Bits {
                unsigned int a : 3;
                signed int b : 4;
            };

            int main(void) {
                struct Bits bits = { 9, -9 };
                printf("%d %d\n", bits.a, bits.b);
                return 0;
            }
        "#;
    assert_stdout(source, "1 7\n");
}

#[test]
fn narrow_unsigned_bit_fields_promote_to_int() {
    let source = r#"
            struct Bits { unsigned int value : 1; };
            int main(void) {
                struct Bits bits = {1};
                return ~bits.value < 0 ? 0 : 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn nested_string_literal_initializer_for_array_member_works() {
    let source = r#"
            #include <stdio.h>
            struct Message {
                char text[4];
            };

            int main(void) {
                struct Message msg = { "hi" };
                printf("%s %d\n", &msg.text[0], msg.text[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "hi 0\n");
}

#[test]
fn aggregate_initializers_support_designators_brace_elision_and_array_bound_deduction() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int x; int y; };
            struct Outer {
                int a[2];
                struct Pair pair;
                union {
                    int tag;
                    unsigned char bytes[4];
                } u;
                char text[4];
            };

            int main(void) {
                struct Pair items[] = { 1, 2, 3, 4 };
                struct Outer outer = {
                    { 7, 8 },
                    .pair.y = 9,
                    .u.bytes = { 65, 66, 0, 0 },
                    "hi"
                };
                printf("%lu %d %d %d %d %u %s\n",
                       sizeof(items),
                       items[1].x,
                       items[1].y,
                       outer.a[0],
                       outer.pair.y,
                       (unsigned int)outer.u.bytes[1],
                       &outer.text[0]);
                return 0;
            }
        "#;
    assert_stdout(source, "16 3 4 7 9 66 hi\n");
}

#[test]
fn union_initializer_accepts_multiple_designators_for_one_struct_member() {
    let source = r#"
            #include <stdio.h>
            union Animal {
                struct { int type; int loudness; } antelope;
                struct { int type; int sea_creature; double intelligence; } octopus;
            };

            int main(void) {
                union Animal animal = {
                    .octopus.type = 2,
                    .octopus.sea_creature = 1,
                    .octopus.intelligence = 12.8
                };
                printf("%d %d %.1f\n", animal.octopus.type,
                       animal.octopus.sea_creature,
                       animal.octopus.intelligence);
                return 0;
            }
        "#;
    assert_stdout(source, "2 1 12.8\n");
}

#[test]
fn later_union_designator_selects_the_last_initialized_member() {
    let source = r#"
            union Value { int first; int second; };
            int main(void) {
                union Value value = { .first = 1, .second = 2 };
                return value.second != 2;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn file_scope_incomplete_array_bounds_respect_brace_elision() {
    let source = r#"
            struct Pair { int x; int y; };
            static const struct Pair pairs[] = { 1, 2, 3, 4 };

            int main(void) {
                return sizeof pairs != 2 * sizeof(struct Pair)
                    || pairs[1].x != 3 || pairs[1].y != 4;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn nested_array_member_in_array_of_structs_works() {
    let source = r#"
            #include <stdio.h>
            struct Item { int values[3]; };

            int main(void) {
                struct Item items[2];
                items[1].values[2] = 9;
                printf("%d\n", items[1].values[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "9\n");
}

#[test]
fn union_member_subobject_write_preserves_other_bytes_from_current_representation() {
    let source = r#"
            #include <stdio.h>
            struct Pair { unsigned char lo; unsigned char hi; };
            union U {
                struct Pair pair;
                unsigned short raw;
            };

            int main(void) {
                union U u;
                u.raw = 0x1234;
                u.pair.lo = 0x56;
                printf("%u %u\n", (unsigned int)u.pair.lo, (unsigned int)u.pair.hi);
                return 0;
            }
        "#;
    assert_stdout(source, "86 18\n");
}

#[test]
fn suitably_converted_union_pointer_points_to_each_member() {
    let source = r#"
            #include <stdio.h>
            union Value { int integer; float real; };

            int main(void) {
                union Value value;
                int *integer = (int *)&value;
                float *real = (float *)&value;
                value.integer = 12;
                printf("%d ", *integer);
                value.real = 3.25f;
                printf("%.2f\n", *real);
                return 0;
            }
        "#;
    assert_stdout(source, "12 3.25\n");
}

#[test]
fn suitably_converted_union_member_pointer_points_back_to_union() {
    let source = r#"
            union Value { int integer; float real; };
            int main(void) {
                union Value value;
                int *member = &value.integer;
                union Value *whole = (union Value *)member;
                whole->real = 4.5f;
                return value.real != 4.5f;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn union_punning_to_invalid_bool_representation_is_ub() {
    let source = r#"
            #include <stdio.h>
            union U {
                unsigned char raw;
                _Bool flag;
            };

            int main(void) {
                union U u;
                u.raw = 2;
                printf("%d\n", u.flag);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("indeterminate _Bool"));
    assert!(rendered.contains("object representation"));
}

#[test]
fn union_punning_to_invalid_pointer_representation_is_ub() {
    let source = r#"
            union U {
                unsigned long long bits;
                int *ptr;
            };

            int main(void) {
                union U u;
                u.bits = 1ULL;
                int *p = u.ptr;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("invalid object representation"));
    assert!(rendered.contains("pointer"));
}

#[test]
fn cannot_take_address_of_bit_field() {
    let source = r#"
            struct Bits { unsigned int a : 3; };

            int main(void) {
                struct Bits bits;
                unsigned int *p = &bits.a;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "address of a bit-field");
}

#[test]
fn sizeof_bit_field_is_rejected() {
    let source = r#"
            #include <stdio.h>
            struct Bits { unsigned int a : 3; };

            int main(void) {
                struct Bits bits;
                printf("%lu\n", sizeof bits.a);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "sizeof cannot be applied to a bit-field");
}

#[test]
fn misaligned_pointer_cast_is_ub_even_without_dereference() {
    let source = r#"
            int main(void) {
                int a[2];
                int *p = (int*)((unsigned char*)&a[0] + 1);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "not correctly aligned");
}

#[test]
fn cast_from_noninitial_member_pointer_to_struct_pointer_is_ub() {
    let source = r#"
            struct S {
                int x;
                int y;
            };

            int main(void) {
                struct S a[2];
                struct S *s = (struct S*)(&a[0].y);
                s->x = 0;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("undefined behavior"));
    assert!(rendered.contains("pointer is not valid to access"));
}

#[test]
fn cast_from_initial_member_pointer_to_struct_pointer_is_allowed() {
    let source = r#"
            #include <stdio.h>
            struct S {
                int x;
                int y;
            };

            int main(void) {
                struct S a[2];
                struct S *s = (struct S*)(&a[1].x);
                s->y = 9;
                printf("%d\n", a[1].y);
                return 0;
            }
        "#;
    assert_stdout(source, "9\n");
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
fn shared_header_record_member_types_work_across_translation_units() {
    let project = TestProject::new("cross-tu-record-member-types");
    project.write("shared.h",
            "typedef struct Node Node;\nstruct Node { Node *next; };\ntypedef enum { MODE_A, MODE_B } Mode;\ntypedef int (*Callback)(Mode mode);\nNode *choose(Node *node);\nNode *extend(Node *node);\nint invoke(Callback callback, Mode mode);\n",
        )
        .unwrap();
    project.write("main.c",
            "#include \"shared.h\"\nint callback(Mode mode) { return mode + 40; }\nint main(void) { return choose(extend(0)) == 0 || invoke(callback, MODE_B) != 41; }\n",
        )
        .unwrap();
    project.write("helper.c",
            "#include \"shared.h\"\nNode *choose(Node *node) { return node->next ? node->next : extend(node); }\nNode *extend(Node *node) { static Node fallback; return node == 0 ? &fallback : node; }\nint invoke(Callback callback, Mode mode) { return callback(mode); }\n",
        )
        .unwrap();

    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn incomplete_record_pointer_member_uses_complete_cross_unit_type() {
    let project = TestProject::new("cross-tu-incomplete-record-pointer-member");
    project.write("shared.h",
            "typedef struct Node Node;\ntypedef struct { Node **nodes; } Base;\ntypedef struct { Node *node; } Iterator;\nvoid initialize(Base *base);\nIterator make_iterator(void);\nchar *next(Base *base, Iterator *iterator);\n",
        )
        .unwrap();
    project.write("main.c",
            "#include \"shared.h\"\nint main(void) { Base base = {0}; initialize(&base); Iterator iterator = make_iterator(); return next(&base, &iterator) == 0; }\n",
        )
        .unwrap();
    project.write("helper.c",
            "#include <stdlib.h>\n#include \"shared.h\"\nstruct Node { int value; };\nvoid initialize(Base *base) { base->nodes = malloc(sizeof(*base->nodes)); base->nodes[0] = malloc(sizeof(*base->nodes[0])); }\nIterator make_iterator(void) { Iterator iterator; iterator.node = 0; return iterator; }\nchar *next(Base *base, Iterator *iterator) { iterator->node = base->nodes[0]; return (char *)(iterator->node + 1); }\n",
        )
        .unwrap();

    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn unnamed_nested_unions_are_compared_structurally_across_translation_units() {
    let project = TestProject::new("cross-tu-unnamed-union");
    project.write("main.c",
            "#include <stdio.h>\nstruct Token { union { int x; unsigned char *s; } u; int kind; };\nint read_token(const struct Token *p);\nint main(void) { struct Token token = { .u.x = 7, .kind = 1 }; printf(\"%d\\n\", read_token(&token)); return 0; }\n",
        )
        .unwrap();
    project.write("helper.c",
            "struct Token { union { int x; unsigned char *s; } u; int kind; };\nint read_token(const struct Token *p) { return p->u.x; }\n",
        )
        .unwrap();

    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "7\n");
}

#[test]
fn call_without_visible_declaration_still_fails_across_multiple_files() {
    let project = TestProject::new("multi-input-undeclared");
    project
        .write("main.c", "int main(void) { helper(); return 0; }\n")
        .unwrap();
    project
        .write("helper.c", "int helper(void) { return 7; }\n")
        .unwrap();

    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("undeclared identifier helper"));
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
fn reading_indeterminate_bool_is_ub_even_after_address_taken() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                _Bool b;
                _Bool *p = &b;
                printf("%d\n", b);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("indeterminate _Bool"));
    assert!(rendered.contains("object representation"));
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
fn memcmp_with_zero_count_returns_zero_without_touching_bytes() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>
            int main(void) {
                int a;
                int b;
                printf("%d\n", memcmp(&a, &b, 0));
                return 0;
            }
        "#;
    assert_stdout(source, "0\n");
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
fn typedef_name_can_be_used_for_object_declarations() {
    let source = r#"
            #include <stdio.h>
            typedef unsigned long word;

            int main(void) {
                word value = 42;
                printf("%lu\n", value);
                return 0;
            }
        "#;
    assert_stdout(source, "42\n");
}

#[test]
fn function_type_typedef_can_declare_a_prototype() {
    let source = r#"
            #include <stdio.h>
            typedef int F(void);
            F f;

            int main(void) {
                printf("%d\n", f());
                return 0;
            }

            int f(void) {
                return 7;
            }
        "#;
    assert_stdout(source, "7\n");
}

#[test]
fn invalid_restrict_usage_is_rejected() {
    let source = r#"
            int main(void) {
                restrict int x = 0;
                return x;
            }
        "#;
    assert_diagnostic_contains(source, "restrict qualifier requires a pointer type");
}

#[test]
fn restrict_qualified_pointer_declaration_works() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int *restrict p = &x;
                *p = 5;
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
}

#[test]
fn function_pointers_cannot_be_restrict_qualified() {
    let source = "int main(void) { int (* restrict function)(void) = 0; return 0; }";
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("must point to a complete object type")
    );
}

#[test]
fn restrict_qualified_pointers_must_point_to_complete_object_types() {
    for source in [
        "int main(void) { void *restrict value = 0; return value != 0; }",
        "struct S; int main(void) { struct S *restrict value = 0; return value != 0; }",
    ] {
        assert_diagnostic_contains(source, "must point to a complete object type");
    }
}

#[test]
fn repeated_qualifiers_from_typedefs_are_idempotent() {
    let source = r#"
            typedef const int ConstInt;
            typedef volatile ConstInt VolatileConstInt;
            int main(void) {
                const ConstInt first = 1;
                const volatile VolatileConstInt second = 2;
                return first != 1 || second != 2;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
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
fn direct_access_after_restrict_write_is_ub() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int *restrict p = &x;
                *p = 5;
                printf("%d\n", x);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified pointer bases"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn restrict_tracking_is_enabled_for_nested_for_declarations() {
    let source = r#"
            int main(void) {
                int x = 0;
                for (int *restrict p = &x; p; p = 0) {
                    *p = 5;
                    x = 6;
                }
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified pointer bases"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn restrict_write_after_prior_unrestricted_read_is_ub() {
    let source = r#"
            int main(void) {
                int x = 0;
                int *restrict p = &x;
                int before = x;
                *p = 5;
                return before;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified pointer bases"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn overlapping_restrict_parameters_are_ub() {
    let source = r#"
            void f(int *restrict a, int *restrict b) {
                *a = *b;
            }

            int main(void) {
                int x = 0;
                f(&x, &x);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified pointer bases"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn derived_pointer_from_restrict_base_is_allowed() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x = 0;
                int *restrict p = &x;
                int *q = p;
                *q = 5;
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_stdout(source, "5\n");
}

#[test]
fn disjoint_subobjects_do_not_violate_restrict() {
    let source = r#"
            #include <stdio.h>
            void f(int *restrict a, int *restrict b) {
                *a = 1;
                *b = 2;
            }

            int main(void) {
                int x[2] = { 0, 0 };
                f(&x[0], &x[1]);
                printf("%d %d\n", x[0], x[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "1 2\n");
}

#[test]
fn restrict_round_trip_through_void_pointer_then_new_restrict_base_is_ub() {
    let source = r#"
            int main(void) {
                int x;
                int *restrict p = &x;
                *p = 0;
                void *pv = p;
                int *restrict p2 = pv;
                *p2 = 1;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("restrict-qualified pointer bases"));
    assert!(rendered.contains("6.7.3.1"));
}

#[test]
fn restrict_round_trip_through_void_pointer_same_base_is_allowed() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x;
                int *restrict p = &x;
                *p = 0;
                *(int *)(void *)p = 1;
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn restrict_accesses_stop_constraining_after_block_ends() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int x;
                {
                    int *restrict p = &x;
                    *p = 1;
                }
                x = 0;
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "0\n");
}

#[test]
fn taking_address_of_register_object_is_rejected() {
    let source = r#"
            int main(void) {
                register int x = 1;
                int *p = &x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "cannot take the address of a register object");
}

#[test]
fn register_array_cannot_undergo_array_to_pointer_conversion() {
    for body in ["int *p = values; return *p;", "return values[0];"] {
        let source = format!("int main(void) {{ register int values[2] = {{1, 2}}; {body} }}");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(err.render().contains("array-to-pointer conversion"));
    }
    run_source(
        "test.c",
        "int main(void) { register int values[2]; return sizeof values != 8; }",
    )
    .unwrap();
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
fn extern_global_declaration_resolves_to_definition_in_another_file() {
    let project = TestProject::new("extern-global");
    project
        .write(
            "main.c",
            r#"
                #include <stdio.h>

                extern int value;

                int main(void) {
                    printf("%d\n", value);
                    return 0;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                int value = 9;

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "9\n");
}

#[test]
fn block_extern_declaration_resolves_to_definition_in_another_file() {
    let project = TestProject::new("block-extern-global");
    project
        .write(
            "main.c",
            "int main(void) { extern int value; return value - 9; }\n",
        )
        .unwrap();
    project.write("helper.c", "int value = 9;\n").unwrap();
    project.run(["main.c", "helper.c"]).unwrap();
}

#[test]
fn block_linkage_declarations_are_checked_across_translation_units() {
    let project = TestProject::new("block-extern-conflict");
    project
        .write(
            "main.c",
            "int main(void) { extern double value; return 0; }\n",
        )
        .unwrap();
    project.write("helper.c", "int value = 9;\n").unwrap();
    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    assert!(err.render().contains("conflicting declarations"));
}

#[test]
fn block_function_declaration_resolves_to_definition_in_another_file() {
    let project = TestProject::new("block-function-declaration");
    project
        .write(
            "main.c",
            "int main(void) { int helper(void); return helper() - 9; }\n",
        )
        .unwrap();
    project
        .write("helper.c", "int helper(void) { return 9; }\n")
        .unwrap();
    project.run(["main.c", "helper.c"]).unwrap();
}

#[test]
fn block_scope_extern_with_initializer_is_rejected() {
    let source = r#"
            int value;

            int main(void) {
                extern int value = 1;
                return value;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "block-scope extern declaration cannot have an initializer",
    );
}

#[test]
fn block_scope_linkage_declarations_obey_storage_and_type_constraints() {
    for (source, expected) in [
        (
            "int main(void) { static int helper(void); return 0; }\n",
            "may only explicitly specify extern",
        ),
        (
            "int main(void) { int n = 2; extern int values[n]; return 0; }\n",
            "with linkage cannot have variably modified type",
        ),
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains(expected), "{}", err.render());
    }
}

#[test]
fn duplicate_local_declarations_are_rejected() {
    let invalid_declarations = [
        "int a; int a;",
        "int a, a;",
        "static int a; static int a;",
        "int a; typedef int a;",
        "typedef int a; int a;",
        "enum { a }; int a;",
        "int a; int a(void);",
        "int a(void); int a;",
        "extern int a; int a;",
    ];
    for declarations in invalid_declarations {
        let source = format!("int main(void) {{ {declarations} return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("redefinition of a")
                || rendered.contains("a is already declared as a different kind of symbol"),
            "unexpected diagnostic for {declarations:?}: {rendered}"
        );
    }
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
fn duplicate_parameters_and_parameter_body_redeclarations_are_rejected() {
    for source in [
        "int f(int a, int a) { return a; }\nint main(void) { return 0; }\n",
        "int f(int a) { int a; return a; }\nint main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains("redefinition of a"));
    }
}

#[test]
fn definition_parameters_require_names_but_prototype_parameters_do_not() {
    run_source(
        "test.c",
        "int helper(int, const char *, int [3], int (*)(int));\nint main(void) { return 0; }\n",
    )
    .unwrap();

    for source in [
        "int helper(int) { return 0; }\nint main(void) { return helper(1); }\n",
        "int helper(int named, int) { return named; }\nint main(void) { return helper(0, 1); }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(
            err.render()
                .contains("function definition parameters must have names")
        );
    }
}

#[test]
fn compatible_same_scope_redeclarations_remain_allowed() {
    let source = r#"
            int main(void) {
                extern int external_value;
                extern int external_value;
                typedef int number;
                typedef int number;
                int shadowed;
                {
                    double shadowed;
                }
                int helper(void);
                int helper(void);
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn incompatible_same_scope_redeclarations_are_rejected() {
    for declarations in [
        "extern int a; extern double a;",
        "typedef int a; typedef double a;",
        "int a(void); double a(void);",
    ] {
        let source = format!("int main(void) {{ {declarations} return 0; }}\n");
        let err = run_source("test.c", &source).unwrap_err();
        assert!(
            err.render().contains("conflicting"),
            "unexpected diagnostic for {declarations:?}: {}",
            err.render()
        );
    }
}

#[test]
fn block_linkage_declarations_must_match_visible_linkage_declarations() {
    for source in [
        "double a; int main(void) { extern int a; return 0; }\n",
        "int f; int main(void) { { int f(void); } return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("conflicting")
                || rendered.contains("already declared as a different kind"),
            "unexpected diagnostic: {rendered}"
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
fn block_function_declaration_shadows_a_local_and_refers_to_the_external_function() {
    let source = r#"
            int helper(void) { return 9; }

            int main(void) {
                int helper = 2;
                {
                    int helper(void);
                    return helper() - 9;
                }
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn block_linkage_declarations_inherit_visible_internal_linkage() {
    let source = r#"
            static int value = 4;
            static int helper(void) { return 5; }

            int main(void) {
                extern int value;
                int helper(void);
                return value + helper() - 9;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn block_linkage_declarations_are_checked_even_when_a_local_hides_the_file_declaration() {
    for source in [
        "int value; int main(void) { int value; { extern double value; } return 0; }\n",
        "int helper(int); int main(void) { int helper; { double helper(int); } return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(
            err.render().contains("conflicting declarations"),
            "unexpected diagnostic: {}",
            err.render()
        );
    }
}

#[test]
fn use_of_undefined_block_extern_is_not_reported_as_undeclared() {
    let source = "int main(void) { extern int missing; return missing; }\n";
    let rendered = rendered_diagnostic(source);
    assert!(
        rendered.contains("does not provide a definition"),
        "{rendered}"
    );
    assert!(!rendered.contains("undeclared identifier"), "{rendered}");
}

#[test]
fn inline_definition_without_external_definition_is_not_callable() {
    let source = r#"
            #include <stdio.h>
            inline int f(void) {
                return 3;
            }

            int main(void) {
                printf("%d\n", f());
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn extern_inline_definition_provides_external_definition() {
    let source = r#"
            #include <stdio.h>
            extern inline int f(void) {
                return 3;
            }

            int main(void) {
                printf("%d\n", f());
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn inline_definition_with_plain_declaration_is_external_definition() {
    let source = r#"
            #include <stdio.h>
            int f(void);
            inline int f(void) {
                return 4;
            }

            int main(void) {
                printf("%d\n", f());
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn inline_declaration_with_external_linkage_requires_definition_in_same_file() {
    let source = r#"
            inline int f(void);

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "requires a definition in the same translation unit");
}

#[test]
fn inline_main_is_rejected() {
    let source = r#"
            inline int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "main shall not be declared inline");
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
fn inline_definition_cannot_reference_internal_linkage_identifier() {
    let source = r#"
            static int helper(void) {
                return 7;
            }

            inline int f(void) {
                return helper();
            }

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "internal-linkage identifier helper");
}

#[test]
fn inline_definition_cannot_define_modifiable_static_local() {
    let source = r#"
            inline int f(void) {
                static int counter;
                return counter;
            }

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "modifiable object with static storage duration");
}

#[test]
fn inline_definition_can_define_static_const_local() {
    let source = r#"
            inline int f(void) {
                static const int value = 6;
                return value;
            }

            int main(void) {
                return 0;
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
fn goto_past_fixed_declaration_creates_the_uninitialized_inner_object() {
    let source = r#"
            int main(void) {
                int x = 0;
                {
                    goto label;
                    int x = 99;
                label:
                    x = 5;
                }
                return x;
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
fn backward_goto_ends_crossed_vla_lifetime() {
    let source = r#"
            int main(void) {
                int pass = 0;
                int *old = 0;
            again:
                ;
                int n = 1;
                int values[n];
                if (pass++ == 0) {
                    old = values;
                    goto again;
                }
                return old == values;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render().contains("lifetime has ended"),
        "{}",
        err.render()
    );
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
fn goto_requires_a_declared_label() {
    let source = r#"
            int main(void) {
                goto missing;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "use of undeclared label missing");
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
fn compound_literal_struct_and_scalar_work() {
    let source = r#"
            #include <stdio.h>
            struct Pair { int x; int y; };

            int main(void) {
                struct Pair p = (struct Pair){ 3, 4 };
                int *q = &(int){ 9 };
                printf("%d %d %d\n", p.x, p.y, *q);
                return 0;
            }
        "#;
    assert_stdout(source, "3 4 9\n");
}

#[test]
fn compound_literals_are_postfix_expressions_for_member_access() {
    assert_exit_status(
        "struct S { int x; }; int main(void) { return (struct S){42}.x != 42; }",
        0,
    );
}

#[test]
fn array_members_of_non_lvalue_structures_use_full_expression_temporaries() {
    assert_exit_status(
        r#"
            struct S { int a[2]; };
            struct S make(void) { struct S s = {{7, 9}}; return s; }
            int main(void) { return make().a[1] != 9; }
        "#,
        0,
    );
}

#[test]
fn pointers_to_temporary_structure_array_members_expire_after_the_full_expression() {
    assert_diagnostic_contains(
        r#"
            struct S { int a[1]; };
            struct S make(void) { struct S s = {{7}}; return s; }
            int main(void) {
                int *p = make().a;
                return *p;
            }
        "#,
        "lifetime",
    );
}

#[test]
fn qualified_structure_array_members_have_qualified_elements() {
    assert_exit_status(
        r#"
            struct S { int a[1]; };
            int main(void) {
                struct S s = {{0}};
                const struct S value = {{0}};
                const struct S *pointer = &s;
                int dot = _Generic(&value.a[0], const int *: 0, default: 1);
                int arrow = _Generic(&pointer->a[0], const int *: 0, default: 1);
                return dot + 2 * arrow;
            }
        "#,
        0,
    );
}

#[test]
fn array_cannot_be_initialized_from_an_array_compound_literal_expression() {
    for source in [
        "int values[] = (int[]){1, 2, 3}; int main(void) { return 0; }\n",
        "int main(void) { int values[] = (int[]){1, 2, 3}; return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(
                err.render().contains(
                    "an array cannot be initialized by copying another array; use a brace-enclosed list instead"
                ),
                "{}",
                err.render()
            );
    }
}

#[test]
fn array_initializer_diagnostics_explain_the_applicable_forms() {
    for (source, expected) in [
        (
            "int main(void) { int a[3] = \"hi\"; }\n",
            "cannot initialize int[3] with an ordinary string literal; use a brace-enclosed list or change the array element type to char",
        ),
        (
            "int main(void) { float a[3] = \"hi\"; }\n",
            "cannot initialize float[3] with an ordinary string literal; use a brace-enclosed list or change the array element type to char",
        ),
        (
            "int main(void) { char a[3] = 1; }\n",
            "an array of this type must use a string literal or a brace-enclosed list",
        ),
        (
            "int main(void) { float a[3] = 1; }\n",
            "an array of this type must use a brace-enclosed list",
        ),
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains(expected), "{}", err.render());
    }
}

#[test]
fn array_copy_initializer_diagnostic_does_not_report_the_decayed_pointer_type() {
    let err = run_source(
        "test.c",
        "int main(void) { int a[3] = {1, 2, 3}; int b[3] = a; }\n",
    )
    .unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("an array cannot be initialized by copying another array"));
    assert!(!rendered.contains("int*"), "{rendered}");
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
fn backward_goto_reuses_non_vla_automatic_objects_in_the_active_block() {
    let source = r#"
            int main(void) {
                int pass = 0;
                int *first = 0;

            again:
                ;
                int value;
                if (pass++ == 0) {
                    value = 7;
                    first = &value;
                    goto again;
                }
                return &value != first;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn backward_goto_reruns_an_automatic_initializer_on_the_same_object() {
    let source = r#"
            int main(void) {
                int pass = 0;
                int *first = 0;

            again:
                ;
                int value = pass + 1;
                if (pass++ == 0) {
                    first = &value;
                    goto again;
                }
                return &value != first || value != 2;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn array_of_structs_accepts_struct_value_initializers_and_addressed_array_compound_literals() {
    let source = r#"
            struct trie {
                struct trie *children;
                int n;
            };

            int main(void) {
                struct trie *p = (struct trie[]) {
                    (struct trie){ .children = 0, .n = 1 }
                };
                return p[0].n - 1;
            }
        "#;
    assert_stdout(source, "");
}

#[test]
fn malloc_allocates_raw_storage_for_typed_access() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>
            struct Pair { int x; int y; };
            struct Raw {
                int prefix;
                union {
                    long alignment;
                    unsigned char bytes[sizeof(struct Pair)];
                } payload;
            };
            int main(void) {
                int *p = (int *)malloc(2 * sizeof(int));
                p[0] = 5;
                p[1] = 7;
                struct Raw *raw = malloc(sizeof *raw);
                struct Pair *pair = (struct Pair *)raw->payload.bytes;
                pair->x = 19;
                pair->y = 23;
                printf("%d %d %d\n", p[0], p[1], pair->x + pair->y);
                free(NULL);
                free(p);
                free(raw);
                return 0;
            }
        "#;
    assert_stdout(source, "5 7 42\n");
}

#[test]
fn allocated_flexible_array_struct_copy_exposes_nested_bit_field_subobjects() {
    let source = r#"
            #include <stdlib.h>
            struct List {
                int count;
                struct Item {
                    void *value;
                    char *name;
                    struct {
                        unsigned char order;
                        unsigned kind : 2;
                        unsigned done : 1;
                    } flags;
                } items[];
            };
            static const struct Item zero_item = {0};
            int main(void) {
                struct List *list = malloc(
                    sizeof(*list) + 3 * sizeof(list->items[0])
                );
                list->count = 3;
                for (int i = 0; i < list->count; ++i) {
                    list->items[i] = zero_item;
                }
                list->items[1].name = "named";
                for (int i = 0; i < list->count; ++i) {
                    if (list->items[i].flags.kind == 0
                        && list->items[i].name != 0) {
                        return i == 1 ? 0 : 1;
                    }
                }
                return 2;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn allocated_union_allows_nested_access_through_an_alternate_member() {
    let source = r#"
            #include <stdlib.h>
            struct List {
                int count;
                union Item {
                    struct {
                        unsigned short first;
                        unsigned short second;
                    } pair;
                    int combined;
                } items[];
            };
            int main(void) {
                struct List *list = malloc(sizeof(*list) + sizeof(list->items[0]));
                list->count = 1;
                list->items[0].combined = 0;
                return list->items[0].pair.first;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn encoded_pointer_can_be_decoded_after_a_void_pointer_union_round_trip() {
    let source = r#"
            #include <stdlib.h>
            struct Node { int value; };
            union Pointer {
                void *untyped;
                struct Node *typed;
            };
            int main(void) {
                struct Node *node = malloc(sizeof(*node));
                union Pointer *pointer = malloc(sizeof(*pointer));
                node->value = 17;
                pointer->untyped = node;
                return pointer->typed->value != 17;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn one_past_pointer_encoding_does_not_collide_with_the_next_object() {
    let source = r#"
            #include <stdint.h>
            #include <stdlib.h>
            struct Value { long value; };
            union Pointer {
                void *untyped;
                struct Value *typed;
            };
            int main(void) {
                char *first = malloc(16);
                struct Value *one_past = (struct Value *)(first + 16);
                uintptr_t encoded_one_past = (uintptr_t)one_past;
                struct Value *second = malloc(sizeof(*second));
                union Pointer pointer;
                second->value = 42;
                pointer.untyped = second;
                return encoded_one_past == 0 || pointer.typed->value != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn allocated_struct_assignment_preserves_nested_pointer_values() {
    let source = r#"
            #include <stdlib.h>
            struct Item {
                int number;
                union {
                    struct { char *name; } named;
                    long bits;
                } value;
            };
            int main(void) {
                char text[] = "kept";
                struct Item source = {7, {.named = {text}}};
                struct Item *copy = malloc(sizeof(*copy));
                *copy = source;
                return copy->number != 7 || copy->value.named.name != text;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn allocated_flexible_array_struct_assignment_and_realloc_preserve_pointers() {
    let source = r#"
            #include <stdlib.h>
            struct Item { char *name; void *first; void *second; };
            struct List { int count; int flag; void *outer; struct Item items[]; };
            int main(void) {
                char text[] = "kept";
                struct Item *source = malloc(sizeof(*source));
                struct List *list = calloc(1, sizeof(*list) + sizeof(list->items[0]));
                source->name = text;
                source->first = 0;
                source->second = 0;
                list->items[list->count++] = *source;
                list = realloc(list, sizeof(*list) + 2 * sizeof(list->items[0]));
                return list->items[0].name != text;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn struct_member_store_replaces_overlapping_suballocator_effective_type() {
    let source = r#"
            #include <stdlib.h>
            struct Item {
                char *first;
                char *second;
                char *third;
                struct {
                    unsigned byte;
                    unsigned selected : 1;
                } flags;
            };
            int main(void) {
                unsigned char *arena = malloc(sizeof(struct Item));
                char **old = (char **)arena;
                old[0] = 0;
                old[1] = 0;
                old[2] = 0;
                old[3] = 0;
                struct Item *item = (struct Item *)arena;
                struct Item source = {0};
                source.flags.selected = 1;
                item->flags = source.flags;
                return item->flags.selected != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn indexed_flexible_struct_member_store_replaces_old_effective_type() {
    let source = r#"
            #include <stdlib.h>
            struct Item {
                char *first;
                char *second;
                char *third;
                struct { unsigned byte; unsigned selected : 1; } flags;
                long cursor;
                void *tail[4];
            };
            struct List { int count; unsigned allocated; struct Item items[]; };
            int main(void) {
                struct List *list = malloc(sizeof(*list) + 2 * sizeof(list->items[0]));
                char **old = (char **)((char *)&list->items[0] + sizeof(list->items[0]));
                old[0] = old[1] = old[2] = old[3] = 0;
                struct Item source = {0};
                source.flags.selected = 1;
                struct Item *item = &list->items[1];
                item->flags = source.flags;
                return item->flags.selected != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn pointer_member_survives_switching_nested_union_members_before_indirect_read() {
    let source = r#"
            struct Node { struct Node *next; };
            struct Iterator {
                int kind;
                union {
                    struct { struct Node *node; } list;
                    struct { int count; struct Node *nodes; } array;
                } value;
            };
            static struct Node *current(struct Iterator *iterator) {
                return iterator->value.list.node;
            }
            int main(void) {
                struct Node node = {0};
                struct Iterator iterator;
                iterator.kind = 0;
                iterator.value.array.nodes = 0;
                iterator.value.list.node = &node;
                return current(&iterator) != &node;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn allocated_storage_enforces_effective_type_for_unmodified_reads() {
    for source in [
        r#"
                #include <stdlib.h>
                int main(void) {
                    void *raw = malloc(sizeof(int));
                    *(int *)raw = 0;
                    return *(float *)raw != 0.0f;
                }
            "#,
        r#"
                #include <stdlib.h>
                #include <string.h>
                int main(void) {
                    int source = 0;
                    void *raw = malloc(sizeof source);
                    memcpy(raw, &source, sizeof source);
                    return *(float *)raw != 0.0f;
                }
            "#,
        r#"
                #include <stdlib.h>
                #include <string.h>
                int main(void) {
                    void *raw = malloc(sizeof(int));
                    *(int *)raw = 0;
                    memcpy(raw, raw, 0);
                    return *(float *)raw != 0.0f;
                }
            "#,
        r#"
                #include <stdlib.h>
                struct Pair { int x; int y; };
                int main(void) {
                    struct Pair initial = { 0x3f800000, 2 };
                    struct Pair *pair = malloc(sizeof initial);
                    *pair = initial;
                    pair->y = 3;
                    return *(float *)&pair->x == 1.0f;
                }
            "#,
        r#"
                #include <stdlib.h>
                #include <string.h>
                struct Pair { int x; int y; };
                int main(void) {
                    struct Pair initial = { 0x3f800000, 2 };
                    struct Pair *pair = malloc(sizeof initial);
                    memcpy(pair, &initial, sizeof initial);
                    pair->y = 3;
                    return *(float *)&pair->x == 1.0f;
                }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains("effective type"), "{}", err.render());
    }
}

#[test]
fn allocated_storage_effective_type_is_replaced_by_modifying_accesses() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>
            #include <string.h>
            int main(void) {
                void *direct = malloc(sizeof(float));
                *(int *)direct = 0;
                *(float *)direct = 1.0f;
                if (*(float *)direct != 1.0f) return 1;

                void *by_byte = malloc(sizeof(float));
                *(int *)by_byte = 0;
                *(unsigned char *)by_byte = 0;
                if (*(float *)by_byte != 0.0f) return 2;

                void *by_memset = malloc(sizeof(float));
                *(int *)by_memset = 1;
                memset(by_memset, 0, sizeof(float));
                if (*(float *)by_memset != 0.0f) return 3;

                float value = 2.0f;
                memcpy(direct, &value, sizeof value);
                if (*(float *)direct != 2.0f) return 4;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn aggregate_read_ignores_stale_effective_type_in_struct_padding() {
    let source = r#"
            #include <stdlib.h>
            struct Padded { int first; long long second; };
            int main(void) {
                unsigned char *arena = malloc(sizeof(struct Padded));
                *(int *)(arena + sizeof(int)) = 99;
                struct Padded *value = (struct Padded *)arena;
                value->first = 19;
                value->second = 23;
                struct Padded copy = *value;
                return copy.first + copy.second != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn allocated_storage_allows_character_and_corresponding_unsigned_aliases() {
    let source = r#"
            #include <stdlib.h>
            int main(void) {
                void *raw = malloc(sizeof(int));
                *(int *)raw = -1;
                unsigned char first = *(unsigned char *)raw;
                unsigned int value = *(unsigned int *)raw;
                return first != 255 || value != 4294967295U;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn declarator_name_is_visible_in_its_initializer() {
    let source = r#"
            #include <stdlib.h>
            struct Item { int value; };
            int main(void) {
                struct Item *item = malloc(sizeof *item);
                if (!item) return 1;
                item->value = 7;
                return item->value != 7;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn every_malloc_result_is_suitably_aligned_after_odd_sized_allocations() {
    let source = r#"
            #include <stdlib.h>
            int main(void) {
                void *odd = malloc(1);
                long double *wide = malloc(sizeof(long double));
                *wide = 3.0L;
                free(wide);
                free(odd);
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn struct_pointer_members_can_point_into_dynamic_raw_storage() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>
            struct T { int a; int b; };
            struct V { struct T *data; unsigned long len; };

            static struct V make(void) {
                return (struct V){ .data = malloc(2 * sizeof(struct T)), .len = 2 };
            }

            int main(void) {
                struct V v = make();
                v.data[0].a = 1;
                v.data[0].b = 2;
                v.data[1].a = 3;
                printf("%d %d %d %lu\n", v.data[0].a, v.data[0].b, v.data[1].a, v.len);
                return 0;
            }
        "#;
    assert_stdout(source, "1 2 3 2\n");
}

#[test]
fn flexible_array_members_work_through_malloc_backed_struct_pointers() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>
            struct S { int n; int a[]; };

            int main(void) {
                struct S *p = (struct S *)malloc(sizeof(struct S) + 2 * sizeof(int));
                p->n = 2;
                p->a[0] = 3;
                p->a[1] = 4;
                printf("%d %d %d\n", p->n, p->a[0], p->a[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "2 3 4\n");
}

#[test]
fn pointer_to_first_flexible_array_element_has_the_allocated_extent() {
    let source = r#"
            #include <stdlib.h>
            struct S { int count; int values[]; };
            int main(void) {
                struct S *list = malloc(sizeof(*list) + 2 * sizeof(list->values[0]));
                int *left = &list->values[0];
                int *right = &left[1];
                left[0] = 19;
                right[0] = 23;
                return right - left != 1 || left[0] + left[1] != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn malloc_backed_storage_can_be_rebased_after_passing_through_void_pointer() {
    let source = r#"
            #include <stdlib.h>

            struct First { long value; };
            struct Second { int left; int right; };

            int main(void) {
                void *storage = malloc(64);
                struct First *first = storage;
                void *erased = first;
                struct Second *second = erased;
                second->left = 19;
                second->right = 23;
                return second->left + second->right != 42;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn dynamic_array_pointer_arithmetic_is_relative_to_its_suballocation_origin() {
    let source = r#"
            #include <stdint.h>
            #include <stdlib.h>
            #include <string.h>

            struct Item { long values[3]; };
            struct Holder { struct Item *position; };
            union StackSlot {
                struct Item item;
                unsigned char bytes[sizeof(struct Item)];
            };

            int main(void) {
                char *storage = malloc(3 * sizeof(struct Item));
                struct Item *items = (struct Item *)(storage + sizeof(struct Item));
                struct Item *second = items + 1;
                second->values[0] = 23;
                (second - 1)->values[0] = 19;
                struct Holder *holder = malloc(sizeof *holder);
                struct Holder *copied = malloc(sizeof *copied);
                holder->position = items;

                struct Item *same_address_from_larger_array =
                    ((struct Item *)storage) + 1;
                uintptr_t encoded_collision =
                    (uintptr_t)same_address_from_larger_array;

                union StackSlot *stack = malloc(3 * sizeof *stack);
                union StackSlot *level = stack + 2;
                struct Item *initial_member = &stack[1].item;
                union StackSlot *member_round_trip =
                    (union StackSlot *)initial_member;

                memcpy(copied, holder, sizeof *copied);
                holder = realloc(holder, 2 * sizeof *holder);
                char *begin = (char *)items;
                char *end = (char *)(items + 2);
                return items[0].values[0] + items[1].values[0] != 42
                    || end - begin != 2 * sizeof(struct Item)
                    || encoded_collision == 0
                    || holder->position - items != 0
                    || copied->position - items != 0
                    || !(member_round_trip < level)
                    || member_round_trip - stack != 1;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn flexible_array_requires_another_named_member() {
    let invalid = "struct S { int : 3; int values[]; };\nint main(void) { return 0; }\n";
    assert_diagnostic_contains(invalid, "at least one other named member");

    let valid = r#"
            struct S {
                struct { int count; };
                int values[];
            };
            int main(void) { return sizeof(struct S) < sizeof(int); }
        "#;
    let result = run_source("test.c", valid).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn flexible_array_containers_cannot_be_struct_members_or_array_elements() {
    for source in [
        "struct Flex { int count; int values[]; };\nstruct Outer { int prefix; struct Flex flex; };\nint main(void) { return 0; }\n",
        "struct Flex { int count; int values[]; };\nstruct Flex items[2];\nint main(void) { return 0; }\n",
        "struct Flex { int count; int values[]; };\nunion Holder { struct Flex flex; int word; };\nunion Holder items[2];\nint main(void) { return 0; }\n",
        "struct Flex { int count; int values[]; };\nunion Holder { struct Flex flex; int word; };\nstruct Outer { union Holder holder; };\nint main(void) { return 0; }\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        let rendered = err.render();
        assert!(
            rendered.contains("cannot be a member of another structure")
                || rendered.contains("cannot be an array element"),
            "{rendered}"
        );
    }

    let valid = r#"
            struct Flex { int count; int values[]; };
            union Holder { struct Flex flex; int word; };
            int main(void) { union Holder holder; return sizeof holder < sizeof(int); }
        "#;
    let result = run_source("test.c", valid).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn flexible_array_member_pointers_survive_raw_storage_round_trips() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>
            struct S { int n; int a[]; };

            int main(void) {
                struct S *p = (struct S *)malloc(sizeof(struct S) + 2 * sizeof(int));
                int **slot = (int **)malloc(sizeof(int *));
                *slot = p->a;
                (*slot)[0] = 9;
                (*slot)[1] = 11;
                printf("%d %d\n", p->a[0], p->a[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "9 11\n");
}

#[test]
fn block_scope_vlas_work_and_sizeof_uses_runtime_extent() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 3;
                int a[n];
                a[0] = 2;
                a[2] = 4;
                printf("%lu %d %d\n", sizeof a, a[0], a[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "12 2 4\n");
}

#[test]
fn nested_block_scope_vlas_work() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 2;
                int m = 3;
                int a[n][m];
                a[1][2] = 7;
                printf("%lu %d\n", sizeof a, a[1][2]);
                return 0;
            }
        "#;
    assert_stdout(source, "24 7\n");
}

#[test]
fn block_scope_vla_typedef_captures_its_bound_when_declared() {
    let source = r#"
            int f(int n) {
                typedef int Row[n];
                n += 1;
                Row captured;
                int current[n];
                captured[0] = 7;
                return captured[0] != 7
                    || sizeof captured != 2 * sizeof(int)
                    || sizeof current != 3 * sizeof(int);
            }
            int main(void) { return f(2); }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn unresolved_linkage_uses_are_diagnosed_even_on_untaken_paths() {
    for source in [
        "extern int missing; int main(void) { if (0) return missing; return 0; }\n",
        "static int missing(void); int main(void) { if (0) return missing(); return 0; }\n",
    ] {
        let diagnostic = rendered_diagnostic(source);
        assert!(
            diagnostic.contains("does not provide a definition"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("standard: 6.9"), "{diagnostic}");
    }
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
fn atomic_aggregate_members_allow_sizeof_but_reject_reads_and_writes() {
    for access in [
        "return s.x;",
        "s.x = 1; return 0;",
        "return p->x;",
        "p->x = 1; return 0;",
    ] {
        let source = format!(
            "struct S {{ int x; }}; int main(void) {{ _Atomic(struct S) s = {{0}}; _Atomic(struct S) *p = &s; {access} }}"
        );
        assert_diagnostic_contains(&source, "access to a member of an atomic");
    }
    assert_exit_status(
        "struct S { int x; }; _Atomic(struct S) s; _Atomic(struct S) *p = &s; int main(void) { return sizeof s.x != sizeof(int) || sizeof p->x != sizeof(int); }",
        0,
    );
}

#[test]
fn sizeof_type_name_supports_vla_bounds() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 5;
                printf("%lu\n", sizeof(int[n]));
                return 0;
            }
        "#;
    assert_stdout(source, "20\n");
}

#[test]
fn sizeof_accepts_abstract_function_pointers_and_unparenthesized_compound_literals() {
    assert_exit_status(
        r#"
        struct S { int x; };
        int main(void) {
            int n = 0;
            return sizeof(int *(*) (int)) != sizeof(void *)
                || sizeof (int){ ++n } != sizeof(int)
                || sizeof (struct S){1}.x != sizeof(int)
                || sizeof (int[]){1, 2} != 2 * sizeof(int)
                || n != 0;
        }
    "#,
        0,
    );
}

#[test]
fn sizeof_vla_type_name_evaluates_the_bound_expression() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 2;
                printf("%lu %d\n", sizeof(int[++n]), n);
                return 0;
            }
        "#;
    assert_stdout(source, "12 3\n");
}

#[test]
fn sizeof_vla_expression_evaluates_the_operand() {
    let source = r#"
            int main(void) {
                int n = 2;
                int a[n];
                int (*p)[n] = &a;
                sizeof *p++;
                return p == &a;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn sizeof_pointer_to_vla_expression_does_not_evaluate_the_operand() {
    let source = r#"
            int main(void) {
                int n = 2;
                int a[n];
                int (*p)[n] = &a;
                sizeof p++;
                return p != &a;
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
fn generic_selection_preserves_lvalue_and_function_designator_categories() {
    let source = r#"
            int f(void) { return 7; }

            int main(void) {
                int x = 3;
                _Generic(x, int: x, default: x) = 9;
                int *q = &_Generic(x, int: x, default: x);
                int (*p)(void) = &_Generic(1, int: f, default: f);
                return x != 9 || *q != 9 || p() != 7;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn address_of_dereferenced_void_pointer_cancels_without_access() {
    let source = r#"
            int main(void) {
                void *p = 0;
                void *q = &*p;
                return q != p;
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
fn sizeof_non_vla_expression_is_an_integer_constant_expression() {
    let source = r#"
            int main(void) {
                int value;
                switch (0) {
                    case sizeof value: return 1;
                    default: return 0;
                }
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
fn immediate_floating_constant_cast_to_zero_is_a_null_pointer_constant() {
    let source = r#"
            int main(void) {
                int *pointer = (int)0.0;
                return pointer != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn aligned_object_pointer_cast_can_round_trip_without_dereference() {
    let source = r#"
            struct S { int x; int y; };
            int main(void) {
                struct S value;
                struct S *as_struct = (struct S *)&value.y;
                int *round_trip = (int *)as_struct;
                return round_trip != &value.y;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn generic_selection_can_be_an_integer_constant_expression() {
    let source = r#"
            enum { VALUE = _Generic(1, int: 7, default: 9) };
            int main(void) {
                return VALUE != 7;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn sizeof_vla_is_not_a_null_pointer_constant() {
    let source = r#"
            int main(void) {
                int n = 2;
                int values[n];
                int *pointer = sizeof values - sizeof values;
                return pointer != 0;
            }
        "#;
    assert_diagnostic_contains(source, "the expression is not a null pointer constant");
}

#[test]
fn sizeof_vla_is_not_a_static_initializer_constant() {
    let source = r#"
            int main(void) {
                int n = 2;
                int values[n];
                static int size = sizeof values;
                return size;
            }
        "#;
    assert_diagnostic_contains(source, "not a compile-time constant");
}

#[test]
fn vla_type_names_work_in_casts() {
    let source = r#"
            #include <stdio.h>
            int main(void) {
                int n = 3;
                int a[3];
                a[2] = 4;
                int (*p)[n] = (int (*)[n])&a;
                printf("%d\n", (*p)[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn vla_type_names_are_rejected_in_compound_literals() {
    let source = r#"
            int main(void) {
                int n = 3;
                int *p = (int[n]){1, 2, 5};
                return p[0];
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("compound literal cannot have variable length array type")
    );
}

#[test]
fn vla_parameter_bounds_are_evaluated_on_call() {
    let source = r#"
            int f(int n, int a[n]) {
                return 0;
            }

            int main(void) {
                int x[1] = {0};
                return f(0, x);
            }
        "#;
    assert_diagnostic_contains(
        source,
        "variable length array bound evaluated to a non-positive value",
    );
}

#[test]
fn static_array_parameter_requires_the_promised_number_of_elements() {
    let source = r#"
            int sum3(int a[static 3]) {
                return a[0] + a[1] + a[2];
            }

            int main(void) {
                int a[2] = {1, 2};
                return sum3(a);
            }
        "#;
    assert_diagnostic_contains(source, "does not provide enough elements");
}

#[test]
fn static_array_parameter_accepts_a_large_enough_array() {
    let source = r#"
            int sum3(int a[static 3]) {
                return a[0] + a[1] + a[2];
            }

            int main(void) {
                int a[3] = {1, 2, 3};
                return sum3(a) != 6;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn variably_modified_static_array_parameter_bound_is_evaluated_once() {
    let source = r#"
            int inspect(int n, int values[static ++n]) {
                return n;
            }

            int main(void) {
                int values[3] = {0};
                return inspect(1, values) != 2;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn fixed_array_parameter_bound_does_not_change_pointer_compatibility() {
    let source = r#"
            int sum3(int a[3]) {
                return a[0];
            }

            int main(void) {
                int values[2] = {1, 2};
                return sum3(values);
            }
        "#;
    assert_exit_status(source, 1);
}

#[test]
fn vla_array_parameter_bound_does_not_encode_argument_length() {
    let source = r#"
            int first(int n, int a[n]) {
                return a[0];
            }

            int main(void) {
                int values[2] = {1, 2};
                return first(3, values);
            }
        "#;
    assert_exit_status(source, 1);
}

#[test]
fn vla_objects_cannot_have_initializers() {
    let source = r#"
            int main(void) {
                int n = 2;
                int a[n] = {1, 2};
                return 0;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "variable length array objects cannot have an initializer",
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
fn casts_between_character_pointer_types_preserve_the_array_byte_domain() {
    let source = r#"
            int main(void) {
                char values[] = "abc";
                unsigned char *bytes = (unsigned char *)values;
                if (bytes[1] != 'b') return 1;
                bytes[1] = 'z';
                if (values[1] != 'z') return 2;
                if (((unsigned char *)"abc")[2] != 'c') return 3;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
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
fn block_scope_function_declarations_are_visible_inside_the_block() {
    let project = TestProject::new("block-scope-function-decl");
    project
        .write(
            "main.c",
            r#"
                #include <stdio.h>
                int main(void) {
                    {
                        int helper(void);
                        printf("%d\n", helper());
                    }
                    return 0;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                int helper(void) {
                    return 7;
                }

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "7\n");
}

#[test]
fn block_scope_function_declarations_do_not_escape_the_block() {
    let project = TestProject::new("block-scope-function-leak");
    project
        .write(
            "main.c",
            r#"
                int main(void) {
                    {
                        int helper(void);
                    }
                    return helper();
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                int helper(void) {
                    return 1;
                }

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]);
    let rendered = result.unwrap_err().render();
    assert!(rendered.contains("use of undeclared identifier helper"));
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
fn variadic_function_requires_a_fixed_parameter() {
    let err = run_source(
        "test.c",
        "int invalid(...);\nint main(void) { return 0; }\n",
    )
    .unwrap_err();
    assert!(
        err.render()
            .contains("requires at least one fixed parameter")
    );

    run_source(
        "test.c",
        "int valid(int, ...);\nint main(void) { return 0; }\n",
    )
    .unwrap();
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
fn va_start_rejects_invalid_last_named_parameter_types() {
    for source in [
        r#"
                #include <stdarg.h>
                int bad(float last, ...) {
                    va_list ap;
                    va_start(ap, last);
                    return 0;
                }
                int main(void) { return bad(1.0f, 2); }
            "#,
        r#"
                #include <stdarg.h>
                int bad(register int last, ...) {
                    va_list ap;
                    va_start(ap, last);
                    return 0;
                }
                int main(void) { return bad(1, 2); }
            "#,
        r#"
                #include <stdarg.h>
                int bad(int last[2], ...) {
                    va_list ap;
                    va_start(ap, last);
                    return 0;
                }
                int main(void) { int values[2]; return bad(values, 2); }
            "#,
        r#"
                #include <stdarg.h>
                int marker(void) { return 0; }
                int bad(int last(void), ...) {
                    va_list ap;
                    va_start(ap, last);
                    return 0;
                }
                int main(void) { return bad(marker, 2); }
            "#,
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains("va_start used with"));
    }
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
fn va_start_rejects_reinitializing_an_active_list() {
    let source = r#"
            #include <stdarg.h>
            int bad(int last, ...) {
                va_list ap;
                va_start(ap, last);
                va_start(ap, last);
                return 0;
            }
            int main(void) { return bad(1, 2); }
        "#;
    assert_diagnostic_contains(source, "reinitialize an active va_list");
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
fn variadic_array_arguments_decay_to_pointers() {
    let source = r#"
            #include <stdarg.h>
            int sum_array(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                int *values = va_arg(ap, int[3]);
                va_end(ap);
                values[0] = 10;
                return values[0] + values[1] + values[2];
            }
            int main(void) {
                int original[3] = {1, 2, 3};
                int result = sum_array(0, original);
                return result != 15 || original[0] != 1;
            }
        "#;
    let err = run_source("test.c", source).unwrap_err();
    assert!(
        err.render()
            .contains("next variadic argument has type int*"),
        "{}",
        err.render()
    );
}

#[test]
fn va_arg_rejects_incomplete_array_types() {
    let source = r#"
            #include <stdarg.h>
            int read_array(int marker, ...) {
                va_list ap;
                va_start(ap, marker);
                int values[1] = va_arg(ap, int[]);
                return values[0];
            }
            int main(void) {
                int values[1] = {1};
                return read_array(0, values);
            }
        "#;
    assert_diagnostic_contains(source, "complete object type");
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
fn defined_cannot_be_defined_or_undefined() {
    assert_diagnostic_contains(
        "#define defined 1\nint main(void) { return 0; }\n",
        "defined",
    );
    assert_diagnostic_contains("#undef defined\nint main(void) { return 0; }\n", "defined");
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
fn run_result_preserves_main_return_status() {
    let source = r#"
            int main(void) {
                return 7;
            }
        "#;
    assert_exit_status(source, 7);
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
fn cboxes_expression_eval_returns_uninitialized_lvalues() {
    let source = "int main(void) { int a; return 0; }\n";
    let baseline = crate_run_source_with_options("test.c", source, &RunOptions::default()).unwrap();
    let event_index = baseline
        .trace
        .iter()
        .position(|event| event.state.iter().any(|item| item.name == "a"))
        .expect("declaration trace event should exist");
    let result = crate_run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            expression_eval: Some(RunExpressionEvalRequest {
                expression: "a".to_owned(),
                event_index,
            }),
            ..RunOptions::default()
        },
    )
    .unwrap();
    let expression = result.expression.expect("expression result should exist");

    assert_eq!(expression.kind, "lvalue");
    assert_eq!(expression.ty, "int");
    assert_eq!(expression.value, "");
    assert_eq!(expression.name, "a");
    assert!(expression.address.is_some());
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
fn synthetic_address_base_shifts_addresses_without_changing_layout() {
    let source = r#"
            int main(void) {
                int a = 1;
                int b = 2;
                a = b;
                return 0;
            }
        "#;
    let addresses = |result: &RunResult| (state_address(result, "a"), state_address(result, "b"));
    let low = run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            synthetic_address_base: 0x2000,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let high = run_source_with_options(
        "test.c",
        source,
        &RunOptions {
            synthetic_address_base: 0x5000,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let (low_a, low_b) = addresses(&low);
    let (high_a, high_b) = addresses(&high);
    assert_eq!(high_a - low_a, 0x3000);
    assert_eq!(high_b - low_b, 0x3000);
    assert_eq!(low_b - low_a, 8);
    assert_eq!(high_b - high_a, 8);
}

#[test]
fn cboxes_expression_eval_returns_array_lvalues() {
    let source = r#"
            int main(void) {
                char a[3][4];
                a[1][0] = 0;
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
    let event_index = baseline.trace.len().saturating_sub(1);
    let result = crate_run_source_with_options(
        "test.c",
        normalized,
        &RunOptions {
            expression_eval: Some(RunExpressionEvalRequest {
                expression: "a[1]".to_owned(),
                event_index,
            }),
            ..RunOptions::default()
        },
    )
    .unwrap();
    let expression = result.expression.expect("expression result should exist");
    let address = expression
        .address
        .expect("array lvalue should have an address");
    assert_eq!(expression.kind, "lvalue");
    assert_eq!(expression.ty, "char[4]");
    assert_eq!(expression.name, "a[1]");
    assert_eq!(expression.value, address.to_string());
}

#[test]
fn gnu_attributes_are_ignored_in_common_declaration_positions() {
    let source = r#"
            #include <stdio.h>

            __attribute__((unused)) int helper(void) __attribute__((unused));

            int helper(void) __attribute__((unused)) {
                return 7;
            }

            int main(void) {
                int __attribute__((unused)) x = 3;
                int y __attribute__((unused)) = 4;
                int * __attribute__((unused)) p = &y;
                printf("%d %d %d\n", helper(), x, *p);
                return 0;
            }
        "#;
    assert_stdout(source, "7 3 4\n");
}

#[test]
fn gnu_attributes_do_not_prevent_block_declarations_from_parsing() {
    let source = r#"
            int main(void) {
                __attribute__((unused)) int x = 5;
                return x - 5;
            }
        "#;
    assert_stdout(source, "");
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
fn typedef_name_can_be_reused_as_object_and_member_name_in_the_same_declaration() {
    let source = r#"
            typedef int T;

            struct Holder {
                T T;
            };

            int main(void) {
                T T = 4;
                struct Holder h;
                h.T = T;
                return h.T - 4;
            }
        "#;
    assert_stdout(source, "");
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
fn free_ub_diagnostics_point_to_user_code() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                int x = 0;
                free(&x);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert_user_code_library_diag(
        &rendered,
        "free requires a pointer value returned by malloc or a null pointer",
    );
}

#[test]
fn memcpy_ub_diagnostics_point_to_user_code() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[4] = "abc";
                memcpy(&text[1], text, 2);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert_user_code_library_diag(&rendered, "memcpy source and destination regions overlap");
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
fn malloc_requires_stdlib_header() {
    let source = r#"
            int main(void) {
                return malloc(4) != 0;
            }

        "#;
    assert_diagnostic_contains(source, "use of undeclared identifier malloc");
}

#[test]
fn memcpy_requires_string_header() {
    let source = r#"
            int main(void) {
                int x = 1;
                int y = 0;
                memcpy(&y, &x, sizeof x);
                return y;
            }

        "#;
    assert_diagnostic_contains(source, "use of undeclared identifier memcpy");
}

#[test]
fn free_requires_stdlib_header() {
    let source = r#"
            int main(void) {
                free((void *)0);
                return 0;
            }

        "#;
    assert_diagnostic_contains(source, "use of undeclared identifier free");
}

#[test]
fn memcpy_copies_object_bytes() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>

            int main(void) {
                int x = 123;
                int y = 0;
                memcpy(&y, &x, sizeof x);
                printf("%d\n", y);
                return 0;
            }
        "#;
    assert_stdout(source, "123\n");
}

#[test]
fn memcpy_overlap_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                char text[4] = "abc";
                memcpy(&text[1], text, 2);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "overlap");
}

#[test]
fn memcpy_reports_object_bound_overrun_clearly() {
    let source = r#"
            #include <string.h>

            int main(void) {
                int a = 0;
                int *p = &a;
                int b = 0;
                int *q = &b;
                memcpy(p, q, 5);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("requested byte access of 5 byte(s)"));
    assert!(rendered.contains("4-byte object"));
}

#[test]
fn free_of_non_malloc_pointer_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                int x = 0;
                free(&x);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "pointer value returned by malloc");
}

#[test]
fn double_free_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                void *p = malloc(4);
                free(p);
                free(p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn dereference_after_free_is_ub() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                int *p = (int *)malloc(sizeof(int));
                *p = 7;
                free(p);
                printf("%d\n", *p);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn realloc_preserves_the_existing_prefix() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                int *p = (int *)malloc(sizeof(int));
                *p = 7;
                p = (int *)realloc(p, 2 * sizeof(int));
                p[1] = 9;
                printf("%d %d\n", p[0], p[1]);
                return 0;
            }
        "#;
    assert_stdout(source, "7 9\n");
}

#[test]
fn oversized_allocations_fail_with_null_instead_of_an_interpreter_error() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                void *from_malloc = malloc(64UL * 1024UL * 1024UL + 1UL);
                void *from_overflowed_calloc = calloc((unsigned long)-1, 2UL);
                return from_malloc != 0 || from_overflowed_calloc != 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn oversized_static_and_automatic_objects_are_rejected_before_host_allocation() {
    for source in [
        "int huge[1000000000]; int main(void) { return 0; }",
        "int main(void) { int huge[1000000000]; return 0; }",
        "int main(void) { int n = 1000000000; int huge[n]; return 0; }",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(err.render().contains("interpreter limit"));
    }
}

#[test]
fn allocation_limit_is_adjustable_and_can_be_disabled() {
    let limited = RunOptions {
        allocation_limit_bytes: Some(1024),
        ..RunOptions::default()
    };
    let static_source = "unsigned char bytes[1025]; int main(void) { return 0; }";
    assert!(
        run_source_with_options("test.c", static_source, &limited)
            .unwrap_err()
            .render()
            .contains("interpreter limit")
    );

    let unlimited = RunOptions {
        allocation_limit_bytes: None,
        ..RunOptions::default()
    };
    let result = run_source_with_options("test.c", static_source, &unlimited).unwrap();
    assert_eq!(result.exit_status, 0);

    let heap_source = r#"
            #include <stdlib.h>
            int main(void) {
                void *p = malloc(1025);
                if (p == 0) return 1;
                free(p);
                return 0;
            }
        "#;
    let result = run_source_with_options("test.c", heap_source, &limited).unwrap();
    assert_eq!(result.exit_status, 1);
    let result = run_source_with_options("test.c", heap_source, &unlimited).unwrap();
    assert_eq!(result.exit_status, 0);
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
fn large_arrays_use_compact_storage_and_bounded_visualization() {
    let source = r#"
            static unsigned char global[2 * 1024 * 1024];
            int main(void) {
                unsigned char local[2 * 1024 * 1024];
                global[0] = 1;
                global[sizeof global - 1] = 2;
                local[0] = 3;
                local[sizeof local - 1] = 4;
                return global[0] + global[sizeof global - 1]
                    + local[0] + local[sizeof local - 1] - 10;
            }
        "#;
    let result = run_source("test.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
    assert!(
        result
            .trace
            .iter()
            .all(|event| event.state.len() <= super::CBOXES_MAX_ARRAY_ELEMENTS + 2)
    );
}

#[test]
fn failed_realloc_preserves_the_original_allocation() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                int *original = malloc(sizeof(int));
                *original = 17;
                void *replacement = realloc(original, 64UL * 1024UL * 1024UL + 1UL);
                if (replacement != 0 || *original != 17) return 1;
                free(original);
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn old_pointer_after_realloc_is_ub() {
    let source = r#"
            #include <stdio.h>
            #include <stdlib.h>

            int main(void) {
                int *p = (int *)malloc(sizeof(int));
                int *old = p;
                p = (int *)realloc(p, 2 * sizeof(int));
                printf("%d\n", *old);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "lifetime has ended");
}

#[test]
fn realloc_of_non_heap_pointer_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                int x = 0;
                realloc(&x, sizeof x);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "malloc/calloc/realloc");
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
fn pointer_arithmetic_on_mismatched_non_character_pointer_is_ub() {
    let source = r#"
            int main(void) {
                int a = 0;
                (short *)&a + 1;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "pointer arithmetic requires a pointer to an array");
}

#[test]
fn multiple_tentative_definitions_in_one_file_coalesce() {
    let source = r#"
            #include <stdio.h>

            int x;
            int x;

            int main(void) {
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "0\n");
}

#[test]
fn tentative_definition_remains_defined_when_merged_with_extern_declarations() {
    let project = TestProject::new("tentative-plus-extern");
    project
        .write(
            "main.c",
            r#"
                extern int x;
                int x;

                int main(void) {
                    x = 7;
                    return x - 7;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                extern int x;
            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "");
}

#[test]
fn tentative_definition_with_incomplete_array_becomes_one_element_for_storage() {
    let source = r#"
            int a[];
            int main(void) {
                return a[0];
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn sizeof_rejects_a_tentative_array_that_is_incomplete_at_the_use() {
    for source in [
        "int a[]; int main(void) { return sizeof a; }\n",
        "extern int a[]; int main(void) { return sizeof a; } int a[3];\n",
    ] {
        let err = run_source("test.c", source).unwrap_err();
        assert!(
            err.render()
                .contains("sizeof cannot determine the size of type int[]"),
            "{}",
            err.render()
        );
    }
}

#[test]
fn internal_tentative_definition_must_have_complete_type() {
    let err = run_source("test.c", "static int a[]; int main(void) { return 0; }\n").unwrap_err();
    assert!(
        err.render().contains("must have complete type"),
        "{}",
        err.render()
    );
}

#[test]
fn later_declaration_can_complete_an_internal_tentative_array() {
    let source = r#"
            static int a[];
            extern int a[2];
            int main(void) { return (int)(sizeof a / sizeof a[0]) - 2; }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn incomplete_and_complete_tentative_array_declarations_merge() {
    let source = r#"
            #include <stdio.h>

            int a[];
            int a[3];

            int main(void) {
                printf("%lu %d\n", sizeof a / sizeof a[0], a[2]);
                return 0;
            }
        "#;
    assert_stdout(source, "3 0\n");
}

#[test]
fn real_definition_and_tentative_definition_in_one_file_do_not_conflict() {
    let source = r#"
            #include <stdio.h>

            int x;
            int x = 4;

            int main(void) {
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "4\n");
}

#[test]
fn extern_after_static_in_same_file_keeps_internal_linkage() {
    let source = r#"
            #include <stdio.h>

            static int x = 3;
            extern int x;

            int main(void) {
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "3\n");
}

#[test]
fn function_without_storage_class_inherits_visible_internal_linkage() {
    let source = r#"
            static int helper(void);
            int helper(void) { return 7; }
            int main(void) { return helper() - 7; }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn static_after_extern_in_same_file_is_reported_as_linkage_ub() {
    let source = r#"
            extern int x;
            static int x;

            int main(void) {
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "internal and external linkage");
}

#[test]
fn static_function_after_external_declaration_is_reported_as_linkage_ub() {
    let source = "int helper(void); static int helper(void); int main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "internal and external linkage");
}

#[test]
fn plain_file_scope_object_after_static_is_reported_as_linkage_ub() {
    let source = "static int x; int x; int main(void) { return 0; }\n";
    assert_diagnostic_contains(source, "internal and external linkage");
}

#[test]
fn hidden_no_linkage_object_prevents_extern_from_inheriting_internal_linkage() {
    let source = r#"
            static int x;
            int main(void) {
                int x;
                {
                    extern int x;
                }
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "internal and external linkage");
}

#[test]
fn incompatible_function_declarations_across_files_are_rejected() {
    let project = TestProject::new("conflicting-function-decls");
    project
        .write(
            "main.c",
            r#"
                int f(int);

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
                double f(int);

            "#,
        )
        .unwrap();
    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("conflicting declarations of function f"));
}

#[test]
fn top_level_parameter_qualifiers_do_not_make_function_declarations_conflict() {
    let project = TestProject::new("function-param-top-level-qualifiers");
    project
        .write(
            "main.c",
            r#"
                int same(const char *text);

                int main(void) {
                    return same("ok") - 2;
                }
            "#,
        )
        .unwrap();
    project
        .write(
            "helper.c",
            r#"
                int same(const char * const text) {
                    return text[0] == 'o' && text[1] == 'k';
                }
            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "");
}

#[test]
fn multiple_tentative_definitions_across_files_conflict() {
    let project = TestProject::new("multiple-tentative-globals");
    project
        .write(
            "main.c",
            r#"
                int x;

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
                int x;

            "#,
        )
        .unwrap();
    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("multiple definitions of object x"));
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
fn declared_but_undefined_external_object_use_is_ub() {
    let source = r#"
            extern int x;

            int main(void) {
                return x;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn declared_but_undefined_function_call_is_ub() {
    let source = r#"
            int f(void);

            int main(void) {
                f();
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn taking_address_of_undefined_function_is_ub() {
    let source = r#"
            int f(void);

            int main(void) {
                int (*p)(void) = f;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn taking_address_of_undefined_object_is_ub() {
    let source = r#"
            extern int x;

            int main(void) {
                int *p = &x;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn static_inline_declaration_without_definition_is_ok_until_used() {
    let source = r#"
            static inline int f(void);

            int main(void) {
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn static_inline_declaration_without_definition_is_ub_when_used() {
    let source = r#"
            static inline int f(void);

            int main(void) {
                return f();
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn taking_address_of_inline_definition_without_external_definition_is_ub() {
    let source = r#"
            inline int f(void) {
                return 1;
            }

            int main(void) {
                int (*p)(void) = f;
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "does not provide a definition");
}

#[test]
fn inline_definition_can_coexist_with_external_definition_in_another_file() {
    let project = TestProject::new("inline-with-external-def");
    project
        .write(
            "main.c",
            r#"
                #include <stdio.h>

                inline int f(void) {
                    return 1;
                }

                int main(void) {
                    printf("%d\n", f());
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
                    return 2;
                }

            "#,
        )
        .unwrap();
    let result = project.run(["main.c", "helper.c"]).unwrap();
    assert_eq!(result.stdout, "2\n");
}

#[test]
fn extern_inline_definition_conflicts_with_another_external_definition() {
    let project = TestProject::new("extern-inline-conflict");
    project
        .write(
            "main.c",
            r#"
                extern inline int f(void) {
                    return 1;
                }

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
                    return 2;
                }
            "#,
        )
        .unwrap();
    let err = project.run(["main.c", "helper.c"]).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("multiple definitions of function f"));
}

#[test]
fn pointer_copy_after_free_is_ub() {
    let source = r#"
            #include <stdlib.h>

            int main(void) {
                int *p = (int *)malloc(sizeof(int));
                int *q = p;
                free(p);
                int *r = q;
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("lifetime has ended") || rendered.contains("indeterminate pointer"));
}

#[test]
fn memcpy_with_pointer_copied_after_free_is_ub() {
    let source = r#"
            #include <stdlib.h>
            #include <string.h>

            int main(void) {
                int *p = (int *)malloc(sizeof(int));
                int *q = p;
                int dst = 0;
                free(p);
                memcpy(&dst, q, sizeof(int));
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("lifetime has ended") || rendered.contains("indeterminate pointer"));
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
fn math_requires_math_header() {
    let source = r#"
            int main(void) {
                return sin(0.0) != 0.0;
            }
        "#;
    assert_diagnostic_contains(source, "undeclared identifier sin");
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
fn math_output_pointer_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <math.h>

            int main(void) {
                int e = 0;
                double i = 0.0;
                int q = 0;
                printf("%d %d %d %d\n",
                    frexp(8.0, &e) == 0.5 && e == 4,
                    modf(3.5, &i) == 0.5 && i == 3.0,
                    remquo(5.5, 2.0, &q) == -0.5 && q == 3,
                    nexttoward(1.0, 2.0L) > 1.0);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1 1 1\n");
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
fn modf_with_null_result_pointer_is_ub() {
    let source = r#"
            #include <math.h>

            int main(void) {
                modf(1.0, 0);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "writable result pointer");
}

#[test]
fn modf_cannot_write_const_object() {
    let source = r#"
            #include <math.h>

            int main(void) {
                const double whole = 0.0;
                modf(1.0, (double *)&whole);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "const-qualified");
}

#[test]
fn nan_rejects_indeterminate_tag_bytes() {
    let source = r#"
            #include <math.h>

            int main(void) {
                char tag[4];
                &tag;
                nan(tag);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "indeterminate byte");
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
fn signal_header_functions_work() {
    let source = r#"
            #include <stdio.h>
            #include <signal.h>

            int seen = 0;

            void handler(int sig) {
                seen = sig;
            }

            int main(void) {
                void (*prev)(int) = signal(SIGINT, handler);
                raise(SIGINT);
                printf("%d %d\n", prev == SIG_DFL, seen == SIGINT);
                return 0;
            }
        "#;
    assert_stdout(source, "1 1\n");
}

#[test]
fn signal_rejects_non_standard_signal_number() {
    let source = r#"
            #include <signal.h>

            void handler(int sig) {
                (void)sig;
            }

            int main(void) {
                signal(999, handler);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "required ISO C signal macros");
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
fn floating_environment_tracks_exceptions() {
    assert_exit_status(
        include_str!("../../tests/standard_examples/floating_environment_tracks_exceptions.c"),
        0,
    );
}

#[test]
fn floating_environment_is_reset_between_executions() {
    assert_exit_status(
        r#"
            #include <fenv.h>
            int main(void) {
                return fesetround(FE_UPWARD) != 0;
            }
        "#,
        0,
    );
    assert_exit_status(
        r#"
            #include <fenv.h>
            int main(void) {
                return fegetround() != FE_TONEAREST ||
                       fetestexcept(FE_ALL_EXCEPT) != 0;
            }
        "#,
        0,
    );
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
fn complex_library_rejects_indeterminate_argument_even_after_address_taken() {
    let source = r#"
            #include <complex.h>

            int main(void) {
                double _Complex z;
                &z;
                cabs(z);
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("indeterminate"));
    assert!(rendered.contains("library functions exhibit undefined behavior"));
}

#[test]
fn setjmp_round_trip_and_longjmp_zero_returns_one() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            struct JumpContext {
                jmp_buf env;
                volatile int status;
            };

            int main(void) {
                struct JumpContext context;
                if (setjmp(context.env) == 0) {
                    context.status = 7;
                    longjmp(context.env, 0);
                }
                printf("ok %d\n", context.status);
                return 0;
            }
        "#;
    assert_stdout(source, "ok 7\n");
}

#[test]
fn setjmp_expression_statement_is_allowed() {
    let source = r#"
            #include <setjmp.h>

            int main(void) {
                jmp_buf env;
                setjmp(env);
                return 0;
            }
        "#;
    run_source("test.c", source).unwrap();
}

#[test]
fn setjmp_invalid_initializer_context_is_ub() {
    let source = r#"
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int value = setjmp(env);
                return value;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("undefined behavior"));
    assert!(rendered.contains("entire expression statement or controlling expression"));
}

#[test]
fn setjmp_invalid_comma_condition_context_is_ub() {
    let source = r#"
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                if ((setjmp(env), 1)) {
                    return 0;
                }
                return 0;
            }
        "#;
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains("undefined behavior"));
    assert!(rendered.contains("entire expression statement or controlling expression"));
}

#[test]
fn longjmp_to_returned_function_is_ub() {
    let source = r#"
            #include <setjmp.h>

            jmp_buf env;

            void save(void) {
                if (setjmp(env) == 0) {
                    return;
                }
            }

            int main(void) {
                save();
                longjmp(env, 1);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "already returned");
}

#[test]
fn longjmp_after_leaving_vla_scope_is_ub() {
    let source = r#"
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int jumped = 0;
                {
                    int n = 2;
                    int a[n];
                    if (setjmp(env) == 0) {
                        jumped = a[0] = 1;
                    }
                }
                if (jumped) {
                    longjmp(env, 1);
                }
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "variably modified object");
}

#[test]
fn changed_nonvolatile_local_becomes_indeterminate_after_longjmp() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int x = 1;
                if (setjmp(env) == 0) {
                    x = 2;
                    longjmp(env, 1);
                }
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_diagnostic_contains(source, "indeterminate");
}

#[test]
fn changed_volatile_local_survives_longjmp() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                volatile int x = 1;
                if (setjmp(env) == 0) {
                    x = 2;
                    longjmp(env, 1);
                }
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "2\n");
}

#[test]
fn unchanged_nonvolatile_local_survives_longjmp() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int x = 1;
                if (setjmp(env) == 0) {
                    longjmp(env, 1);
                }
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn longjmp_resumes_while_condition_without_restarting_loop() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                volatile int count = 0;
                while (setjmp(env) == 0) {
                    count++;
                    longjmp(env, 1);
                }
                printf("%d\n", count);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
}

#[test]
fn longjmp_resumes_for_condition_without_reexecuting_init() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int i = 99;
                for (i = 0; setjmp(env) == 0; i++) {
                    longjmp(env, 1);
                }
                printf("%d\n", i);
                return 0;
            }
        "#;
    assert_stdout(source, "0\n");
}

#[test]
fn longjmp_resumes_do_while_condition_without_reexecuting_body_first() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                volatile int body = 0;
                do {
                    body++;
                    if (body == 2) {
                        longjmp(env, 1);
                    }
                } while (setjmp(env) == 0);
                printf("%d\n", body);
                return 0;
            }
        "#;
    assert_stdout(source, "2\n");
}

#[test]
fn longjmp_resumes_switch_expression() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                switch (setjmp(env)) {
                    case 0:
                        longjmp(env, 2);
                        break;
                    case 2:
                        printf("ok\n");
                        break;
                }
                return 0;
            }
        "#;
    assert_stdout(source, "ok\n");
}

#[test]
fn block_scope_shadowing_is_restored_after_longjmp() {
    let source = r#"
            #include <stdio.h>
            #include <setjmp.h>

            jmp_buf env;

            int main(void) {
                int x = 1;
                {
                    int x = 2;
                    if (setjmp(env) == 0) {
                        longjmp(env, 1);
                    }
                }
                printf("%d\n", x);
                return 0;
            }
        "#;
    assert_stdout(source, "1\n");
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
fn memcpy_to_volatile_object_through_cast_is_ub() {
    let source = r#"
            #include <string.h>

            int main(void) {
                volatile int x = 0;
                int y = 1;
                memcpy((void *)&x, &y, sizeof x);
                return 0;
            }
        "#;
    assert_diagnostic_contains(
        source,
        "volatile-qualified type through a non-volatile pointer",
    );
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
fn bounded_wide_memory_functions_accept_nonterminated_arrays() {
    let source = r#"
            #include <wchar.h>
            int main(void) {
                wchar_t left[2] = {1, 2};
                wchar_t right[2] = {1, 2};
                if (wmemcmp(left, right, 2) != 0) return 1;
                if (wcsncmp(left, right, 2) != 0) return 2;
                if (wmemchr(left, 2, 2) != &left[1]) return 3;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
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
fn jumping_out_of_a_vla_scope_is_allowed() {
    let source = r#"
            int main(void) {
                int result = 1;
                {
                    int n = 2;
                    int values[n];
                    values[0] = 0;
                    goto done;
                }
            done:
                return result - 1;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn implicit_pointer_to_integer_conversion_is_rejected() {
    let source = "int main(void) { int value; int converted = &value; return converted; }";
    assert_diagnostic_contains(source, "cannot convert int* to int");

    let allowed = r#"
            int main(void) {
                int value;
                unsigned long address = (unsigned long)&value;
                _Bool truth = &value;
                return address == 0 || !truth;
            }
        "#;
    assert_eq!(run_source("test.c", allowed).unwrap().exit_status, 0);
}

#[test]
fn character_constant_to_pointer_diagnostic_preserves_the_expected_type() {
    for (source, expected_type) in [
        ("int main(void) { char *p = 'a'; }\n", "char*"),
        ("int main(void) { int *p = 'a'; }\n", "int*"),
    ] {
        let rendered = run_source("test.c", source).unwrap_err().render();
        assert!(
            rendered.contains(&format!(
                "cannot convert a character constant to pointer type {expected_type}"
            )),
            "{rendered}"
        );
    }

    run_source(
        "test.c",
        "int main(void) { char *p = '\\0'; return p != 0; }\n",
    )
    .unwrap();

    let rendered = run_source("test.c", "int main(void) {\n  char *p;\n  p = 'a';\n}\n")
        .unwrap_err()
        .render();
    assert!(rendered.contains("test.c:3:7"), "{rendered}");
}

#[test]
fn explicit_integer_pointer_conversions_round_trip_addresses() {
    let source = r#"
            #include <stdint.h>

            int function(void) { return 1; }

            int main(void) {
                int scalar;
                int array[3];
                struct Pair { int first; int second; } pair;

                uintptr_t scalar_address = (uintptr_t)&scalar;
                uintptr_t element_address = (uintptr_t)&array[1];
                uintptr_t member_address = (uintptr_t)&pair.second;
                uintptr_t one_past_address = (uintptr_t)(array + 3);
                uintptr_t function_address = (uintptr_t)function;
                void *void_element = array + 1;

                return (int *)scalar_address != &scalar
                    || (int *)element_address != &array[1]
                    || (int *)member_address != &pair.second
                    || (int *)one_past_address != array + 3
                    || (int (*)(void))function_address != function
                    || (int *)void_element != &array[1];
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let opaque = r#"
            #include <stdint.h>
            int main(void) {
                int *p = (int *)1234;
                void *opaque = (void *)5;
                const char *bytes = (const char *)opaque;
                return p == 0 || (uintptr_t)p != 1234 || p != (int *)1234
                    || (uintptr_t)bytes != 5;
            }
        "#;
    assert_eq!(run_source("test.c", opaque).unwrap().exit_status, 0);

    let opaque_function = r#"
            typedef void (*Callback)(void);
            int main(void) {
                Callback sentinel = (Callback)-1;
                return sentinel == 0 || sentinel != (Callback)-1;
            }
        "#;
    assert_eq!(
        run_source("test.c", opaque_function).unwrap().exit_status,
        0
    );

    let trap = "int main(void) { return *(int *)1234; }";
    let err = run_source("test.c", trap).unwrap_err();
    let rendered = err.render();
    assert!(rendered.contains("undefined behavior"));
    assert!(rendered.contains("dereference"));

    let function_trap = r#"
            typedef void (*Callback)(void);
            int main(void) { ((Callback)-1)(); }
        "#;
    let rendered = run_source("test.c", function_trap).unwrap_err().render();
    assert!(rendered.contains("opaque implementation-defined function pointer"));
}

#[test]
fn sizeof_pointer_difference_and_reversed_subscript_work() {
    let source = r#"
            int main(void) {
                int values[3] = {4, 5, 6};
                return sizeof(&values[2] - &values[0]) != sizeof(long)
                    || 1[values] != 5
                    || *(&1[values]) != 5;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn sizeof_dereferenced_array_uses_the_inner_array_to_pointer_conversion() {
    let source = r#"
            int main(void) {
                int values[5];
                return sizeof *values != sizeof(int)
                    || sizeof *(values + 1) != sizeof(int);
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn comma_operator_converts_array_and_function_designators() {
    let source = r#"
            int function(void) { return 0; }

            int main(void) {
                int values[5];
                return sizeof(0, values) != sizeof(int *)
                    || sizeof(0, function) != sizeof(int (*)(void));
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
}

#[test]
fn exact_length_string_array_initializers_may_omit_the_terminator() {
    let source = r#"
            #include <wchar.h>
            int main(void) {
                char narrow[3] = "abc";
                wchar_t wide[2] = L"xy";
                return narrow[0] != 'a' || narrow[2] != 'c'
                    || wide[0] != L'x' || wide[1] != L'y';
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let too_short = "int main(void) { char text[2] = \"abc\"; return 0; }";
    assert!(run_source("test.c", too_short).is_err());
}

#[test]
fn calls_without_prototypes_use_default_argument_promotions() {
    let source = r#"
            int twice();
            int main(void) { return twice(4) - 8; }
            int twice(int value) { return value * 2; }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let void_source = "int f(); int f(void) { return 3; } int main(void) { return f() - 3; }";
    assert_eq!(run_source("test.c", void_source).unwrap().exit_status, 0);
}

#[test]
fn calls_without_prototypes_detect_definition_mismatches() {
    let bad_declaration =
        "int f(); int f(float value) { return value; } int main(void) { return f(1.0f); }";
    assert!(run_source("test.c", bad_declaration).is_err());

    let bad_call = "int f(); int main(void) { return f(1.0); } int f(int value) { return value; }";
    let err = run_source("test.c", bad_call).unwrap_err();
    assert!(
        err.render()
            .contains("incompatible with parameter type int in the definition of f"),
        "{}",
        err.render()
    );

    let extra_argument = "int f() { return 0; } int main(void) { return f(1); }";
    let err = run_source("test.c", extra_argument).unwrap_err();
    assert!(
        err.render().contains("call without a prototype"),
        "{}",
        err.render()
    );
}

#[test]
fn function_arguments_require_complete_object_types() {
    let source = "void accept(int fixed, ...) {} int main(void) { accept(7, (void)0); }";
    assert_diagnostic_contains(source, "function arguments must have complete object type");
}

#[test]
fn old_style_calls_allow_character_and_void_pointer_types() {
    let source = r#"
            int inspect();
            int inspect(p)
                char *p;
            {
                return p[0] == 'x';
            }
            int main(void) {
                char text[] = "x";
                void *value = text;
                return inspect(value) != 1;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let mismatch = r#"
            int inspect();
            int inspect(p)
                int *p;
            {
                return *p;
            }
            int main(void) {
                char value = 0;
                return inspect(&value);
            }
        "#;
    assert_diagnostic_contains(mismatch, "incompatible with parameter type int*");
}

#[test]
fn noreturn_is_restricted_to_function_identifiers_other_than_main() {
    for source in [
        "typedef _Noreturn void Callback(void); int main(void) { return 0; }",
        "int main(void) { typedef _Noreturn void Callback(void); return 0; }",
    ] {
        assert_diagnostic_contains(source, "_Noreturn is not valid in a typedef declaration");
    }
    assert_diagnostic_contains(
        "_Noreturn int main(void) { for (;;) {} }",
        "function specifiers are not valid in a declaration of main",
    );
}

#[test]
fn declarations_must_declare_something() {
    for source in [
        "int; int main(void) { return 0; }",
        "int main(void) { int; return 0; }",
        "struct { int value; }; int main(void) { return 0; }",
        "union { int value; }; int main(void) { return 0; }",
    ] {
        assert_diagnostic_contains(source, "declaration does not declare");
    }

    let valid = r#"
            struct Forward;
            struct Defined { int value; };
            enum Choice { FIRST };
            enum Choice;
            int main(void) {
                struct Local;
                enum LocalChoice { LOCAL };
                return FIRST + LOCAL;
            }
        "#;
    assert_eq!(run_source("test.c", valid).unwrap().exit_status, 0);

    for source in [
        "enum Incomplete; int main(void) { return 0; }",
        "enum Incomplete *value; int main(void) { return 0; }",
    ] {
        assert_diagnostic_contains(source, "must refer to a complete enum type");
    }
}

#[test]
fn c11_alignment_static_assert_noreturn_and_func_features_work() {
    let source = r#"
#include <assert.h>
#include <stdalign.h>
#include <stddef.h>
_Static_assert(_Alignof(long double) == _Alignof(max_align_t), "max alignment");
static_assert(__STDC__ == 1 && __STDC_VERSION__ == 201112L, "C11 macros");
struct checked { _Static_assert(sizeof(int) == 4, "record assertion"); int value; };
alignas(64) int aligned_value;
int main(void) {
    static_assert(_Alignof(int) == 4, "int alignment");
    return ((unsigned long)&aligned_value % 64) || __func__[0] != 'm';
}
"#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);

    let noreturn = "_Noreturn void f(void) { return; } int main(void) { f(); }";
    assert_diagnostic_contains(noreturn, "_Noreturn function f returned");
}

#[test]
fn alignment_specifiers_are_intermixed_and_apply_to_record_members() {
    assert_exit_status(
        r#"
            struct S { _Alignas(16) char x; char y; };
            _Static_assert(_Alignof(struct S) == 16, "member alignment");
            _Static_assert(sizeof(struct S) == 16, "record size");
            int main(void) {
                const _Alignas(4) int value = 1;
                return value != 1;
            }
        "#,
        0,
    );
}

#[test]
fn prohibited_zero_alignment_specifiers_are_still_diagnosed() {
    assert_diagnostic_contains(
        "typedef _Alignas(0) int T; int main(void) { return 0; }",
        "cannot appear in a typedef",
    );
    assert_diagnostic_contains(
        "int f(_Alignas(0) int x); int main(void) { return 0; }",
        "not valid in this declaration context",
    );
}

#[test]
fn parameter_list_enumerators_are_visible_in_the_function_definition_body() {
    assert_exit_status(
        r#"
            int definition(enum Defined { BODY_VALUE = 17 } value) {
                return value != BODY_VALUE;
            }
            int main(void) { return definition(17); }
        "#,
        0,
    );
}

#[test]
fn enum_types_are_compatible_with_the_implemented_underlying_int_type() {
    assert_diagnostic_contains(
        r#"
            enum E { E0, E1 };
            int main(void) {
                enum E value = E1;
                return _Generic(value, enum E: 0, int: 1, default: 2);
            }
        "#,
        "compatible types more than once",
    );
}

#[test]
fn repeated_prototype_scope_tags_denote_distinct_types() {
    assert_diagnostic_contains(
        "int f(struct S *p); int f(struct S *p); int main(void) { return 0; }",
        "conflicting declarations of function f",
    );
}

#[test]
fn old_style_parameters_accept_function_declarators_and_adjust_to_pointers() {
    assert_exit_status(
        r#"
            int inc(int x) { return x + 1; }
            int apply(fn) int fn(); { return fn(4); }
            int call(a) int a(int); { return a(2); }
            int main(void) { return apply(inc) != 5 || call(inc) != 3; }
        "#,
        0,
    );
}

#[test]
fn file_scope_enumerator_names_are_independent_between_translation_units() {
    let project = TestProject::new("cross-tu-enumerators");
    project
        .write("a.c", "enum { X = 1 }; int f(void) { return X; }\n")
        .unwrap();
    project
        .write(
            "b.c",
            "enum { X = 2 }; int f(void); int main(void) { return f() != 1; }\n",
        )
        .unwrap();
    assert_eq!(project.run(["a.c", "b.c"]).unwrap().exit_status, 0);
}

#[test]
fn external_declarations_can_mix_function_and_object_declarators() {
    assert_exit_status(
        r#"
            int f(void), x;
            int y, g(void);
            int f(void) { return 3; }
            int g(void) { return 4; }
            int main(void) {
                x = 7;
                y = 8;
                return f() != 3 || g() != 4 || x != 7 || y != 8;
            }
        "#,
        0,
    );
}

#[test]
fn inline_applies_to_each_function_in_a_declarator_list() {
    assert_exit_status(
        r#"
            inline int f(void), g(void);
            int f(void) { return 1; }
            int g(void) { return 2; }
            int main(void) { return f() != 1 || g() != 2; }
        "#,
        0,
    );
}

#[test]
fn external_inline_definitions_reject_all_internal_linkage_references() {
    assert_diagnostic_contains(
        r#"
            static int helper = 7;
            inline int f(void) { extern int helper; return helper; }
            int main(void) { return 0; }
        "#,
        "internal-linkage identifier helper",
    );
    assert_diagnostic_contains(
        r#"
            static int bound = 7;
            inline int f(int values[bound]) { return 0; }
            int main(void) { return 0; }
        "#,
        "internal-linkage identifier bound",
    );
}

#[test]
fn tags_declared_in_selection_substatements_do_not_leak() {
    assert_diagnostic_contains(
        r#"
            int main(void) {
                if (1) (struct S { int x; }){1};
                struct S x = {7};
                return x.x;
            }
        "#,
        "incomplete type",
    );
}

#[test]
fn switch_entry_into_for_body_keeps_the_for_declaration_in_scope() {
    assert_exit_status(
        r#"
            int main(void) {
                switch (1) {
                    for (int i = 0; i < 1; ++i) {
                        case 1:
                            i = 9;
                            return i;
                    }
                }
                return 0;
            }
        "#,
        9,
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
fn aligned_alloc_rejects_invalid_c11_preconditions() {
    for expression in ["aligned_alloc(3, 6)", "aligned_alloc(8, 12)"] {
        let source =
            format!("#include <stdlib.h>\nint main(void) {{ (void){expression}; return 0; }}\n");
        let rendered = run_source("test.c", &source).unwrap_err().render();
        assert_user_code_library_diag(&rendered, "aligned_alloc");
    }
}

#[test]
fn zero_count_library_arrays_still_require_valid_pointers() {
    let cases = [
        ("#include <string.h>", "memcpy((void *)0, (void *)0, 0)"),
        ("#include <string.h>", "memmove((void *)0, (void *)0, 0)"),
        ("#include <string.h>", "memset((void *)0, 0, 0)"),
        ("#include <string.h>", "memchr((void *)0, 0, 0)"),
        ("#include <string.h>", "memcmp((void *)0, (void *)0, 0)"),
        ("#include <wchar.h>", "wmemset((wchar_t *)0, 0, 0)"),
        ("#include <stdio.h>", "fread((void *)0, 0, 1, stdin)"),
        ("#include <stdio.h>", "fwrite((void *)0, 0, 1, stdout)"),
        ("#include <stdlib.h>", "mbstowcs((wchar_t *)0, \"\", 0)"),
    ];
    for (include, expression) in cases {
        let source = format!("{include}\nint main(void) {{ {expression}; return 0; }}\n");
        let rendered = run_source("test.c", &source).unwrap_err().render();
        assert_user_code_library_diag(&rendered, "null pointer");
    }

    let source = r#"
            #include <string.h>
            #include <wchar.h>
            int main(void) {
                char bytes[1] = {0};
                mbstate_t state = {0};
                if (memmove(bytes + 1, bytes + 1, 0) != bytes + 1) return 1;
                if (mbrlen(bytes + 1, 0, &state) != (size_t)-2) return 2;
                return 0;
            }
        "#;
    assert_eq!(run_source("test.c", source).unwrap().exit_status, 0);
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
fn formatted_and_temporary_output_arrays_are_prechecked() {
    let cases = [
        (
            "#include <wchar.h>",
            "swprintf((wchar_t *)0, 0, L\"\")",
            "null pointer",
        ),
        ("#include <stdio.h>", "tmpnam((char[1]){0})", "exceeds"),
    ];
    for (include, expression, expected) in cases {
        let source = format!("{include}\nint main(void) {{ {expression}; return 0; }}\n");
        let rendered = run_source("test.c", &source).unwrap_err().render();
        assert_user_code_library_diag(&rendered, expected);
    }
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

fn sqlite_varint_source() -> &'static str {
    // Extracted from SQLite src/util.c. SQLITE_NOINLINE is only an optimizer
    // annotation in SQLite, so it is intentionally omitted here.
    r#"
            #include <assert.h>
            #include <stdint.h>

            typedef unsigned char u8;
            typedef uint32_t u32;
            typedef uint64_t u64;

            static int putVarint64(unsigned char *p, u64 v){
              int i, j, n;
              u8 buf[10];
              if( v & (((u64)0xff000000)<<32) ){
                p[8] = (u8)v;
                v >>= 8;
                for(i=7; i>=0; i--){
                  p[i] = (u8)((v & 0x7f) | 0x80);
                  v >>= 7;
                }
                return 9;
              }
              n = 0;
              do{
                buf[n++] = (u8)((v & 0x7f) | 0x80);
                v >>= 7;
              }while( v!=0 );
              buf[0] &= 0x7f;
              assert( n<=9 );
              for(i=0, j=n-1; j>=0; j--, i++){
                p[i] = buf[j];
              }
              return n;
            }

            int sqlite3PutVarint(unsigned char *p, u64 v){
              if( v<=0x7f ){
                p[0] = v&0x7f;
                return 1;
              }
              if( v<=0x3fff ){
                p[0] = ((v>>7)&0x7f)|0x80;
                p[1] = v&0x7f;
                return 2;
              }
              return putVarint64(p,v);
            }

            #define SLOT_2_0     0x001fc07f
            #define SLOT_4_2_0   0xf01fc07f

            u8 sqlite3GetVarint(const unsigned char *p, u64 *v){
              u32 a,b,s;

              if( ((signed char*)p)[0]>=0 ){
                *v = *p;
                return 1;
              }
              if( ((signed char*)p)[1]>=0 ){
                *v = ((u32)(p[0]&0x7f)<<7) | p[1];
                return 2;
              }
              assert( SLOT_2_0 == ((0x7f<<14) | (0x7f)) );
              assert( SLOT_4_2_0 == ((0xfU<<28) | (0x7f<<14) | (0x7f)) );

              a = ((u32)p[0])<<14;
              b = p[1];
              p += 2;
              a |= *p;
              if (!(a&0x80)){
                a &= SLOT_2_0;
                b &= 0x7f;
                b = b<<7;
                a |= b;
                *v = a;
                return 3;
              }

              a &= SLOT_2_0;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                b &= SLOT_2_0;
                a = a<<7;
                a |= b;
                *v = a;
                return 4;
              }

              b &= SLOT_2_0;
              s = a;
              p++;
              a = a<<14;
              a |= *p;
              if (!(a&0x80)){
                b = b<<7;
                a |= b;
                s = s>>18;
                *v = ((u64)s)<<32 | a;
                return 5;
              }

              s = s<<7;
              s |= b;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                a &= SLOT_2_0;
                a = a<<7;
                a |= b;
                s = s>>18;
                *v = ((u64)s)<<32 | a;
                return 6;
              }

              p++;
              a = a<<14;
              a |= *p;
              if (!(a&0x80)){
                a &= SLOT_4_2_0;
                b &= SLOT_2_0;
                b = b<<7;
                a |= b;
                s = s>>11;
                *v = ((u64)s)<<32 | a;
                return 7;
              }

              a &= SLOT_2_0;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                b &= SLOT_4_2_0;
                a = a<<7;
                a |= b;
                s = s>>4;
                *v = ((u64)s)<<32 | a;
                return 8;
              }

              p++;
              a = a<<15;
              a |= *p;
              b &= SLOT_2_0;
              b = b<<8;
              a |= b;
              s = s<<4;
              b = p[-4];
              b &= 0x7f;
              b = b>>3;
              s |= b;
              *v = ((u64)s)<<32 | a;
              return 9;
            }

            static int check(u64 input){
              unsigned char encoded[9] = {0};
              u64 decoded = 0;
              int written = sqlite3PutVarint(encoded, input);
              int read = sqlite3GetVarint(encoded, &decoded);
              return written==read && decoded==input;
            }

            int main(void){
              const u64 boundaries[] = {
                0, 1, 0x7f, 0x80, 0x3fff, 0x4000,
                0x1fffff, 0x200000, 0xfffffff, 0x10000000,
                0x7ffffffffULL, 0x800000000ULL,
                0x3ffffffffffULL, 0x40000000000ULL,
                0x1ffffffffffffULL, 0x2000000000000ULL,
                0xffffffffffffffULL, 0x100000000000000ULL,
                0x7fffffffffffffffULL, 0x8000000000000000ULL,
                0xffffffffffffffffULL
              };
              unsigned long i;
              for(i=0; i<sizeof(boundaries)/sizeof(boundaries[0]); i++){
                if( !check(boundaries[i]) ) return 1;
              }
              u64 value = 0x243f6a8885a308d3ULL;
              for(i=0; i<512; i++){
                value = value * 6364136223846793005ULL + 1442695040888963407ULL;
                if( !check(value) ) return 2;
              }
              return 0;
            }
        "#
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
fn reported_signal_sentinels_are_not_declarable_function_addresses() {
    let source = r#"
            #include <signal.h>
            #include <stdio.h>

            int main(void) {
                void (*previous)(int) = signal(SIGINT, SIG_IGN);
                void (*ignored)(int) = signal(SIGINT, SIG_DFL);
                printf("%d %d %d %d %d\n",
                    SIG_DFL == __codex_sig_dfl,
                    SIG_IGN == __codex_sig_ign,
                    SIG_ERR == __codex_sig_err,
                    previous == SIG_DFL,
                    ignored == SIG_IGN);
                return 0;
            }
        "#;
    assert_stdout(source, "0 0 0 1 1\n");
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

#[test]
fn reported_longjmp_targets_resume_inside_loops_and_switches() {
    let source = r#"
            #include <setjmp.h>
            #include <stdio.h>

            jmp_buf loop_env;
            jmp_buf switch_env;

            int main(void) {
                int count = 0;
                while (count < 2) {
                    if (setjmp(loop_env) == 0)
                        longjmp(loop_env, 1);
                    count++;
                }
                printf("%d ", count);
                switch (0) {
                    case 0:
                        if (setjmp(switch_env) == 0)
                            longjmp(switch_env, 1);
                        printf("ok\n");
                        break;
                }
                return 0;
            }
        "#;
    assert_stdout(source, "2 ok\n");
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
fn muse_function_call_arguments_remain_unsequenced_with_outer_siblings() {
    let source = r#"
            int f(int value) { return value; }
            int g(int value) { return value; }
            int main(void) {
                int i = 0;
                return f(i++) + g(i++);
            }
        "#;
    assert_diagnostic_contains(source, "unsequenced");

    let host = r#"
            #include <stdio.h>
            int main(void) {
                int i = 0;
                return printf("%d", i++) + printf("%d", i++);
            }
        "#;
    assert_diagnostic_contains(host, "unsequenced");

    let sequenced = r#"
            int f(int value) { return value; }
            int main(void) {
                int i = 0;
                return (f(i++), i++) != 1;
            }
        "#;
    assert_exit_status(sequenced, 0);
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
fn muse_typedef_name_can_also_name_a_label() {
    let source = r#"
            typedef int T;
            int main(void) {
                goto T;
                return 1;
            T:
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn muse_for_init_and_declarationless_block_objects_reach_end_of_lifetime() {
    let for_init = r#"
            int main(void) {
                int *saved;
                for (int value = 1; value; value = 0)
                    saved = &value;
                return *saved;
            }
        "#;
    assert_diagnostic_contains(for_init, "lifetime has ended");

    let compound_literal = r#"
            int main(void) {
                int *saved;
                { saved = &(int){42}; }
                return *saved;
            }
        "#;
    assert_diagnostic_contains(compound_literal, "lifetime has ended");

    let after_longjmp = r#"
            #include <setjmp.h>
            jmp_buf env;
            int *saved;
            int main(void) {
                for (int value = 1; setjmp(env) == 0; ) {
                    saved = &value;
                    longjmp(env, 1);
                }
                return *saved;
            }
        "#;
    assert_diagnostic_contains(after_longjmp, "lifetime has ended");
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

#[test]
fn muse_extended_bit_fields_follow_width_based_integer_promotions() {
    let source = r#"
            #include <stdio.h>
            struct S {
                long a : 3;
                unsigned long b : 3;
                unsigned long c : 32;
                unsigned long long d : 33;
            };
            int main(void) {
                struct S s = {1, 2, 3, 4};
                printf("%d %d %u %d\n", s.a, s.b, s.c,
                    _Generic(+s.d, unsigned long long: 1, default: 0));
                return !(_Generic(+s.a, int: 1, default: 0)
                    && _Generic(+s.b, int: 1, default: 0)
                    && _Generic(+s.c, unsigned int: 1, default: 0));
            }
        "#;
    assert_stdout(source, "1 2 3 1\n");
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

#[test]
fn muse_for_declaration_rejects_forbidden_storage_classes_and_typedefs() {
    for declaration in ["static int i=0", "extern int i", "typedef int I"] {
        let source = format!("int main(void) {{ for ({declaration}; ; ) break; return 0; }}");
        assert_diagnostic_contains(&source, "for-loop declaration");
    }
    assert_exit_status(
        "int main(void) { for (register int i=0; i<1; ++i) {} return 0; }",
        0,
    );
}

#[test]
fn muse_multidimensional_array_decay_preserves_the_selected_row() {
    let source = r#"
            #include <stdio.h>
            #include <string.h>
            int main(void) {
                char text[3][3] = {"ab", "cd", "ef"};
                char copy[3] = {0};
                memcpy(copy, text[2], 3);
                printf("%s %s\n", text[1], copy);
                return 0;
            }
        "#;
    assert_stdout(source, "cd ef\n");
}

#[test]
fn multidimensional_array_subscript_cannot_cross_a_row_boundary() {
    let read = r#"
            int main(void) {
                int matrix[2][2] = {{1, 2}, {3, 4}};
                volatile int column = 2;
                return matrix[0][column];
            }
        "#;
    assert_diagnostic_contains(read, "not valid to dereference");

    let write = r#"
            int main(void) {
                int matrix[2][2] = {{1, 2}, {3, 4}};
                volatile int column = 2;
                matrix[0][column] = 9;
                return 0;
            }
        "#;
    assert_diagnostic_contains(write, "not valid to dereference");

    let member = r#"
            struct Values { int array[2]; int next; };
            int main(void) {
                struct Values values = {{1, 2}, 3};
                volatile int index = 2;
                return values.array[index];
            }
        "#;
    assert_diagnostic_contains(member, "not valid to dereference");

    let allocated = r#"
            #include <stdlib.h>
            int main(void) {
                int (*matrix)[3] = malloc(2 * sizeof matrix[0]);
                if (!matrix) return 2;
                volatile int column = 3;
                return matrix[0][column];
            }
        "#;
    assert_diagnostic_contains(allocated, "not valid to dereference");
}

#[test]
fn audit_nonlocal_jump_after_control_entry_still_ends_block_lifetimes() {
    let goto_entry = r#"
            #include <setjmp.h>
            jmp_buf env;
            int *saved;
            int main(void) {
                goto inside;
                {
                    int value;
                inside:
                    value = 7;
                    saved = &value;
                    if (setjmp(env) == 0) longjmp(env, 1);
                }
                return *saved;
            }
        "#;
    assert_diagnostic_contains(goto_entry, "lifetime has ended");

    let switch_entry = r#"
            #include <setjmp.h>
            jmp_buf env;
            int *saved;
            int main(void) {
                switch (1) {
                    {
                        int value;
                    case 1:
                        value = 7;
                        saved = &value;
                        if (setjmp(env) == 0) longjmp(env, 1);
                    }
                }
                return *saved;
            }
    "#;
    assert_diagnostic_contains(switch_entry, "lifetime has ended");

    let switch_entry_into_for = r#"
            #include <setjmp.h>
            jmp_buf env;
            int *saved;
            int main(void) {
                switch (1) {
                    for (int value; ; ) {
                    case 1:
                        value = 7;
                        saved = &value;
                        if (setjmp(env) == 0) longjmp(env, 1);
                        break;
                    }
                }
                return *saved;
            }
    "#;
    assert_diagnostic_contains(switch_entry_into_for, "lifetime has ended");

    let repeated_jump = r#"
            #include <setjmp.h>
            jmp_buf env;
            int *saved;
            volatile int jumps;
            int main(void) {
                goto inside;
                {
                    int value;
                inside:
                    value = 7;
                    saved = &value;
                    switch (setjmp(env)) {
                        case 0: jumps = 1; longjmp(env, 1);
                        case 1: jumps = 2; longjmp(env, 2);
                    }
                }
                return *saved;
            }
        "#;
    assert_diagnostic_contains(repeated_jump, "lifetime has ended");
}

#[test]
fn audit_character_pointer_arithmetic_cannot_escape_its_designated_subobject() {
    let member = r#"
            struct Pair { char first; char second; };
            int main(void) {
                struct Pair pair = {0, 0};
                char *pointer = (char *)&pair.first;
                pointer += 2;
                return 0;
            }
        "#;
    assert_diagnostic_contains(member, "outside the bounds");

    let selected_row = r#"
            int main(void) {
                char matrix[2][2] = {{0, 0}, {0, 0}};
                char *pointer = matrix[0];
                pointer += 3;
                return 0;
            }
    "#;
    assert_diagnostic_contains(selected_row, "outside the bounds");

    let before_selected_row = r#"
            int main(void) {
                char matrix[2][2] = {{0, 0}, {0, 0}};
                char *pointer = matrix[1];
                pointer -= 1;
                return 0;
            }
    "#;
    assert_diagnostic_contains(before_selected_row, "outside the bounds");

    let allocated_selected_row = r#"
            #include <stdlib.h>
            struct Matrix { char rows[2][2]; };
            int main(void) {
                struct Matrix *matrix = malloc(sizeof *matrix);
                char *pointer = matrix->rows[0];
                pointer += 3;
                return 0;
            }
        "#;
    assert_diagnostic_contains(allocated_selected_row, "outside the bounds");

    let valid_boundaries = r#"
            struct Pair { char first; char second; };
            int main(void) {
                char matrix[2][2] = {{1, 2}, {3, 4}};
                char *row = matrix[1];
                row += 2;
                row -= 2;
                struct Pair pair = {5, 6};
                unsigned char *whole = (unsigned char *)&pair;
                whole += sizeof pair;
                whole -= sizeof pair;
                return *row != 3 || *whole != 5;
            }
        "#;
    assert_exit_status(valid_boundaries, 0);
}

#[test]
fn character_pointer_conversion_is_bounded_by_the_converted_object() {
    let array_element = r#"
            int main(void) {
                int values[2] = {1, 2};
                unsigned char *bytes = (unsigned char *)&values[0];
                bytes += sizeof values[0] + 1;
                return *bytes;
            }
        "#;
    assert_diagnostic_contains(array_element, "converted object's representation");

    let matrix_row = r#"
            int main(void) {
                int matrix[2][3] = {{1, 2, 3}, {4, 5, 6}};
                unsigned char *bytes = (unsigned char *)&matrix[0];
                bytes += sizeof matrix[0] + 1;
                return *bytes;
            }
        "#;
    assert_diagnostic_contains(matrix_row, "converted object's representation");

    let struct_element = r#"
            struct Pair { int left; int right; };
            int main(void) {
                struct Pair pairs[2] = {{1, 2}, {3, 4}};
                unsigned char *bytes = (unsigned char *)&pairs[0];
                bytes += sizeof pairs[0] + 1;
                return *bytes;
            }
        "#;
    assert_diagnostic_contains(struct_element, "converted object's representation");

    let through_void = r#"
            int main(void) {
                int values[2] = {1, 2};
                void *object = &values[0];
                unsigned char *bytes = object;
                bytes += sizeof values[0] + 1;
                return *bytes;
            }
    "#;
    assert_diagnostic_contains(through_void, "converted object's representation");

    let string_write = r#"
            #include <string.h>
            int main(void) {
                int values[3] = {0};
                strcpy((char *)&values[0], "abcdefgh");
                return 0;
            }
        "#;
    assert_diagnostic_contains(string_write, "converted object's representation");

    let string_read = r#"
            #include <string.h>
            int main(void) {
                int values[2];
                memset(&values[0], 'A', sizeof values[0]);
                memset(&values[1], 0, sizeof values[1]);
                return (int)strlen((char *)&values[0]);
            }
        "#;
    assert_diagnostic_contains(string_read, "converted object's representation");
}

#[test]
fn character_pointer_conversion_preserves_valid_whole_objects_and_round_trips() {
    let source = r#"
            #include <string.h>
            int main(void) {
                int matrix[2][3] = {{0}};
                unsigned char *whole = (unsigned char *)&matrix;
                whole += sizeof matrix;
                whole -= 1;
                if (*whole != 0) return 1;

                int values[3] = {1, 2, 3};
                int *original = values;
                void *erased = original;
                int *restored = erased;
                restored += 2;
                if (*restored != 3) return 2;

                memset((unsigned char *)&matrix, 0, sizeof matrix);
                int copy[3] = {0};
                memcpy((unsigned char *)&copy[0], values, sizeof values);
                if (copy[2] != 3) return 3;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);
}

#[test]
fn byte_counted_memory_functions_can_span_nested_array_rows() {
    let source = r#"
            #include <string.h>
            int main(void) {
                int source[2][3] = {{1, 2, 3}, {4, 5, 6}};
                int copy[2][3] = {{0}};
                memcpy(&copy[0][0], &source[0][0], sizeof source);
                if (copy[1][2] != 6) return 1;

                memmove(&copy[0][1], &copy[0][0], 5 * sizeof(int));
                if (copy[0][1] != 1 || copy[1][2] != 5) return 2;
                memset(&copy[0][2], 0, 2 * sizeof(int));
                if (copy[0][2] != 0 || copy[1][0] != 0) return 3;

                int equal[2][3] = {{1, 2, 3}, {4, 5, 6}};
                if (memcmp(&source[0][0], &equal[0][0], sizeof source)) return 4;

                unsigned char bytes[2][3] = {{1, 2, 3}, {4, 5, 6}};
                if (memchr(&bytes[0][0], 5, sizeof bytes) != &bytes[1][1]) return 5;

                struct Matrix { int guard; int values[2][2]; int tail; };
                struct Matrix from = {7, {{1, 2}, {3, 4}}, 8};
                struct Matrix to = {9, {{0}}, 10};
                memcpy(&to.values[0][0], &from.values[0][0], sizeof from.values);
                if (to.guard != 9 || to.values[1][1] != 4 || to.tail != 10) return 6;
                return 0;
            }
        "#;
    assert_exit_status(source, 0);

    let member_overflow = r#"
            #include <string.h>
            struct S { int member[2]; int next; };
            int main(void) {
                struct S value = {{0, 0}, 0};
                int source[3] = {1, 2, 3};
                memcpy(value.member, source, sizeof source);
                return 0;
            }
        "#;
    assert_diagnostic_contains(member_overflow, "containing record member");
}

#[test]
fn audit_function_designator_and_arguments_obey_call_sequencing() {
    let unsequenced_designator = r#"
            int identity(int value) { return value; }
            int main(void) {
                int (*functions[2])(int) = {identity, identity};
                int index = 0;
                return functions[index++](index++);
            }
        "#;
    assert_diagnostic_contains(unsequenced_designator, "unsequenced");

    let sequenced_body = r#"
            int value;
            int replace(int argument) {
                value = argument;
                return value;
            }
            int main(void) {
                value = 3;
                return replace(value + 1) != 4;
            }
        "#;
    assert_exit_status(sequenced_body, 0);
}

#[test]
fn audit_compound_literals_obey_selection_and_iteration_block_lifetimes() {
    let selection = r#"
            int main(void) {
                int *saved = 0;
                if (1)
                    saved = &(int){7};
                return *saved;
            }
        "#;
    assert_diagnostic_contains(selection, "lifetime has ended");

    let iteration = r#"
            int main(void) {
                int *saved = 0;
                int run = 1;
                while (run--)
                    saved = &(int){7};
                return *saved;
            }
    "#;
    assert_diagnostic_contains(iteration, "lifetime has ended");

    let between_iterations = r#"
            int main(void) {
                int *saved = 0;
                int iteration = 0;
                while (iteration++ < 2)
                    iteration == 1 ? (saved = &(int){7}, 0) : *saved;
                return 0;
            }
    "#;
    assert_diagnostic_contains(between_iterations, "lifetime has ended");

    let between_for_iterations = r#"
            int main(void) {
                int *saved = 0;
                for (int iteration = 0; iteration < 2; ++iteration)
                    iteration == 0 ? (saved = &(int){7}, 0) : *saved;
                return 0;
            }
        "#;
    assert_diagnostic_contains(between_for_iterations, "lifetime has ended");

    let controlling_expressions = r#"
            int main(void) {
                int *saved = 0;
                if ((saved = &(int){7}, 1))
                    ;
                return *saved;
            }
        "#;
    assert_diagnostic_contains(controlling_expressions, "lifetime has ended");

    let switch_expression = r#"
            int main(void) {
                int *saved = 0;
                switch ((saved = &(int){7}, 0)) {
                    default: ;
                }
                return *saved;
            }
    "#;
    assert_diagnostic_contains(switch_expression, "lifetime has ended");

    let valid_during_statement = r#"
            int main(void) {
                int *saved = 0;
                if ((saved = &(int){7}, 1))
                    if (*saved != 7) return 1;
                switch ((saved = &(int){8}, 0)) {
                    default: if (*saved != 8) return 2;
                }
                return 0;
            }
    "#;
    assert_exit_status(valid_during_statement, 0);

    let goto_entry = r#"
            int main(void) {
                int *saved = 0;
                goto inside;
                if (0)
                inside:
                    saved = &(int){7};
                return *saved;
            }
    "#;
    assert_diagnostic_contains(goto_entry, "lifetime has ended");

    let switch_entry_into_loop = r#"
            int main(void) {
                int *saved = 0;
                switch (1) {
                    while (0)
                    case 1:
                        saved = &(int){7};
                }
                return *saved;
            }
        "#;
    assert_diagnostic_contains(switch_entry_into_loop, "lifetime has ended");
}

fn temp_test_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("c_interpreter-{label}-{unique}"))
}
