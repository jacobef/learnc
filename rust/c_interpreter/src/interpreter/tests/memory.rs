use super::*;

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
fn synthetic_address_base_shifts_addresses_without_changing_layout() {
    let source = r#"
            int main(void) {
                int a = 1;
                int b = 2;
                a = b;
                return 0;
            }
        "#;
    let addresses =
        |result: &ProgramOutput| (state_address(result, "a"), state_address(result, "b"));
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
