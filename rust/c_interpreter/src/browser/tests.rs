use super::*;
use crate::{
    NativeExecutionOptions, NativeStreamIo, run_files_with_options, run_native_source, run_source,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
struct ChunkedNativeStreamIo {
    stdin: Mutex<VecDeque<u8>>,
    stdout: Mutex<Vec<u8>>,
    stderr: Mutex<Vec<u8>>,
}

impl ChunkedNativeStreamIo {
    fn new(stdin: &[u8]) -> Self {
        Self {
            stdin: Mutex::new(stdin.iter().copied().collect()),
            stdout: Mutex::new(Vec::new()),
            stderr: Mutex::new(Vec::new()),
        }
    }
}

impl NativeStreamIo for ChunkedNativeStreamIo {
    fn read_stdin(&self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut stdin = self.stdin.lock().unwrap();
        let count = buffer.len().min(stdin.len()).min(3);
        for slot in &mut buffer[..count] {
            *slot = stdin.pop_front().unwrap();
        }
        Ok(count)
    }

    fn write_stdout(&self, bytes: &[u8]) -> std::io::Result<()> {
        self.stdout.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }

    fn write_stderr(&self, bytes: &[u8]) -> std::io::Result<()> {
        self.stderr.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn native_stream_io_incrementally_reads_and_writes_standard_streams() {
    let stream = Arc::new(ChunkedNativeStreamIo::new(b"alpha\nbeta\n"));
    let source = format!(
        "{}\n",
        r#"
        #include <stdio.h>
        int main(void) {
            char first[8];
            char second[8];
            if (!fgets(first, sizeof(first), stdin)) return 1;
            printf("out:%s", first);
            if (!fgets(second, sizeof(second), stdin)) return 2;
            fprintf(stderr, "err:%s", second);
            return 0;
        }
    "#
        .trim()
    );
    let result = run_native_source(
        "stream.c",
        source,
        &NativeExecutionOptions {
            stream_io: Some(stream.clone()),
            ..NativeExecutionOptions::default()
        },
    )
    .unwrap();

    assert_eq!(result.exit_status, 0);
    assert_eq!(result.stdout, "out:alpha\n");
    assert_eq!(result.stderr, "err:beta\n");
    assert_eq!(*stream.stdout.lock().unwrap(), b"out:alpha\n");
    assert_eq!(*stream.stderr.lock().unwrap(), b"err:beta\n");
}

#[test]
fn browser_entry_point_accepts_the_current_request_schema() {
    let source = b"int main(void) { int answer = 42; return answer != 42; }\n";
    let words = [
        CBOXES_BRIDGE_SCHEMA_ID,
        0,
        0,
        0x1000,
        0,
        10_000,
        256,
        source.len() as u32,
        0,
        0,
    ];
    let mut request = Vec::with_capacity(CBOXES_BRIDGE_HEADER_WORDS * 4 + source.len());
    for word in words {
        request.extend_from_slice(&word.to_le_bytes());
    }
    request.extend_from_slice(source);

    let result_ptr = unsafe { cboxes_execute(request.as_ptr(), request.len()) };
    let result_len = cboxes_last_result_len();
    let json = unsafe { std::slice::from_raw_parts(result_ptr, result_len) }.to_vec();
    unsafe { cboxes_free(result_ptr, result_len) };
    let json = String::from_utf8(json).unwrap();

    assert!(json.contains("\"ok\":true"), "{json}");
    assert!(json.contains("\"exitStatus\":0"), "{json}");
    assert!(json.contains("\"trace\":[{"), "{json}");
    assert!(json.contains("\"executionLimit\":null"), "{json}");
}

#[test]
fn browser_request_rejects_a_different_schema() {
    let mut request = vec![0; CBOXES_BRIDGE_HEADER_WORDS * 4];
    request[..4].copy_from_slice(&(CBOXES_BRIDGE_SCHEMA_ID + 1).to_le_bytes());
    let error = CboxesBridgeRequest::decode(&request).unwrap_err();
    assert!(error.contains("request schema"));
}

#[test]
fn browser_diagnostics_preserve_the_full_source_range() {
    let source = "int main(void) {\n  char a = \"hi\";\n}\n";
    let diagnostic = run_source("program.c", source).unwrap_err();
    let range = diagnostic.display_range().unwrap();

    assert_eq!(range.path, PathBuf::from("program.c"));
    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (1, 11, 1, 15)
    );

    let json = cboxes_diagnostic_json(
        &diagnostic,
        &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
    );
    assert!(json.contains("\"line\":1,\"column\":11"));
    assert!(json.contains("\"endLine\":1,\"endColumn\":15"));
    assert!(json.contains("\"runtimeContext\":null"), "{json}");
}

#[test]
fn runtime_ub_reports_when_it_happened_and_the_pre_failure_state() {
    let source = format!(
        "{}\n",
        r#"
        int main(void) {
            int x = 0;
            while (x < 4) {
                int y = 3 / (3 - x);
                x++;
            }
            return 0;
        }
    "#
        .trim()
    );
    let diagnostic = run_source_with_options(
        "program.c",
        source.clone(),
        &RunOptions {
            execution_step_limit: Some(10_000),
            ..RunOptions::default()
        },
    )
    .unwrap_err();
    let context = diagnostic.runtime_context().unwrap_or_else(|| {
        panic!(
            "runtime undefined behavior should retain execution context: {}",
            diagnostic.render()
        )
    });

    assert!(context.executed_steps > 0);
    assert_eq!(context.line_execution_count, Some(4));
    assert!(
        context
            .state
            .iter()
            .any(|object| object.name == "x" && object.value == "3"),
        "last recorded state before the failure was {:#?}",
        context.state,
    );

    let json = cboxes_diagnostic_json(
        &diagnostic,
        &HashMap::from([("program.c".to_owned(), cboxes_source_display(&source, 0))]),
    );
    assert!(json.contains("\"runtimeContext\":{"), "{json}");
    assert!(json.contains("\"lineExecutionCount\":4"), "{json}");
    assert!(json.contains("\"name\":\"x\""), "{json}");
}

#[test]
fn browser_eof_diagnostics_stay_at_the_original_source_end() {
    let source = "int f(int x) {\n}\n\nint a";
    let run = cboxes_run_virtual_sources(
        vec![(PathBuf::from("program.c"), source.to_owned())],
        &RunOptions::default(),
        false,
    )
    .unwrap();
    let diagnostic = run.result.unwrap_err();
    let json = cboxes_diagnostic_json(&diagnostic, &run.source_display);

    assert!(json.contains("\"file\":\"program.c\""), "{json}");
    assert!(json.contains("\"line\":3,\"column\":5"), "{json}");
}

#[test]
fn missing_semicolon_at_an_include_boundary_points_into_the_header() {
    let files = vec![
        (
            PathBuf::from("main.c"),
            "#include \"bad.h\"\nint main(void) {}".to_owned(),
        ),
        (PathBuf::from("bad.h"), "extern int object".to_owned()),
    ];
    let run = cboxes_run_virtual_sources(files, &RunOptions::default(), false).unwrap();
    let diagnostic = run.result.unwrap_err();
    let range = diagnostic.display_range().unwrap();

    assert_eq!(range.path, PathBuf::from("bad.h"));
    assert_eq!((range.start_line, range.start_column), (0, 11));
}

#[test]
fn member_and_initializer_diagnostics_do_not_blame_following_punctuation() {
    let cases = [
        ("struct S { int x; char x; }; int main(void) {}\n", "x"),
        ("struct S { void x; }; int main(void) {}\n", "x"),
        ("struct S { int x : 0; }; int main(void) {}\n", "0"),
        ("struct S { int a[]; int x; }; int main(void) {}\n", "a[]"),
        ("int main(void) { int x = {1, 2}; }\n", "2"),
        ("int main(void) { int a[2] = {[3] = 1}; }\n", "[3]"),
    ];

    for (source, expected) in cases {
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let start = source.rfind(expected).unwrap();
        assert_eq!(
            (range.start_column, range.end_column),
            (start, start + expected.len()),
            "{expected}: {}",
            diagnostic.render()
        );
    }
}

#[test]
fn conversions_link_call_parameters_and_function_return_types() {
    let cases = [
        (
            "void f(int count) {} int main(void) { f(\"wrong\"); }\n",
            "int count",
        ),
        ("int *f(void) { return 1; } int main(void) {}\n", "int"),
    ];

    for (source, destination) in cases {
        let diagnostic = run_source("program.c", source).unwrap_err();
        let annotation = diagnostic.display_annotations().first().unwrap();
        let start = source.find(destination).unwrap();
        assert_eq!(annotation.id, "destination");
        assert_eq!(
            (annotation.range.start_column, annotation.range.end_column),
            (start, start + destination.len())
        );
    }
}

#[test]
fn c11_type_context_diagnostics_highlight_the_type_or_compound_literal() {
    let cases = [
        (
            "struct S; int main(void) { return _Alignof(struct S); }\n",
            "_Alignof(struct S)",
        ),
        (
            "struct S; int main(void) { return _Generic(1, struct S: 1, default: 2); }\n",
            "struct S",
        ),
        (
            "int main(void) { return _Generic(1, int: 1, signed int: 2); }\n",
            "signed int",
        ),
        (
            "struct S; int main(void) { (struct S){0}; }\n",
            "(struct S){0}",
        ),
    ];

    for (source, expected) in cases {
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let start = source.rfind(expected).unwrap();
        assert_eq!(
            (range.start_column, range.end_column),
            (start, start + expected.len()),
            "{expected}: {}",
            diagnostic.render()
        );
    }
}

#[test]
fn hexadecimal_integer_constants_with_e_digits_remain_integer_constants() {
    let source = "enum E { VALUE = 0xdead }; int main(void) { return VALUE != 0xdead; }\n";
    let result = run_source("program.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn c11_rejects_undeclared_old_style_parameters() {
    let source = "int function(value) { return value; }\nint main(void) { return 0; }\n";
    let diagnostic = run_source("program.c", source).unwrap_err();

    assert!(
        diagnostic
            .render()
            .contains("old-style parameter value is missing its declaration"),
        "{}",
        diagnostic.render()
    );
    let range = diagnostic.display_range().unwrap();
    let start = source.find("value").unwrap();
    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (0, start, 0, start + "value".len())
    );
}

#[test]
fn c11_still_accepts_declared_old_style_parameters() {
    let source = "int identity(value) int value; { return value; }\nint main(void) { return identity(7) != 7; }\n";
    let result = run_source("program.c", source).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn main_must_use_a_standard_parameter_list() {
    for source in [
        "int main(int value) { return value; }\n",
        "int main(value) int value; { return value; }\n",
    ] {
        let diagnostic = run_source("program.c", source).unwrap_err();
        assert!(
            diagnostic
                .render()
                .contains("this definition gives main 1 parameter"),
            "{}",
            diagnostic.render()
        );
        if source.starts_with("int main(int value)") {
            let start = source.find("int value").unwrap();
            let range = diagnostic.display_range().unwrap();
            assert_eq!(
                (
                    range.start_line,
                    range.start_column,
                    range.end_line,
                    range.end_column,
                ),
                (0, start, 0, start + "int value".len())
            );
        }
    }

    for source in [
        "int main(void) { return 0; }\n",
        "int main() { return 0; }\n",
        "int main(int argc, char **argv) { return argc < 1 || argv[argc] != 0; }\n",
    ] {
        let result = run_source("program.c", source).unwrap();
        assert_eq!(result.exit_status, 0, "{source}");
    }
}

#[test]
fn incomplete_function_header_is_not_misdiagnosed_as_an_unknown_type() {
    for source in ["int main()\n", "int function(value) int value;\n"] {
        let diagnostic = run_source("program.c", source).unwrap_err();
        assert!(
            diagnostic
                .render()
                .contains("is missing a body or semicolon"),
            "{}",
            diagnostic.render()
        );
        let range = diagnostic.display_range().unwrap();
        assert_eq!(range.start_line, 0);
        assert!(range.start_column > 0, "{}", diagnostic.render());
    }
}

#[test]
fn missing_main_points_to_the_end_instead_of_blaming_another_function() {
    let source = "int f(int value) { return value; }\n";
    let diagnostic = run_source("program.c", source).unwrap_err();

    assert!(
        diagnostic.render().contains("program does not define main"),
        "{}",
        diagnostic.render()
    );
    let range = diagnostic.display_range().unwrap();
    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (0, source.trim_end().len(), 0, source.trim_end().len())
    );
}

#[test]
fn prefix_unary_diagnostics_include_the_operator_in_their_range() {
    let cases = [
        ("int *b = 0; int *c = *b;", "*b"),
        ("int *b = 0; int *c = *(b);", "*(b)"),
        ("int ***b = 0; int **c = **b;", "**b"),
        ("int b; int c = &b;", "&b"),
        ("struct S { int x; } s; int x = +s;", "+s"),
        ("struct S { int x; } s; int x = -s;", "-s"),
        ("struct S { int x; } s; int x = !s;", "!s"),
        ("double d = 1; int x = ~d;", "~d"),
        ("int x = ++3;", "++3"),
        ("int x = --3;", "--3"),
    ];

    for (body, expression) in cases {
        let source = format!("int main(void) {{ {body} }}\n");
        let diagnostic = run_source("program.c", &source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let start = source.rfind(expression).unwrap();
        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, start, 0, start + expression.len()),
            "{expression}: {}",
            diagnostic.render()
        );
    }
}

#[test]
fn conversion_diagnostics_highlight_the_converted_expression_in_every_context() {
    let cases = [
        ("int main(void) { int **p; int *q = **p; }\n", "**p"),
        ("int main(void) { int **p; int *q; q = **p; }\n", "**p"),
        (
            "void f(int **value) {} int main(void) { int **p; f(*p); }\n",
            "*p",
        ),
        (
            "int **f(void) { int **p; return *p; } int main(void) { return 0; }\n",
            "*p",
        ),
    ];

    for (source, expression) in cases {
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let start = source.rfind(expression).unwrap();
        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, start, 0, start + expression.len()),
            "{expression}: {}",
            diagnostic.render()
        );
    }
}

#[test]
fn aggregate_initializer_diagnostics_identify_and_annotate_the_failing_member() {
    let cases = [
        (
            "struct S { int a; int b; }; int main(void) { struct S s = {0, \"x\"}; }\n",
            "b",
            "int b",
        ),
        (
            "struct S { int a; int b; }; int main(void) { struct S s = {.b = \"x\"}; }\n",
            "b",
            "int b",
        ),
        (
            "union U { int a; double d; }; int main(void) { union U u = {.a = \"x\"}; }\n",
            "a",
            "int a",
        ),
        (
            "struct I { int x; }; struct O { int lead; struct I inner; }; int main(void) { struct O o = {0, {\"x\"}}; }\n",
            "x",
            "int x",
        ),
    ];

    for (source, member_name, member_declaration) in cases {
        let diagnostic = run_source("program.c", source).unwrap_err();
        assert!(
            diagnostic
                .render()
                .contains(&format!("initializing member {member_name}")),
            "{}",
            diagnostic.render()
        );
        let annotation = diagnostic.display_annotations().first().unwrap();
        let member_start = source.find(member_declaration).unwrap() + "int ".len();
        assert_eq!(annotation.id, "destination");
        assert_eq!(
            (
                annotation.range.start_line,
                annotation.range.start_column,
                annotation.range.end_line,
                annotation.range.end_column,
            ),
            (0, member_start, 0, member_start + member_name.len()),
        );

        let json = cboxes_diagnostic_json(
            &diagnostic,
            &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
        );
        assert!(json.contains("\"annotations\":[{\"id\":\"destination\""));
    }
}

#[test]
fn unterminated_literals_highlight_from_the_quote_to_the_line_end() {
    let source = "int main(void) {\n  char a[] = \"hi\n}\n";
    let diagnostic = run_source("program.c", source).unwrap_err();
    let range = diagnostic.display_range().unwrap();

    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (1, 13, 1, 16)
    );
}

#[test]
fn preprocessor_fallback_ranges_cover_the_relevant_line() {
    let source = "  #define\nint main(void) { return 0; }\n";
    let diagnostic = run_source("program.c", source).unwrap_err();
    let range = diagnostic.display_range().unwrap();

    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (0, 2, 0, 9)
    );
}

#[test]
fn diagnostic_columns_are_mapped_back_across_macro_expansion() {
    let source = "#include <limits.h>\nint main(void){int x=INT_MIN; int y=-1; return x/y;}\n";
    let diagnostic = run_source("program.c", source).unwrap_err();
    let range = diagnostic.display_range().unwrap();
    let expression_start = source.lines().nth(1).unwrap().find("x/y").unwrap();

    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (1, expression_start, 1, expression_start + 3)
    );
}

#[test]
fn translation_unit_validation_errors_keep_their_source_range() {
    let source = "int f(void){return 1;} int f(void){return 2;} int main(void){return f();}\n";
    let diagnostic = run_source("program.c", source).unwrap_err();
    let range = diagnostic.display_range().unwrap();
    let definition_start = source.match_indices("int f").nth(1).unwrap().0;
    let start = definition_start + source[definition_start..].find('f').unwrap();
    let end = start + source[start..].find('}').unwrap() + 1;

    assert_eq!(
        (
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ),
        (0, start, 0, end)
    );
}

#[test]
fn virtual_project_links_sources_and_resolves_headers() {
    let files = vec![
        (
            PathBuf::from("program.c"),
            "#include \"answer.h\"\n#include <stdio.h>\nint main(void) {\n  int x = answer();\n  printf(\"%d\\n\", x);\n}\n"
                .to_owned(),
        ),
        (
            PathBuf::from("answer.c"),
            "#include \"answer.h\"\nint answer(void) { return 42; }\n".to_owned(),
        ),
        (
            PathBuf::from("answer.h"),
            "int answer(void);\n".to_owned(),
        ),
    ];
    let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();

    assert_eq!(result.stdout, "42\n");
    assert_eq!(result.state[0].name, "x");
    assert_eq!(result.state[0].value, "42");
    assert!(result.trace.iter().all(|event| event.file == "program.c"));
    assert!(result.trace.iter().any(|event| event.start_line == 3));
}

#[test]
fn virtual_project_normalizes_a_missing_final_newline_in_headers() {
    let files = vec![
        (
            PathBuf::from("program.c"),
            "#include \"answer.h\"\nint main(void) { return answer() != 42; }".to_owned(),
        ),
        (
            PathBuf::from("answer.c"),
            "#include \"answer.h\"\nint answer(void) { return 42; }".to_owned(),
        ),
        (PathBuf::from("answer.h"), "int answer(void);".to_owned()),
    ];

    let (files, _, _) = cboxes_prepare_virtual_sources(files, false).unwrap();
    assert!(files.iter().all(|(_, source)| source.ends_with('\n')));
    let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
    assert_eq!(result.exit_status, 0);
}

#[test]
fn browser_state_exposes_semantic_metadata_and_aliases() {
    let source = "int main(void) { double value = 0.1; double *p = &value; double **pp = &p; int items[2] = {1, 2}; }\n";
    let result = run_source("program.c", source).unwrap();
    let value = result
        .state
        .iter()
        .find(|item| item.name == "value")
        .unwrap();
    assert_eq!(value.type_info.kind, "floating");
    assert_eq!(value.type_info.size, Some(8));
    assert_eq!(value.display_value, "0.1");
    assert!(value.exact_value.starts_with("0.10000000000000000555"));
    assert_eq!(value.aliases, ["*p", "**pp"]);

    let items = result
        .state
        .iter()
        .find(|item| item.name == "items")
        .unwrap();
    assert_eq!(items.type_info.kind, "array");
    assert_eq!(items.type_info.array_shape, [2]);

    let json = cboxes_success_json(
        &result,
        &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
    );
    assert!(json.contains("\"typeInfo\""));
    assert!(json.contains("\"exactValue\""));
    assert!(json.contains("\"aliases\":[\"*p\",\"**pp\"]"));
}

#[test]
fn browser_state_uses_c_syntax_for_type_names() {
    let source = concat!(
        "int global;\n",
        "int *returns_pointer(void) { return &global; }\n",
        "int main(void) {\n",
        "  int (*function_pointer)(void) = main;\n",
        "  int *(*pointer_to_pointer_returning_function)(void) = returns_pointer;\n",
        "  int values[3] = {0};\n",
        "  int (*pointer_to_array)[3] = &values;\n",
        "  int *array_of_pointers[3] = {0};\n",
        "  int (*pointer_to_incomplete_array)[];\n",
        "  const int *pointer_to_const = 0;\n",
        "  int * const const_pointer = 0;\n",
        "  _Bool flag = 0;\n",
        "}\n",
    );
    let result = run_source("program.c", source).unwrap();
    let type_of = |name: &str| {
        result
            .state
            .iter()
            .find(|item| item.name == name)
            .map(|item| item.ty.as_str())
    };
    let help_for = |name: &str| {
        result
            .state
            .iter()
            .find(|item| item.name == name)
            .and_then(|item| item.type_info.help.as_deref())
    };
    let info_for = |name: &str| {
        &result
            .state
            .iter()
            .find(|item| item.name == name)
            .unwrap()
            .type_info
    };

    assert_eq!(type_of("function_pointer"), Some("int (*)(void)"));
    assert_eq!(
        help_for("function_pointer"),
        Some("pointer to function taking no arguments and returning int")
    );
    let function_pointer_tree = info_for("function_pointer").help_tree.as_ref().unwrap();
    assert_eq!(function_pointer_tree.kind, "pointer");
    assert_eq!(function_pointer_tree.children[0].relation, "to");
    assert_eq!(function_pointer_tree.children[0].node.kind, "function");
    assert_eq!(info_for("function_pointer").help_type_names, ["int"]);
    assert_eq!(
        type_of("pointer_to_pointer_returning_function"),
        Some("int*(*)(void)")
    );
    assert_eq!(
        help_for("pointer_to_pointer_returning_function"),
        Some("pointer to function taking no arguments and returning pointer to int")
    );
    assert_eq!(type_of("pointer_to_array"), Some("int (*)[3]"));
    assert_eq!(
        help_for("pointer_to_array"),
        Some("pointer to array of 3 ints")
    );
    assert_eq!(type_of("array_of_pointers"), Some("int*[3]"));
    assert_eq!(
        help_for("array_of_pointers"),
        Some("array of 3 pointers to int")
    );
    assert_eq!(type_of("pointer_to_incomplete_array"), Some("int (*)[]"));
    assert_eq!(
        help_for("pointer_to_incomplete_array"),
        Some("pointer to array of unknown length containing ints")
    );
    let incomplete_array_tree = info_for("pointer_to_incomplete_array")
        .help_tree
        .as_ref()
        .unwrap();
    assert_eq!(incomplete_array_tree.kind, "pointer");
    assert_eq!(incomplete_array_tree.children[0].node.kind, "array");
    assert_eq!(
        incomplete_array_tree.children[0].node.label,
        "array of unknown length"
    );
    assert_eq!(info_for("pointer_to_incomplete_array").size, Some(8));
    assert_eq!(type_of("pointer_to_const"), Some("const int*"));
    assert_eq!(help_for("pointer_to_const"), None);
    assert!(info_for("pointer_to_const").help_tree.is_none());
    assert!(info_for("pointer_to_const").help_type_names.is_empty());
    assert_eq!(type_of("const_pointer"), Some("int* const"));
    assert_eq!(help_for("const_pointer"), None);
    assert!(info_for("const_pointer").help_tree.is_none());
    assert_eq!(type_of("flag"), Some("_Bool"));
    assert_eq!(help_for("flag"), None);
}

#[test]
fn browser_state_displays_struct_and_union_values() {
    let source = format!(
        "{}\n",
        r#"
        struct Point { int x; double y; };
        struct Shape { struct Point point; int sides[2]; };
        union Number { int integer; double decimal; };

        int main(void) {
            struct Shape shape = {{3, 2.5}, {4, 5}};
            union Number number = {.decimal = 1.25};
        }
    "#
        .trim()
    );
    let result = run_source("program.c", source).unwrap();

    let shape = result
        .state
        .iter()
        .find(|item| item.name == "shape")
        .unwrap();
    assert_eq!(shape.value, "");
    assert_eq!(shape.type_info.kind, "aggregate");
    assert_eq!(shape.aggregate_kind.as_deref(), Some("struct"));
    let state_value = |name: &str| {
        result
            .state
            .iter()
            .find(|item| item.name == name)
            .map(|item| item.value.as_str())
    };
    assert_eq!(state_value("shape.point.x"), Some("3"));
    assert_eq!(state_value("shape.point.y"), Some("2.5"));
    assert_eq!(state_value("shape.sides[0]"), Some("4"));
    assert_eq!(state_value("shape.sides[1]"), Some("5"));
    let point = result
        .state
        .iter()
        .find(|item| item.name == "shape.point")
        .unwrap();
    assert_eq!(point.aggregate_root.as_deref(), Some("shape"));
    assert_eq!(point.aggregate_path, ["point"]);
    let point_x = result
        .state
        .iter()
        .find(|item| item.name == "shape.point.x")
        .unwrap();
    let point_y = result
        .state
        .iter()
        .find(|item| item.name == "shape.point.y")
        .unwrap();
    assert_eq!(point_x.address, shape.address);
    assert_eq!(point_y.address, shape.address.map(|address| address + 8));

    let number = result
        .state
        .iter()
        .find(|item| item.name == "number")
        .unwrap();
    assert_eq!(number.value, "");
    assert_eq!(number.type_info.kind, "aggregate");
    assert_eq!(number.aggregate_kind.as_deref(), Some("union"));
    assert_eq!(state_value("number.decimal"), Some("1.25"));
    assert!(
        result
            .state
            .iter()
            .all(|item| item.name != "number.integer")
    );
}

#[test]
fn browser_state_does_not_invent_a_union_member_after_byte_copying() {
    let source = concat!(
        "#include <string.h>\n",
        "union Number { int integer; double decimal; };\n",
        "int main(void) {\n",
        "  union Number source = {.integer = 7};\n",
        "  union Number copied;\n",
        "  memcpy(&copied, &source, sizeof copied);\n",
        "}\n",
    );
    let result = run_source("program.c", source).unwrap();
    let copied = result
        .state
        .iter()
        .find(|item| item.name == "copied")
        .unwrap();

    assert_eq!(copied.value, "active member unknown");
    assert!(
        result
            .state
            .iter()
            .all(|item| !item.name.starts_with("copied."))
    );
}

#[test]
fn browser_state_includes_visible_program_globals() {
    let source = format!(
        "{}\n",
        r#"
        #include <stdio.h>
        struct Counter { int current; int limit; };
        int total = 3;
        static int zeroed;
        struct Counter counter = {1, 10};

        int main(void) {
            int local = 4;
            total = 8;
        }
    "#
        .trim()
    );
    let result = run_source("program.c", source).unwrap();

    let state_value = |name: &str| {
        result
            .state
            .iter()
            .find(|item| item.name == name)
            .map(|item| item.value.as_str())
    };
    assert_eq!(state_value("total"), Some("8"));
    assert_eq!(state_value("zeroed"), Some("0"));
    assert_eq!(state_value("counter"), Some(""));
    assert_eq!(state_value("counter.current"), Some("1"));
    assert_eq!(state_value("counter.limit"), Some("10"));
    assert_eq!(state_value("local"), Some("4"));
    assert!(
        result
            .state
            .iter()
            .all(|item| !matches!(item.name.as_str(), "stdin" | "stdout" | "stderr"))
    );
}

#[test]
fn browser_state_uses_locals_instead_of_shadowed_globals() {
    let source = "int value = 1; int main(void) { int value = 2; }\n";
    let result = run_source("program.c", source).unwrap();
    let values = result
        .state
        .iter()
        .filter(|item| item.name == "value")
        .collect::<Vec<_>>();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value, "2");
}

#[test]
fn browser_state_includes_cross_file_globals_visible_to_main() {
    let files = vec![
        (
            PathBuf::from("program.c"),
            "extern int answer; int main(void) { answer = 42; }\n".to_owned(),
        ),
        (
            PathBuf::from("answer.c"),
            "int answer = 7; static int hidden = 9;\n".to_owned(),
        ),
    ];
    let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();

    assert_eq!(
        result
            .state
            .iter()
            .find(|item| item.name == "answer")
            .map(|item| item.value.as_str()),
        Some("42")
    );
    assert!(result.state.iter().all(|item| item.name != "hidden"));
}

#[test]
fn implicit_main_trace_excludes_generated_lines() {
    let original = "int a = 1;\na = 2;";
    let (files, source_display, implicit_main) = cboxes_prepare_virtual_sources(
        vec![(PathBuf::from("program.c"), original.to_owned())],
        true,
    )
    .unwrap();
    let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
    let json = cboxes_success_json(&result, &source_display);

    assert!(implicit_main.applied);
    assert!(json.contains("\"startLine\":0"));
    assert!(json.contains("\"startLine\":1"));
    assert!(!json.contains("\"startLine\":2"));
    assert!(json.contains("\"mainClose\":null"));
}

#[test]
fn branch_trace_reports_the_exact_skipped_source_range() {
    let original = "if (0) {\n  int first = 1;\n} else if (0) {\n  int second = 2;\n} else {\n  int taken = 3;\n}\n";
    let (files, source_display, implicit_main) = cboxes_prepare_virtual_sources(
        vec![(PathBuf::from("program.c"), original.to_owned())],
        true,
    )
    .unwrap();
    let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
    let json = cboxes_success_json(&result, &source_display);

    assert!(implicit_main.applied);
    assert!(json.contains("\"kind\":\"branch\""));
    assert!(json.contains(
        "\"skippedRange\":{\"file\":\"program.c\",\"startLine\":0,\"startColumn\":0,\"endLine\":2,\"endColumn\":1}"
    ));
    assert!(json.contains(
        "\"skippedRange\":{\"file\":\"program.c\",\"startLine\":2,\"startColumn\":2,\"endLine\":4,\"endColumn\":1}"
    ));
}

#[test]
fn explicit_main_reports_its_closing_brace_as_the_program_end() {
    let source = "int main(void) {\n  int a = 1;\n}\n";
    let result = run_source("program.c", source).unwrap();
    let source_display =
        HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
    let json = cboxes_success_json(&result, &source_display);

    assert_eq!(result.trace.len(), 1);
    assert_eq!(result.trace[0].start_line, 1);
    assert_eq!(result.main_close.file, "program.c");
    assert_eq!(result.main_close.line, 2);
    assert!(json.contains("\"mainClose\":{\"file\":\"program.c\",\"line\":2}"));
}

#[test]
fn execution_step_limit_returns_the_trace_before_an_infinite_loop() {
    let source = "int main(void) {\n  int before = 1;\n  while (1) {\n    before++;\n  }\n  int after = 2;\n}\n";
    let result = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(40),
            execution_trace_following_limit: 2,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let source_display =
        HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
    let json = cboxes_success_json(&result, &source_display);

    let execution_limit = result.execution_limit.as_ref().unwrap();
    assert_eq!(execution_limit.start_line, 2);
    assert_eq!(execution_limit.trace_position, 1);
    assert_eq!(result.trace.len(), execution_limit.trace_position + 2);
    assert!(result.trace.iter().all(|event| event.start_line < 5));
    assert_eq!(
        result.state.first().map(|item| (&item.name, &item.value)),
        result
            .trace
            .get(execution_limit.trace_position - 1)
            .and_then(|event| event.state.first())
            .map(|item| (&item.name, &item.value)),
    );
    assert!(
        json.contains(
            "\"executionLimit\":{\"file\":\"program.c\",\"startLine\":2,\"endLine\":2,\"tracePosition\":1}"
        )
    );

    let expanded = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(80),
            execution_trace_following_limit: 4,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let expanded_limit = expanded.execution_limit.as_ref().unwrap();
    assert_eq!(
        expanded_limit.trace_position,
        execution_limit.trace_position
    );
    assert_eq!(expanded.trace.len(), expanded_limit.trace_position + 4);
}

#[test]
fn execution_step_limit_immediately_recognizes_a_constant_empty_loop() {
    let source = "int main(void) {\n  while (1) {\n    ;\n  }\n}\n";
    let result = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(10_000),
            execution_trace_following_limit: 6,
            ..RunOptions::default()
        },
    )
    .unwrap();

    let execution_limit = result.execution_limit.as_ref().unwrap();
    assert_eq!(execution_limit.start_line, 1);
    assert_eq!(execution_limit.trace_position, 1);
    assert_eq!(result.trace.len(), 1);
}

#[test]
fn execution_step_limit_rewinds_before_a_goto_cycle() {
    let source = "int main(void) {\n  int before = 1;\nrepeat:\n  before++;\n  goto repeat;\n  int after = 2;\n}\n";
    let result = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(600),
            ..RunOptions::default()
        },
    )
    .unwrap();

    let execution_limit = result.execution_limit.as_ref().unwrap();
    assert_eq!(execution_limit.start_line, 3);
    assert_eq!(execution_limit.trace_position, 1);
    assert!(result.trace.len() > execution_limit.trace_position);
    assert_eq!(result.state[0].name, "before");
    assert_eq!(result.state[0].value, "1");
}

#[test]
fn execution_step_limit_preserves_trace_when_no_location_repeats() {
    let mut source = "int main(void) {\n  int value = 0;\n".to_owned();
    for _ in 0..50 {
        source.push_str("  value++;\n");
    }
    source.push_str("}\n");
    let result = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(20),
            ..RunOptions::default()
        },
    )
    .unwrap();

    let execution_limit = result.execution_limit.as_ref().unwrap();
    assert_eq!(result.state[0].name, "value");
    assert!(result.state[0].value.parse::<usize>().unwrap() > 0);
    assert!(execution_limit.start_line > result.trace.last().unwrap().end_line);
    assert_eq!(execution_limit.trace_position, result.trace.len());
}

#[test]
fn blocked_stdin_location_is_exposed_to_the_browser() {
    let source =
        "#include <stdio.h>\nint main(void) {\n  int before = 1;\n  int ch = getchar();\n}\n";
    let result = run_source("program.c", source).unwrap();
    let source_display =
        HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
    let json = cboxes_success_json(&result, &source_display);

    assert!(result.blocked.is_some());
    assert!(json.contains(
        "\"blocked\":{\"file\":\"program.c\",\"startLine\":3,\"endLine\":3,\"function\":\"getchar\""
    ));
    assert!(json.contains("\"name\":\"before\""));
}

#[test]
fn implicit_main_wraps_the_entire_entry_source() {
    let original = "int a = 1;\na += 2;";
    let (files, _, implicit_main) = cboxes_prepare_virtual_sources(
        vec![(PathBuf::from("program.c"), original.to_owned())],
        true,
    )
    .unwrap();

    assert!(implicit_main.applied);
    assert_eq!(
        files[0].1,
        "int main(void) {\nint a = 1;\na += 2;\nreturn 0;\n}\n"
    );
}

#[test]
fn implicit_main_wraps_header_includes_too() {
    let original = "#include <stdio.h>\nprintf(\"hello\\n\");";
    let (files, _, implicit_main) = cboxes_prepare_virtual_sources(
        vec![(PathBuf::from("program.c"), original.to_owned())],
        true,
    )
    .unwrap();

    assert!(implicit_main.applied);
    assert!(implicit_main.notice.is_none());
    assert!(
        files[0]
            .1
            .starts_with("int main(void) {\n#include <stdio.h>")
    );
}

#[test]
fn legacy_implicit_main_detection_ignores_comments_and_literals() {
    let source = r#"#include <stdio.h>
        // main() is introduced later in the course.
        printf("main() is where programs start\n");
    "#;
    assert!(!cboxes_has_explicit_main(source));
    let result = run_source("program.c", cboxes_wrap_implicit_main(source)).unwrap();
    assert_eq!(result.stdout, "main() is where programs start\n");

    assert!(!cboxes_has_explicit_main("/* int main(void) {} */\n"));
    assert!(!cboxes_has_explicit_main(
        "char text[] = \"main (void)\";\n"
    ));
    assert!(cboxes_has_explicit_main(
        "int ma\\\nin /* comment */ (void) { return 0; }\n"
    ));
}

#[test]
fn implicit_main_stays_on_and_suggests_the_toggle_for_an_existing_main() {
    let files = vec![(
        PathBuf::from("program.c"),
        "int main(void) { return 0; }".to_owned(),
    )];
    let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

    assert!(run.result.is_err());
    assert!(run.implicit_main.applied);
    assert!(
        run.implicit_main
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("Try turning off Implicit main"))
    );
}

#[test]
fn implicit_main_does_not_suggest_the_toggle_when_off_still_errors() {
    let files = vec![(
        PathBuf::from("program.c"),
        "int answer(void) { return 42; }".to_owned(),
    )];
    let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

    assert!(run.result.is_err());
    assert!(run.implicit_main.applied);
    assert!(run.implicit_main.notice.is_none());
}

#[test]
fn implicit_main_stays_on_and_suggests_the_toggle_for_header_programs() {
    let files = vec![(
        PathBuf::from("program.c"),
        "#include <stdio.h>\nint main(void) { printf(\"hello\\n\"); }".to_owned(),
    )];
    let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

    assert!(run.result.is_err());
    assert!(run.implicit_main.applied);
    assert!(
        run.implicit_main
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("Try turning off Implicit main"))
    );
}

#[test]
fn tutorial_implicit_main_keeps_includes_outside_the_wrapper() {
    let source = cboxes_wrap_implicit_main(
        "#include <stdio.h>\nprintf(\"tutorial wrapper still works\\n\");",
    );
    let result = run_source("program.c", source).unwrap();

    assert_eq!(result.stdout, "tutorial wrapper still works\n");
}

#[test]
#[ignore = "developer-only external performance harness"]
fn external_performance_harness() {
    let files = std::env::var("CBOXES_PROFILE_FILES")
        .expect("set CBOXES_PROFILE_FILES to colon-separated C source paths")
        .split(':')
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let include_dirs = std::env::var("CBOXES_PROFILE_INCLUDE_DIRS")
        .unwrap_or_default()
        .split(':')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect();
    let execution_step_limit = std::env::var("CBOXES_PROFILE_STEP_LIMIT")
        .ok()
        .map(|limit| limit.parse().expect("step limit must be an integer"));
    let allocation_limit_bytes = std::env::var("CBOXES_PROFILE_ALLOCATION_LIMIT_BYTES")
        .ok()
        .map(|limit| limit.parse().expect("allocation limit must be an integer"));
    let optimizing_precomputations = std::env::var("CBOXES_PROFILE_PRECOMPUTE")
        .map(|value| value != "0")
        .unwrap_or(true);
    let options = RunOptions {
        include_dirs,
        capture_visualization: false,
        execution_step_limit,
        allocation_limit_bytes,
        optimizing_precomputations,
        ..RunOptions::default()
    };
    let started = std::time::Instant::now();
    let result = run_files_with_options(files, &options);
    let elapsed = started.elapsed();
    match result {
        Ok(result) => eprintln!(
            "CBOXES_PROFILE elapsed_us={} exit_status={} stdout_bytes={}",
            elapsed.as_micros(),
            result.exit_status,
            result.stdout.len()
        ),
        Err(error) => eprintln!(
            "CBOXES_PROFILE elapsed_us={} error={}",
            elapsed.as_micros(),
            error.render()
        ),
    }
}
