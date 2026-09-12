use super::*;

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
            .all(|event| event.state.len() <= crate::interpreter::CBOXES_MAX_ARRAY_ELEMENTS + 2)
    );
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
