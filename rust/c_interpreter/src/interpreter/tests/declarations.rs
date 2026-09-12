use super::*;

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
fn functions_cannot_return_function_types() {
    let source = "int function(void)(void); int main(void) { return 0; }";
    assert_diagnostic_contains(source, "cannot return a function type");
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
fn internal_tentative_definition_must_have_complete_type() {
    let err = run_source("test.c", "static int a[]; int main(void) { return 0; }\n").unwrap_err();
    assert!(
        err.render().contains("must have complete type"),
        "{}",
        err.render()
    );
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
