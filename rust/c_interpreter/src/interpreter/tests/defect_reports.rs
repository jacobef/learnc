//! WG14 defect-report regressions. Each DR number refers to the committee
//! response, not an unadopted proposal in the report. Sources:
//! https://www.open-std.org/jtc1/sc22/wg14/www/docs/summary-c99.htm
//! https://www.open-std.org/jtc1/sc22/wg14/www/docs/summary.htm

use super::*;

#[test]
fn dr237_static_prototype_contracts_cross_translation_units_without_leaking_internal_names() {
    let options = RunOptions::default();
    let files = vec![
        (
            PathBuf::from("main.c"),
            "int f(int *); int main(void) { int a[1] = {0}; return f(a); }\n".to_owned(),
        ),
        (
            PathBuf::from("f.c"),
            "int f(int p[static 3]); int f(int *p) { return p[0]; }\n".to_owned(),
        ),
    ];
    let err = crate::run_virtual_sources_with_options(&files, &options)
        .err()
        .expect("static contract must apply across files");
    assert_eq!(err.severity(), crate::diag::Severity::UndefinedBehavior);
    assert!(err.message().contains("static"), "{}", err.render());
    let files = vec![
        (
            PathBuf::from("main.c"),
            "static int f(int *p) { return p[0]; } int main(void) { int a[1] = {0}; return f(a); }\n".to_owned(),
        ),
        (
            PathBuf::from("f.c"),
            "static int f(int p[static 3]); static int f(int *p) { return p[0]; }\n".to_owned(),
        ),
    ];
    assert_eq!(
        crate::run_virtual_sources_with_options(&files, &options)
            .unwrap_or_else(|err| panic!("{}", err.render()))
            .exit_status,
        0
    );
}

macro_rules! defined {
    ($name:ident, $source:expr) => {
        #[test]
        fn $name() {
            let result =
                run_source("test.c", $source).unwrap_or_else(|err| panic!("{}", err.render()));
            assert_eq!(result.exit_status, 0, "{}", result.stdout);
            assert!(result.execution_limit.is_none());
        }
    };
}

macro_rules! rejected {
    ($name:ident, $source:expr, $severity:ident, $message:expr) => {
        #[test]
        fn $name() {
            let err = run_source("test.c", $source).expect_err(stringify!($name));
            assert_eq!(
                err.severity(),
                crate::diag::Severity::$severity,
                "{}",
                err.render()
            );
            assert!(err.render().contains($message), "{}", err.render());
        }
    };
}

defined!(
    dr317_empty_parentheses_definition_does_not_create_a_prototype,
    r#"
    void f() {}
    int main(void) { if (0) f(1); return 0; }
"#
);
rejected!(
    dr317_executed_extra_argument_to_empty_definition_is_ub,
    r#"
    void f() {}
    int main(void) { f(1); return 0; }
"#,
    UndefinedBehavior,
    "parameter(s)"
);
rejected!(
    dr262_bitfield_width_cannot_exceed_its_type_width,
    r#"
    struct S { _Bool b : 2; };
    int main(void) { return 0; }
"#,
    Error,
    "width"
);
defined!(
    dr290_assignment_cast_and_register_stores_remove_extra_precision,
    r#"
    int main(void) {
        double x = 16777217.0;
        float a = x; register float b = x; double c = (float)x;
        return a != 16777216.0 || b != 16777216.0 || c != 16777216.0;
    }
"#
);
defined!(
    dr276_dr322_perror_on_unoriented_stderr_accepts_both_permitted_orientations,
    r#"
    #include <stdio.h>
    #include <wchar.h>
    #include <errno.h>
    int main(void) { errno = EDOM; perror("test"); return fwide(stderr, 0) > 0; }
"#
);

rejected!(
    dr219_partial_copy_preserves_multiple_members_across_padding,
    r#"
    #include <stdlib.h>
    #include <stddef.h>
    #include <string.h>
    struct S { char c; int i; long tail; };
    int main(void) {
        struct S s = {0, 0, 0}; void *p = malloc(offsetof(struct S, tail));
        memcpy(p, (char *)&s, offsetof(struct S, tail));
        return *(float *)((char *)p + offsetof(struct S, i)) != 0;
    }
"#,
    UndefinedBehavior,
    "effective type"
);
rejected!(
    dr219_character_pointer_whole_copy_preserves_the_record_type,
    r#"
    #include <stdlib.h>
    #include <string.h>
    struct A { char c; }; struct B { char c; };
    int main(void) { struct A a = {1}; void *p = malloc(sizeof a);
        memcpy(p, (char *)&a, sizeof a); return (0, *(struct B *)p).c; }
"#,
    UndefinedBehavior,
    "effective type"
);
rejected!(
    dr219_partial_dynamic_copy_preserves_subobject_types,
    r#"
    #include <stdlib.h>
    #include <string.h>
    struct S { int a[4]; int b; };
    int main(void) { struct S s = {{0, 0, 0, 0}, 0};
        void *p = malloc(sizeof s), *q = malloc(2 * sizeof(int));
        memcpy(p, &s, sizeof s); memmove(q, (char *)p + sizeof(int), 2 * sizeof(int));
        return ((float *)q)[1] != 0;
    }
"#,
    UndefinedBehavior,
    "effective type"
);
defined!(
    dr219_partial_copy_allows_compatible_scalar_accesses,
    r#"
    #include <stdlib.h>
    #include <stddef.h>
    #include <string.h>
    struct S { char c; int i; long tail; };
    int main(void) { struct S s = {7, 19, 0};
        void *p = malloc(offsetof(struct S, tail));
        memcpy(p, (char *)&s, offsetof(struct S, tail));
        int result = *(char *)p != 7 || *(int *)((char *)p + offsetof(struct S, i)) != 19;
        free(p); return result;
    }
"#
);
defined!(
    dr219_declared_character_array_can_be_copied_to_raw_storage,
    r#"
    #include <stdlib.h>
    #include <string.h>
    int main(void) { unsigned char a[4] = {0}; void *p = malloc(4);
        memcpy(p, a, 4); int result = *(unsigned char *)p; free(p); return result; }
"#
);
rejected!(
    dr237_static_bounds_apply_to_later_and_hidden_declarations,
    r#"
    int f(int *p) { return p[0]; }
    int main(void) { int a[2] = {0}; return f(a); }
    void g(void) { if (0) { int f(int p[static 3]); } }
"#,
    UndefinedBehavior,
    "static"
);
rejected!(
    dr237_renamed_prototype_parameters_bind_by_position,
    r#"
    int f(int n, int p[static n]);
    int f(int count, int *values) { return values[0]; }
    int main(void) { int a[1] = {0}; return f(3, a); }
"#,
    UndefinedBehavior,
    "static"
);
rejected!(
    dr237_all_prototype_minima_apply_to_indirect_calls,
    r#"
    int f(int p[static 2]); int f(int q[static 5]);
    int f(int *p) { return p[0]; }
    int main(void) { int a[3] = {0}; int (*fp)(int *) = f; return fp(a); }
"#,
    UndefinedBehavior,
    "static"
);
rejected!(
    dr237_static_minima_count_remaining_elements,
    r#"
    int f(int a[static 3]); int f(int *a) { return a[0]; }
    int main(void) { int a[3] = {0}; return f(a + 1); }
"#,
    UndefinedBehavior,
    "static"
);
defined!(
    dr237_sufficient_array_satisfies_every_prototype,
    r#"
    int f(int n, int a[static n]); int f(int n, int a[static 2]);
    int f(int count, int *p) { return p[count - 1]; }
    int main(void) { int a[4] = {0}; return f(3, a + 1); }
"#
);
defined!(
    dr265_unsigned_suffix_and_64_bit_boundary_stay_unsigned,
    r#"
    #if !(0xffffffffU < -1) || !(0xffffffffffffffff > -2)
    #error incorrect unsigned preprocessor arithmetic
    #endif
    #if 0x7fffffffffffffffL < -1
    #error incorrect signed preprocessor arithmetic
    #endif
    int main(void) { return 0; }
"#
);

defined!(
    dr208_overridden_initializers_have_optional_side_effects,
    r#"
    int calls;
    int f(int n) { ++calls; return n; }
    int main(void) {
        int a[2] = {f(0), f(1), [0] = f(2)};
        return a[0] != 2 || a[1] != 1 || (calls != 2 && calls != 3);
    }
"#
);

defined!(
    dr215_scalar_objects_support_one_past_pointer_equality,
    r#"
    int main(void) { int x = 0; int *p = &x; return p + 1 != &p[1]; }
"#
);

rejected!(
    dr219_character_pointer_copy_preserves_declared_subobject_type,
    r#"
    #include <stdlib.h>
    #include <string.h>
    #include <stddef.h>
    struct S { long prefix; int i; long tail; };
    int main(void) {
        struct S s = {0, 0, 0};
        void *p = malloc(sizeof s.i);
        memcpy(p, (char *)&s + offsetof(struct S, i), sizeof s.i);
        return *(float *)p != 0.0f;
    }
"#,
    UndefinedBehavior,
    "effective type"
);

defined!(
    dr222_aggregate_copy_preserves_partially_initialized_members,
    r#"
    struct S { int a; int *b; };
    int main(void) { struct S x; x.a = 1; struct S y = x; return y.a != 1; }
"#
);

rejected!(
    dr236_incompatible_typed_aliases_of_allocated_storage,
    r#"
    #include <stdlib.h>
    int f(int *i, double *d) { *i = 2; *d = 3.0; return *i; }
    int main(void) { void *p = malloc(sizeof(double)); return f(p, p); }
"#,
    UndefinedBehavior,
    "effective type"
);

rejected!(
    dr237_static_array_bound_on_a_prototype_constrains_calls,
    r#"
    int f(int a[static 3]);
    int f(int *a) { return a[0]; }
    int main(void) { int a[1] = {1}; return f(a); }
"#,
    UndefinedBehavior,
    "static"
);

defined!(
    dr253_braced_whole_object_initializer_overrides_previous_members,
    r#"
    struct S { char s[6]; int n; };
    int main(void) {
        struct S a[] = {{{"abc"}, 1}, [0] = {.s[0] = 'q'}};
        return a[0].s[0] != 'q' || a[0].s[1] != 0 || a[0].n != 0;
    }
"#
);

defined!(
    dr257_common_initial_sequence_survives_different_trailing_members,
    r#"
    union U { struct { int tag; double value; } a; struct { int tag; char value; } b; };
    int main(void) { union U u = {.a = {19, 2.5}}; return u.b.tag != 19; }
"#
);

defined!(
    dr259_empty_macro_invocations_distinguish_zero_and_one_parameter,
    r#"
    #define ZERO() 7
    #define ONE(x) #x
    int main(void) { return ZERO() != 7 || sizeof ONE() != 1; }
"#
);

rejected!(
    dr261_evaluated_comma_expression_is_not_a_null_pointer_constant,
    r#"
    int main(void) { int *p = (0, 0); return p != 0; }
"#,
    Error,
    "pointer"
);

defined!(
    dr263_all_zero_bits_represent_integer_zero,
    r#"
    #include <stdlib.h>
    #include <string.h>
    int main(void) {
        long a[2]; memset(a, 0, sizeof a);
        long long *b = calloc(2, sizeof *b);
        int result = a[0] != 0 || a[1] != 0 || b[0] != 0 || b[1] != 0;
        free(b); return result;
    }
"#
);

defined!(
    dr265_preprocessor_literal_typing_uses_intmax_width,
    r#"
    #if 0xffffffff < -1
    #error hexadecimal constant should be signed in preprocessor arithmetic
    #endif
    int main(void) { return 0; }
"#
);

defined!(
    dr268_jump_into_while_body_still_repeats_the_loop,
    r#"
    int main(void) {
        int i = 0, count = 0;
        goto inside;
        while (++i < 3) { inside: ++count; }
        return i != 3 || count != 3;
    }
"#
);

defined!(
    dr282_struct_assignment_does_not_copy_flexible_array_tail,
    r#"
    #include <stdlib.h>
    struct S { int n; double d[]; };
    int main(void) {
        struct S *a = malloc(sizeof *a + 2 * sizeof a->d[0]);
        struct S *b = malloc(sizeof *b + 2 * sizeof b->d[0]);
        a->n = 2; a->d[1] = 19; b->n = 1; b->d[1] = 23;
        *b = *a;
        int result = b->n != 2 || b->d[1] != 23;
        free(a); free(b); return result;
    }
"#
);

defined!(
    dr283_union_type_punning_reinterprets_the_representation,
    r#"
    union U { unsigned int i; float f; };
    int main(void) { union U u = {.i = 0x3f800000u}; return u.f != 1.0f; }
"#
);

defined!(
    dr289_abstract_array_parameters_accept_restrict,
    r#"
    int f(int [restrict]);
    int f(int a[restrict]) { return a[0]; }
    int main(void) { int a[1] = {19}; return f(a) != 19; }
"#
);

rejected!(
    dr298_integer_constants_must_fit_a_supported_type,
    r#"
    int main(void) { unsigned long long x = 18446744073709551616ULL; return 0; }
"#,
    Error,
    "integer literal"
);

rejected!(
    dr304_invalid_preprocessing_number_requires_a_diagnostic,
    r#"
    #if 4hello
    #endif
    int main(void) { return 0; }
"#,
    Error,
    "integer"
);

defined!(
    dr305_dr344_keywords_are_identifiers_in_preprocessor_expressions,
    r#"
    #if (int) + 0
    #error int is replaced with zero, not parsed as a cast
    #endif
    #if sizeof || return
    #error keywords are replaced with zero
    #endif
    int main(void) { return 0; }
"#
);

defined!(
    dr306_object_like_macro_replacements_are_rescanned,
    r#"
    #define F(x) ((x) + 1)
    #define A B
    #define B F
    int main(void) { return A(18) != 19; }
"#
);

rejected!(
    dr338_never_addressed_uninitialized_unsigned_char_is_ub,
    r#"
    int main(void) { unsigned char x; return x; }
"#,
    UndefinedBehavior,
    "uninitialized"
);

defined!(
    dr400_realloc_zero_size_has_an_implementation_defined_outcome,
    r#"
    #include <stdlib.h>
    int main(void) { void *p = malloc(1); void *q = realloc(p, 0); free(q); return 0; }
"#
);

defined!(
    dr412_invalid_elif_after_a_taken_branch_is_skipped,
    r#"
    #if 1
    int main(void) { return 0; }
    #elif this is not a valid expression
    #error skipped
    #endif
"#
);

defined!(
    dr413_implicit_initialization_does_not_override_explicit_nested_values,
    r#"
    typedef struct { int k; int l; int a[2]; } T;
    typedef struct { int i; T t; } S;
    int main(void) {
        T x = {.l = 43, .k = 42, .a[1] = 19, .a[0] = 18};
        S s = {1, .t = x, .t.l = 41, .t.a[1] = 17};
        return s.i != 1 || s.t.k != 42 || s.t.l != 41 || s.t.a[0] != 18 || s.t.a[1] != 17;
    }
"#
);

defined!(
    dr427_parameter_and_return_values_initialize_const_members,
    r#"
    struct S { const int x; };
    struct S f(const struct S x) { return x; }
    int main(void) { struct S s = {19}; struct S t = f(s); return t.x != 19; }
"#
);

defined!(
    dr439_initializer_list_side_effects_are_indeterminately_sequenced,
    r#"
    int main(void) {
        int i = 0; int a[3] = {i++, i++, i++};
        int *b = (int[3]){i++, i++, i++};
        return i != 6 || a[0] + a[1] + a[2] != 3 || b[0] + b[1] + b[2] != 12;
    }
"#
);

rejected!(
    dr439_compound_literal_initializer_and_outer_read_are_unsequenced,
    r#"
    int main(void) { int i = 0; return ((int[2]){i++, i++})[i % 2]; }
"#,
    UndefinedBehavior,
    "unsequenced"
);

defined!(
    dr446_memory_functions_count_bytes_including_embedded_nulls,
    r#"
    #include <string.h>
    int main(void) {
        unsigned char a[4] = {0, 0xc3, 0xa9, 255}, b[4];
        memcpy(b, a, 4); memmove(b + 1, b, 3);
        return b[0] != 0 || b[1] != 0 || b[2] != 0xc3 || b[3] != 0xa9;
    }
"#
);

defined!(
    dr447_complex_to_bool_tests_both_components,
    r#"
    #include <complex.h>
    int main(void) { _Bool a = 0.0 + 2.0 * I; _Bool b = 0.0 + 0.0 * I; return a != 1 || b != 0; }
"#
);

rejected!(
    dr451_library_calls_reject_addressed_indeterminate_values,
    r#"
    #include <stdio.h>
    int main(void) { int x; (void)&x; printf("%d", x); return 0; }
"#,
    UndefinedBehavior,
    "indeterminate"
);

rejected!(
    dr452_comma_rvalue_does_not_bypass_effective_type_checks,
    r#"
    #include <stdlib.h>
    #include <string.h>
    struct S { int i; };
    struct T { int i; };
    int main(void) {
        struct S s = {19}; void *p = malloc(sizeof s); memcpy(p, &s, sizeof s);
        return (0, *(struct T *)p).i;
    }
"#,
    UndefinedBehavior,
    "effective type"
);

defined!(
    dr460_invalid_aligned_alloc_requests_fail_with_null,
    r#"
    #include <stdlib.h>
    int main(void) {
        void *a = aligned_alloc(0, 16), *b = aligned_alloc(3, 12), *c = aligned_alloc(8, 9);
        return a != 0 || b != 0 || c != 0;
    }
"#
);

rejected!(
    dr463_signed_left_shift_into_the_sign_bit_remains_ub,
    r#"
    int main(void) { return 1 << 31; }
"#,
    UndefinedBehavior,
    "overflow"
);

defined!(
    dr466_for_body_can_shadow_the_control_variable,
    r#"
    int main(void) { for (int i = 0; i < 1; ++i) { long i = 19; if (i != 19) return 1; } return 0; }
"#
);

defined!(
    dr483_location_macros_in_replacement_lists_use_the_invocation_site,
    r#"
    #include <string.h>
    #line 500 "definition.c"
    #define LOC() __LINE__
    #define FILE_NAME() __FILE__
    #line 1000 "invocation.c"
    int line = LOC();
    char *file = FILE_NAME();
    int main(void) { return line != 1000 || strcmp(file, "invocation.c") != 0; }
"#
);

defined!(
    dr489_unevaluated_division_does_not_break_a_constant_expression,
    r#"
    static int x = 2 || 1 / 0;
    enum { Y = 1 ? 19 : 1 / 0 };
    int main(void) { return x != 1 || Y != 19; }
"#
);

defined!(
    dr499_anonymous_struct_members_do_not_overlap_in_a_union,
    r#"
    union U { struct { char a, b, c, d; }; int word; };
    int main(void) { union U u; u.a = 1; u.b = 2; u.c = 3; u.d = 4;
        return &u.a == &u.b || u.a != 1 || u.b != 2 || u.c != 3 || u.d != 4; }
"#
);

rejected!(
    dr502_anonymous_flexible_array_still_requires_another_named_member,
    r#"
    struct S { int n; struct { int a[]; }; };
    int main(void) { return 0; }
"#,
    Error,
    "named member"
);

defined!(
    dr225_dr503_strtod_accepts_longest_valid_hex_and_infinity_prefixes,
    r#"
    #include <stdlib.h>
    #include <math.h>
    int main(void) {
        char *end; double a = strtod("0x123tail", &end);
        if (a != 291.0 || *end != 't') return 1;
        double b = strtod("INFINF", &end);
        return !isinf(b) || *end != 'I';
    }
"#
);

defined!(
    dr233_general_float_format_selects_style_after_rounding,
    r#"
    #include <stdio.h>
    #include <string.h>
    int main(void) {
        char s[80]; sprintf(s, "%.4g %.0g %#.3g", 9999.5, 12.0, 1.0);
        return strcmp(s, "1e+04 1e+01 1.00") != 0;
    }
"#
);

defined!(
    dr260_memcpy_preserves_pointer_provenance,
    r#"
    #include <string.h>
    int main(void) { int x = 19; int *p = &x, *q;
        memcpy(&q, &p, sizeof p); return *q != 19; }
"#
);
rejected!(
    dr260_copied_pointer_becomes_indeterminate_at_the_end_of_lifetime,
    r#"
    #include <stdlib.h>
    #include <string.h>
    int main(void) { int *p = malloc(sizeof *p), *q;
        *p = 19; memcpy(&q, &p, sizeof p); free(p); return *q; }
"#,
    UndefinedBehavior,
    "lifetime has ended"
);
defined!(
    dr337_stdio_size_macros_meet_the_required_minima,
    r#"
    #include <stdio.h>
    _Static_assert(BUFSIZ >= 256, "buffer size");
    _Static_assert(FOPEN_MAX >= 8, "open files");
    _Static_assert(FILENAME_MAX >= 1, "filename size");
    int main(void) { return 0; }
"#
);
rejected!(
    dr324_unknown_string_escapes_require_a_diagnostic,
    r#"
    int main(void) { return "\q"[0]; }
"#,
    Error,
    "escape"
);
defined!(
    dr296_exact_infinity_from_infinite_input_is_not_a_range_error,
    r#"
    #include <math.h>
    #include <errno.h>
    int main(void) { errno = 0; double x = exp(INFINITY);
        return !isinf(x) || errno != 0; }
"#
);
defined!(
    dr323_complex_and_i_macros_may_be_undefined_and_redefined,
    r#"
    #include <complex.h>
    float complex z = I;
    #undef complex
    #undef I
    #define complex 7
    #define I 19
    int main(void) { return complex != 7 || I != 19 || cimagf(z) != 1; }
"#
);
defined!(
    dr256_multiple_standard_header_inclusions_preserve_existing_use,
    r#"
    #include <stdio.h>
    #undef FOPEN_MAX
    #include <stdio.h>
    int main(void) { FILE *p = tmpfile(); if (!p) return 0; return fclose(p); }
"#
);
defined!(
    dr315_narrow_unsigned_bitfields_promote_using_their_width,
    r#"
    struct S { unsigned int x : 3; };
    int main(void) { struct S s = {7};
        return _Generic(+s.x, int: 0, default: 1) || (+s.x - 8) != -1; }
"#
);
defined!(
    dr340_array_composite_types_combine_element_types_recursively,
    r#"
    extern int (*a[])[3]; extern int (*a[2])[]; int (*a[2])[3] = {0};
    int main(void) { return sizeof a != 2 * sizeof(void *) || sizeof *a[0] != 3 * sizeof(int); }
"#
);

defined!(
    dr237_prototype_scope_bounds_do_not_execute_side_effects,
    r#"
    int n;
    int f(int p[static ++n]); int f(int *p) { return p[0]; }
    int main(void) { int a[4] = {0}; f(a); return n != 0; }
"#
);
