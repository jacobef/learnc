use super::*;

#[test]
fn standard_example_mktime_ignores_output_fields() {
    assert_exit_status(
        include_str!("../../../tests/standard_examples/mktime_ignores_output_fields.c"),
        0,
    );
}

#[test]
fn standard_example_long_double_math_uses_the_interpreted_format() {
    assert_exit_status(
        include_str!(
            "../../../tests/standard_examples/long_double_math_uses_the_interpreted_format.c"
        ),
        0,
    );
}

#[test]
fn standard_example_vla_parameters_retain_inner_dimensions() {
    assert_exit_status(
        include_str!("../../../tests/standard_examples/vla_parameters_retain_inner_dimensions.c"),
        0,
    );
}

#[test]
fn standard_example_transform_sizing_null_exception_is_narrow() {
    assert_exit_status(
        include_str!(
            "../../../tests/standard_examples/transform_sizing_null_exception_is_narrow.c"
        ),
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
        include_str!(
            "../../../tests/standard_examples/scanf_percent_skips_only_leading_whitespace.c"
        ),
        0,
    );
}

#[test]
fn floating_environment_tracks_exceptions() {
    assert_exit_status(
        include_str!("../../../tests/standard_examples/floating_environment_tracks_exceptions.c"),
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
fn standard_example_operator_precedence_and_associativity() {
    let result = run_source(
        "operator_precedence.c",
        include_str!("../../../tests/standard_examples/operator_precedence.c"),
    )
    .unwrap();
    assert_eq!(result.exit_status, 0);
}
