use super::*;

#[test]
fn beginner_loop_mistakes_are_detected_without_spending_the_step_budget() {
    let bodies = [
        "int i = 0; while (i < 10) {}",
        "int i = 0; while (i < 10); i++;",
        "int i = 0, j = 0; while (i < 10) { j++; }",
        "int i = 0; while (i < 10) { i = 0; i++; }",
        "int i = 0; while (i = 1) { i++; }",
        "int i = 0; while (i < 10) { i = 1 - i; }",
        "int i = 0; while (i < 10) { if (i == 5) break; i = 1 - i; }",
        "for (int i = 0; i < 10;) {}",
        "int j = 0; for (int i = 0; i < 10; j++) {}",
        "for (;;) { continue; }",
        "int i = 0; do { i = 0; } while (i < 10);",
        "int i = 0; do { continue; } while (i < 10);",
        "int i = 0; while (i < 10) { printf(\"%d\", i); }",
        "while (1) { puts(\"hello\"); }",
        "double x = 1.0; while (x > 0) { x = 1.0; }",
        "int i = 0; while (i < 3) { int unused = i; i = 0; }",
    ];
    for body in bodies {
        let source = format!("#include <stdio.h>\nint main(void) {{\n{body}\nreturn 7;\n}}\n");
        let output = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 8,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(
            limit.executed_steps < 500,
            "{} steps: {body}",
            limit.executed_steps
        );
        assert_ne!(output.exit_status, 7);
    }
}

#[test]
fn loop_cycle_detection_preserves_finite_progress_and_hidden_state() {
    let bodies = [
        "int i = 0; while (i < 2000) i++;",
        "for (int i = 0; i < 2000; i++) {}",
        "int i = 0; do { i++; } while (i < 2000);",
        "int i = 0, j = 0; while (i < 10) { if (++j % 100 == 0) i++; }",
        "int i = 0, j = 0; while (i < 10) { i = j / 100; j++; }",
        "int i = 0, j = 0; while (i < 10) { j++; j == 100 && (i = 10); }",
        "int i = 0, j = 0; while (i < 10) { j++; j < 100 || (i = 10); }",
        "int i = 0, j = 0; while (i < 10) { j++; j < 100 ? 0 : (i = 10); }",
        "int i = 0, j = 0; while (i < 10) { if (++j == 100) break; }",
        "int i = 0, j = 0; while (i < 10) { if (++j == 100) return 7; }",
        "int i = 0; while (i < 10) { int i = 100; break; }",
        "int i = 0; while (i < 10) { int next = i + 1; i = next; }",
        "int i = 0; while (i < 10) { int *p = &i; (*p)++; }",
        "int i = 0; while (i < 10) { i = next(); }",
        "while (next() < 10) {}",
        "int i = 0; while (i < 10) { scanf(\"%d\", &i); }",
        "int i = 0; while (i < 10) { printf(\"0123456789%n\", &i); }",
        "volatile int i = 0; while (i < 10) i++;",
        "int i = 0; while (i++ < 10) {}",
        "int i = 0; do {} while (i++ < 10);",
        "for (int i = 0; i < 10;) { if (++i < 10) continue; }",
    ];
    for body in bodies {
        let source = format!(
            "#include <stdio.h>\nint next(void) {{ static int n; return ++n; }}\nint main(void) {{\n{body}\nreturn 7;\n}}\n"
        );
        let output = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                stdin: "0 0 0 0 0 10\n".to_owned(),
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 4,
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn unsupported_loop_analysis_retains_the_step_limit_fallback() {
    let source = "#include <stdlib.h>\nint main(void) { while(rand() >= 0) {} }\n";
    let output = run_source_with_options(
        "program.c",
        source,
        &RunOptions {
            execution_step_limit: Some(1000),
            ..RunOptions::default()
        },
    )
    .unwrap();
    let limit = output.execution_limit.unwrap();
    assert_eq!(limit.reason, "step-limit");
    assert_eq!(limit.executed_steps, 1000);
}

#[test]
fn loop_counter_progress_detects_wrong_direction_and_unreachable_targets() {
    for body in [
        "int i = 0; while (i < 10) i--;",
        "for (int i = 0; i <= 10; --i) {}",
        "int i = 10; while (i > 0) ++i;",
        "int i = 10; do { i += 2; } while (i >= 0);",
        "int i = 0; while (10 > i) { i = i - 1; }",
        "int i = 1; while (i != 10) { i += 2; }",
        "int i = 1; for (; i != 10; i = 2 + i) {}",
        "unsigned i = 1; while (i != 10u) { i += 2u; }",
        "unsigned i = 0; while (i >= 0u) { i++; }",
        "unsigned i = 0; while (i <= 4294967295u) { i++; }",
        "int i = 0; while(i < 10) { printf(\"count %d\\n\", i); i--; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <stdio.h>\nint main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 8,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(
            limit.executed_steps < 500,
            "{} steps: {body}",
            limit.executed_steps
        );
    }
}

#[test]
fn loop_progress_respects_wraparound_breaks_and_conversions() {
    for body in [
        "unsigned i = 0; while(i < 10u) { i--; }",
        "unsigned i = 4294967295u; while(i > 10u) { i++; }",
        "unsigned i = 4294967295u; while(i != 0u) { i++; }",
        "unsigned i = 0; while(i != 4294967295u) { i--; }",
        "int i = 0; while(i < 10u) { i--; }",
        "int i = 1; while(i != 11) { i += 2; }",
        "int i = 10; while(i > 0) { i--; }",
        "int i = 0; while(i < 10) { if (i < -5) break; i--; }",
        "int i = 0; while(i < 10) { i--; if (i < -5) return 7; }",
        "int i = -2147483647; while(i < 10) { i -= 2L; }",
        "int i = 0; while(i < 10) { i = 10; }",
        "int i = 0; do { i++; } while(i < 0);",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 8,
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn detected_loop_rewinds_to_its_own_start_after_a_long_finite_loop() {
    let output = run_source_with_options(
        "program.c",
        "int main(void) {\nint i=0;\nwhile(i < 100) i++;\nwhile(i > 0) i++;\n}\n",
        &RunOptions {
            execution_step_limit: Some(100_000),
            execution_trace_following_limit: 4,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let limit = output.execution_limit.unwrap();
    assert_eq!(limit.start_line, 3);
    assert_eq!(limit.reason, "unreachable-loop-exit");
    assert_eq!(
        output
            .state
            .iter()
            .find(|item| item.name == "i")
            .unwrap()
            .value,
        "100"
    );
}

#[test]
fn loop_detection_follows_simple_helper_functions_and_enum_bounds() {
    for source in [
        "int always(void) { return 1; } int main(void) { while(always()) {} }",
        "int same(int n) { int copy=n; return copy; } int main(void) { int i=0; while(same(i)<10) {} }",
        "int same(int n) { return n; } int wrapper(int n) { return same(n); } int main(void) { int i=0; while(wrapper(i)<10) {} }",
        "#include <stdio.h>\nvoid show(int n) { printf(\"counter %d\\n\", n); } int main(void) { int i=0; while(i<10) show(i); }",
        "int i; void reset(void) { i=0; } int main(void) { while(i<10) reset(); }",
        "enum { LIMIT=10 }; int main(void) { int i=0; while(i<LIMIT) {} }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("{source}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {source}"));
        assert_eq!(limit.reason, "repeating-loop", "{source}");
        assert!(limit.executed_steps < 500, "{source}");
    }
}

#[test]
fn loop_helper_summaries_track_globals_independently_of_caller_shadowing() {
    for source in [
        "int i; void advance(void) { i++; } int main(void) { while(i<10) advance(); return 7; }",
        "int n; int next(void) { return ++n; } int main(void) { while(next()<10) {} return 7; }",
        "int n; int next(void) { return ++n; } int main(void) { int n=0,i=0; while(i<10) { i=next(); } return n+7; }",
        "int next(int n) { return n+1; } int main(void) { int i=0; while(i<10) { i=next(i); } return 7; }",
        "int n; int next(void) { {int n=1;n++;} return ++n; } int main(void) { while(next()<10) {} return 7; }",
        "int next(void) { static int n; return ++n; } int main(void) { while(next()<10) {} return 7; }",
        "int done; void advance(void) { done++; } int main(void) { int (*f)(void)=0; while(done<10) advance(); return 7; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("{source}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {source}");
        assert_eq!(output.exit_status, 7, "{source}");
    }
}

#[test]
fn loop_progress_handles_condition_updates_and_invariant_variable_bounds() {
    for body in [
        "int i=-1; while(i--) {}",
        "int i=0; while(--i) {}",
        "int i=1; while(i++) {}",
        "int i=-1; while(i) i--;",
        "int i=0, limit=10; while(i < limit) i--;",
        "int i=0; const int limit=10; for(;i<limit;i--) {}",
        "unsigned i=1; while(i++ != 10u) { i++; }",
        "int i=1; while(i++ != 10) { i++; }",
        "int i=0; do {} while(--i);",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 500, "{body}");
    }
    for body in [
        "int i=10; while(i--) {}",
        "int i=10; while(--i) {}",
        "int i=-10; while(++i) {}",
        "int i=-10; while(i++) {}",
        "int i=0; while(i++) {}",
        "int i=10; while(i) i--;",
        "int i=0, limit=10; while(i<limit) limit--;",
        "unsigned i=0; while(i++ != 10u) { i++; }",
        "int i=0; while(i++ != 10) { i++; }",
        "int i=0; do {} while(i++ < 10);",
        "unsigned i=0; do {} while(i--);",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn loop_detection_tracks_static_helper_state() {
    for source in [
        "int same(void) { static int n=1; return n; } int main(void) { while(same()) {} }",
        "int toggle(void) { static int n; n=1-n; return n; } int main(void) { while(toggle()<2) {} }",
        "int same(void) { static int n; n=0; return n; } int main(void) { while(same()<10) {} }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("{source}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {source}"));
        assert_eq!(limit.reason, "repeating-loop", "{source}");
        assert!(limit.executed_steps < 500, "{source}");
    }
}

#[test]
fn loop_detection_handles_string_array_and_structure_reads() {
    for body in [
        "char text[]=\"hello\"; int i=0; while(text[i]) {}",
        "const char *text=\"hello\"; int i=0; while(text[i]) { printf(\"%c\",text[i]); }",
        "char text[]=\"hello\"; char *p=text; while(*p) {}",
        "char text[]=\"hello\"; char *p=text; while(*p) { p=p; }",
        "char text[]=\"hello\"; int i=0; while(i<strlen(text)) {}",
        "char text[]=\"hello\"; while(strcmp(text,\"done\")) {}",
        "int a[3]={1,2,0}; int i=0; while(a[i]) {}",
        "struct S {int done;}; struct S s={0}; while(!s.done) {}",
        "struct S {int done;}; struct S s={0}; struct S *p=&s; while(!p->done) {}",
        "int i=0; while(i<10) { switch(i) { case 5: return 7; default: break; } }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!(
                "#include <stdio.h>\n#include <string.h>\nint main(void) {{ {body} return 7; }}\n"
            ),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 500, "{body}");
    }
    for body in [
        "char text[]=\"hello\"; int i=0; while(text[i]) i++;",
        "const char *text=\"hello\"; int i=0; while(text[i]) i++;",
        "char text[]=\"hello\"; char *p=text; while(*p) p++;",
        "char text[]=\"hello\"; int i=0; while(i<strlen(text)) i++;",
        "int a[3]={1,2,0}; int i=0; while(a[i]) i++;",
        "int i=0; while(i<10) { switch(i) { case 5: return 7; default: i++; } }",
        "int i=0; while(i<10) { switch(i) { case 5: i=10; break; default: i++; } }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <string.h>\nint main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn loop_detection_handles_repeated_array_and_pointer_writes() {
    for body in [
        "int a[3]={0}; int i=0; while(i<3) a[i]=1;",
        "int i=0; int *p=&i; while(i<10) *p=0;",
        "struct S {int done;}; struct S s={0}; while(!s.done) s.done=0;",
        "int i=0; while(i<10) reset(&i);",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("void reset(int *p) {{ *p=0; }}\nint main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 6,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 500, "{body}");
    }
    for body in [
        "int a[3]={0}; int i=0; while(i<3) a[i++]=1;",
        "int i=0; int *p=&i; while(i<10) ++*p;",
        "int i=0,j=0; int *p=&i; while(i<10) *p=j++/100;",
        "int i=0,j=0; int *p=&i; while(i<10) *p=(++j==100 ? 10:0);",
        "struct S {int done;}; struct S s={0}; while(s.done<10) s.done++;",
        "int i=0; while(i<10) advance(&i);",
        "int i=0; while(i<10) delayed(&i);",
    ] {
        let output = run_source_with_options("program.c", format!("void advance(int *p) {{ (*p)++; }} void delayed(int *p) {{ static int n; *p=(++n==100 ? 10:0); }}\nint main(void) {{ {body} return 7; }}\n"), &RunOptions {
            execution_step_limit: Some(100_000), execution_trace_following_limit: 6,
            ..RunOptions::default()
        }).unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn loop_detection_recognizes_input_that_cannot_make_progress() {
    for (body, stdin) in [
        ("int n=0; while(scanf(\"%d\", &n)!=1) {}", "wrong\n"),
        ("int n; while(scanf(\"%d\", &n)!=1) {}", "wrong\n"),
        ("int n=0; while(n<10) { scanf(\"%d\", &n); }", "0 0 wrong\n"),
        ("int c=0; while((c=getchar())!='\\n') {}", "abc\u{4}"),
        (
            "int n=0; char text[]=\"7\"; while(sscanf(text,\"%d\",&n)==1) {}",
            "",
        ),
        ("int c=0; while((c=readnext())!='\\n') {}", "abc\u{4}"),
    ] {
        let output = run_source_with_options("program.c", format!("#include <stdio.h>\nint readnext(void) {{ return getchar(); }}\nint main(void) {{ {body} return 7; }}\n"), &RunOptions {
            stdin: stdin.to_owned(), execution_step_limit: Some(100_000), execution_trace_following_limit: 6,
            ..RunOptions::default()
        }).unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("missed: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 500, "{body}");
    }
    for (body, stdin) in [
        ("int n=0; while(n<10) scanf(\"%d\",&n);", "0 0 0 0 0 10\n"),
        ("int c=0; while((c=getchar())!='\\n') {}", "aaaaaaa\n"),
        ("int c=0; while((c=readnext())!='\\n') {}", "aaaaaaa\n"),
        (
            "int n=0; while(scanf(\"%d\", &n)!=1) { getchar(); }",
            "xxx7\n",
        ),
        ("int c=0; while((c=getchar())!=EOF) {}", "aaaaaaa\u{4}"),
    ] {
        let output = run_source_with_options("program.c", format!("#include <stdio.h>\nint readnext(void) {{ return getchar(); }}\nint main(void) {{ {body} return 7; }}\n"), &RunOptions {
            stdin: stdin.to_owned(), execution_step_limit: Some(100_000), execution_trace_following_limit: 6,
            ..RunOptions::default()
        }).unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
    let waiting = run_source_with_options(
        "program.c",
        "#include <stdio.h>\nint main(void) { while(getchar()!='\\n') {} }\n",
        &RunOptions {
            stdin: "aaa".to_owned(),
            execution_step_limit: Some(100_000),
            ..RunOptions::default()
        },
    )
    .unwrap();
    assert!(waiting.blocked.is_some());
    assert!(waiting.execution_limit.is_none());
}

#[test]
fn loop_progress_preserves_finite_endpoints_in_a_generated_integer_matrix() {
    let mut checked = 0;
    for unsigned in [false, true] {
        let starts: &[i128] = if unsigned {
            &[0, 1, 2, u32::MAX as i128 - 1, u32::MAX as i128]
        } else {
            &[-2, -1, 0, 1, 2, i32::MIN as i128 + 2, i32::MAX as i128 - 2]
        };
        let bounds: &[i128] = if unsigned {
            &[0, 2, u32::MAX as i128]
        } else {
            &[-2, 0, 2]
        };
        for &start in starts {
            for &bound in bounds {
                for stride in [-2i128, -1, 0, 1, 2] {
                    for offset in [-2, 0, 2] {
                        for op in ["<", "<=", ">", ">=", "!="] {
                            for post_delta in [-1i128, 0, 1] {
                                let advance = |value: i128, delta: i128| {
                                    let result = value + delta;
                                    if unsigned {
                                        Some(result.rem_euclid(1i128 << 32))
                                    } else {
                                        (i32::MIN as i128..=i32::MAX as i128)
                                            .contains(&result)
                                            .then_some(result)
                                    }
                                };
                                let mut current = start;
                                let mut endpoint = None;
                                for _ in 0..128 {
                                    let Some(compared) = advance(current, offset) else {
                                        break;
                                    };
                                    let condition = match op {
                                        "<" => compared < bound,
                                        "<=" => compared <= bound,
                                        ">" => compared > bound,
                                        ">=" => compared >= bound,
                                        _ => compared != bound,
                                    };
                                    let Some(after_condition) = advance(current, post_delta) else {
                                        break;
                                    };
                                    current = after_condition;
                                    if !condition {
                                        endpoint = Some(current);
                                        break;
                                    }
                                    let Some(after_body) = advance(current, stride) else {
                                        break;
                                    };
                                    current = after_body;
                                }
                                let Some(endpoint) = endpoint else {
                                    continue;
                                };
                                let ty = if unsigned { "unsigned" } else { "int" };
                                let suffix = if unsigned { "u" } else { "" };
                                let counter = match post_delta {
                                    -1 => "i--",
                                    1 => "i++",
                                    _ => "i",
                                };
                                let source = format!(
                                    "int main(void) {{ {ty} i={start}{suffix}; while(({counter} + ({offset}{suffix})) {op} {bound}{suffix}) {{ i += ({stride}); }} return i==({endpoint}{suffix}) ? 7:8; }}\n"
                                );
                                let output = run_source_with_options(
                                    "program.c",
                                    &source,
                                    &RunOptions {
                                        capture_visualization: false,
                                        execution_step_limit: Some(5000),
                                        execution_trace_following_limit: 2,
                                        ..RunOptions::default()
                                    },
                                )
                                .unwrap_or_else(|error| panic!("{source}\n{}", error.render()));
                                assert!(
                                    output.execution_limit.is_none(),
                                    "false positive: {source}"
                                );
                                assert_eq!(output.exit_status, 7, "wrong endpoint: {source}");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 3000, "only {checked} finite cases");
}

#[test]
fn floating_counters_detect_wrong_direction_without_assuming_exact_addition() {
    for body in [
        "double i=0; while(i<10) i-=0.1;",
        "float i=10; while(i>0) i+=0.5f;",
        "for(double i=0; i<=1; i=i-0.25) {}",
        "double i=0; while(i!=1) i+=0.3;",
        "double i=1; while(i!=0) ++i;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 8,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(
            limit.executed_steps < 500,
            "{body}: {}",
            limit.executed_steps
        );
    }
    for body in [
        "double i=0; while(i<1) i+=0.1;",
        "double i=0; while(i!=1) i+=0.25;",
        "float i=10; while(i>0) --i;",
        "double i=0; while(i<1) { i-=0.25; i+=0.5; }",
        "double i=0; while(i<10) { i-=0.5; if(i<-2) break; }",
        "double i=0; while(i!=1) { i+=0.3; if(i>1) i=1; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn repeating_loops_with_long_periods_are_detected_with_bounded_memory() {
    for period in [65, 100, 257, 1000] {
        let source = format!("int main(void) {{ int i=0; while(i<{period}) i=(i+1)%{period}; }}\n");
        let output = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output.execution_limit.unwrap();
        assert_eq!(limit.reason, "repeating-loop", "period {period}");
        assert!(
            limit.executed_steps < period * 12,
            "period {period}: {}",
            limit.executed_steps
        );
    }
    let output = run_source_with_options("program.c", "int main(void) { int i=0, turns=0; while(turns<4) { i=(i+1)%100; if(i==0) ++turns; } return turns; }\n", &RunOptions {
        execution_step_limit: Some(100_000), ..RunOptions::default()
    }).unwrap();
    assert!(output.execution_limit.is_none());
    assert_eq!(output.exit_status, 4);
}

#[test]
fn loop_local_temporaries_preserve_their_external_dependencies() {
    for body in [
        "int i=0; while(i<10) { int next=i; i=next; }",
        "int i=0; while(1) { int next=(i+1)%3; i=next; if(next==4) break; }",
        "while(1) { static int count=0; ++count; count%=3; if(count==4) break; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0; while(i<10) { int next=i+1; i=next; }",
        "int i=0; while(1) { int next=i+1; if(next==4) break; i=next; }",
        "int i=0; while(i<10) { i++; { int i=1; if(i==0) break; } }",
        "while(1) { static int count=0; ++count; if(count==4) break; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn compound_loop_guards_respect_short_circuiting_and_every_possible_exit() {
    for body in [
        "int i=0; while(i<10 || i>=0) ++i;",
        "int i=0; while(i<10 && i<=0) --i;",
        "int i=0, keep=1; while(i!=10 && keep) i+=3;",
        "int i=0; while(i<10 || i%2==0) --i;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0; while(i<10 && i>=0) --i;",
        "int i=0, keep=1; while(i!=10 && keep) { i+=3; keep=0; }",
        "int i=0; while(i<10 || i>=0) { ++i; if(i==3) break; }",
        "int i=0; while(i++<10 || i<10) {}",
        "int i=0; while(++i<10 && i>=0) {}",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn unreachable_trailing_breaks_are_detected_in_unconditional_loops() {
    for body in [
        "int i=0; while(1) { --i; if(i==10) break; }",
        "int i=0; for(;;) { i+=3; if(i==10) { break; } }",
        "double i=0; do { i-=0.1; if(i>=1) break; } while(1);",
        "unsigned i=0; while(1) { i+=2; if(i==3) break; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0; while(1) { ++i; if(i==10) break; }",
        "int i=0; for(;;++i) { if(i==10) break; }",
        "int i=0; while(i>-3) { --i; if(i==10) break; }",
        "unsigned i=0; while(1) { --i; if(i>10u) break; }",
        "int i=0; while(1) { ++i; if(i++==11) break; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn outer_loop_cycles_include_nested_for_dependencies() {
    for body in [
        "while(1) { for(int i=0;i<3;i++) {} }",
        "int total=0; while(total<10) { for(int j=0;j<3;j++) total=0; }",
        "int i=0; while(i<10) { for(i=0;i<3;i++) {} }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 300, "{body}");
    }
    for body in [
        "int total=0; while(total<10) { for(int j=0;j<3;j++) total++; }",
        "int count=0; while(1) { for(int j=0;j<3;j++) count++; if(count>10) break; }",
        "int i=0; while(i<10) { i++; for(int i=0;i<3;i++) {} }",
        "int i=0; while(i<10) { for(int j=i;j<i+2;j++) {} i++; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn counter_progress_uses_only_invariant_bounds_and_step_sizes() {
    for body in [
        "int i=0, step=1; while(i<10) i-=step;",
        "int i=0, limit=10; while(limit>i) i--;",
        "double i=0, limit=10; while(limit>i) i-=0.1;",
        "int i=0, end=10, step=3; while(i!=end) i+=step;",
        "int i=0, end=11, step=3; while(i!=end-1) i+=step;",
        "double i=0, step=0.1; while(i<10) i+=-step;",
        "int i=0, step=1; while(i<10) i-=2*step;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0, step=1; while(i<10) i+=step;",
        "int i=0, step=-1; while(i<10) { i+=step; step=1; }",
        "int i=0, end=10; while(i!=end) { i+=3; end=6; }",
        "int i=0, step=1; while(i<10) { i-=step; int *p=&step; *p=-1; }",
        "double i=0, step=-0.1; while(i<10) { i+=step; step=0.1; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn negated_guards_and_comma_updates_retain_counter_progress() {
    for body in [
        "int i=0; while(!(i==10)) i+=3;",
        "int i=0; while(!(i>=10)) i--;",
        "int i=0, j=0; for(; i<10; i--,j++) {}",
        "int i=0, j=0; while(i<10) j++,i--;",
        "double i=0; int j=0; for(;i<10;i-=0.1,j++) {}",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0; while(!(i==10)) ++i;",
        "int i=0; while(!(i++>=10)) {}",
        "int i=0, j=0; for(;i<10;i--,i+=2,j++) {}",
        "double i=0; while(i<10) i-=0.1,i+=0.2;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn standard_input_stream_loops_distinguish_failed_reads_from_progress() {
    for (body, input) in [
        (
            "int n; while(!feof(stdin)) scanf(\"%d\", &n);",
            "wrong\n\u{4}",
        ),
        (
            "int n; while(fscanf(stdin, \"%d\", &n)!=EOF) {}",
            "wrong\n\u{4}",
        ),
        ("while(fgetc(stdin)!='\\n') {}", "abc\u{4}"),
        ("while(getc(stdin)!='\\n') {}", "abc\u{4}"),
        (
            "char c; do { scanf(\"%c\", &c); } while(c!='x');",
            "abc\u{4}",
        ),
        (
            "char line[16]; do { fgets(line,16,stdin); } while(line[0]!='x');",
            "abc\n\u{4}",
        ),
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <stdio.h>\nint main(void) {{ {body} }}\n"),
            &RunOptions {
                stdin: input.to_owned(),
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 300, "{body}");
    }
    for (body, input) in [
        (
            "int n; while(!feof(stdin)) scanf(\"%d\", &n);",
            "1 2 3\n\u{4}",
        ),
        (
            "int n; while(fscanf(stdin, \"%d\", &n)!=EOF) {}",
            "1 2 3\n\u{4}",
        ),
        ("while(fgetc(stdin)!='\\n') {}", "abc\n\u{4}"),
        ("while(getc(stdin)!=EOF) {}", "abc\u{4}"),
        (
            "char line[16]; do { fgets(line,16,stdin); } while(line[0]!='x');",
            "abc\nx\n\u{4}",
        ),
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <stdio.h>\nint main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                stdin: input.to_owned(),
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn counter_progress_across_branches_requires_a_valid_update_on_every_path() {
    for body in [
        "int i=0, j=0; while(i<10) { if(j++%2) --i; else i-=1; }",
        "double i=0; int j=0; while(i<10) { if(j++%2) i-=0.1; else i-=0.5; }",
        "double i=0; int j=0; while(i<10) { if(j++%2) i-=0.1; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0, j=0; while(i<10) { if(j++%2) i--; else i+=2; }",
        "int i=0; while(i<10) { if(i++%2) ++i; }",
        "double i=0; int j=0; while(i<10) { if(j++%2) i-=0.1; else i+=0.5; }",
        "int i=0; while(i<10) { if(i== -3) break; --i; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn shifted_counter_guards_use_the_actual_arithmetic_domain() {
    for body in [
        "int i=0; while(i+1<10) i--;",
        "int i=0; while(i-1!=10) i+=2;",
        "int i=0; while(i-10) i-=1;",
        "unsigned i=0; while(i+1u!=10u) i+=2u;",
        "unsigned i=0; while(i+1u>=0u) i++;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 200, "{body}");
    }
    for body in [
        "int i=0; while(i+1<10) i++;",
        "int i=0; while(i-1!=10) i++;",
        "int i=0; while(i-10) i++;",
        "unsigned i=4294967294u; while(i+1u>0u) i++;",
        "unsigned i=4294967294u; while(i+1u!=0u) i++;",
        "int i=-2; while(i+1u>0u) i+=1L;",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn loop_snapshots_track_read_subobjects_without_requiring_unread_storage() {
    for body in [
        "int a[1000]; a[0]=0; while(a[0]<10) a[0]=0;",
        "int a[3][1000]; a[2][5]=0; while(a[2][5]<10) a[2][5]=0;",
        "struct S { int ready; int unused; } s; s.ready=0; while(!s.ready) s.ready=0;",
        "char line[100]; do { fgets(line,100,stdin); puts(line); } while(line[0]!='x');",
        "char line[100]; while(fgets(line,1,stdin)) {}",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <stdio.h>\nint main(void) {{ {body} }}\n"),
            &RunOptions {
                stdin: "abc\n\u{4}".to_owned(),
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "repeating-loop", "{body}");
        assert!(limit.executed_steps < 300, "{body}");
    }
    for body in [
        "int a[1000]; a[0]=0; while(a[0]<10) ++a[0];",
        "int a[2]={0,0}; int *p=a; while(a[0]==0) { if(++p[1]==5) a[0]=1; }",
        "int a[2]={0,0}; while(a[0]==0) { if(++a[1]==5) a[0]=1; }",
        "char b[8]={0}; char *p=b; while(b[4]==0) { fgets(p,2,stdin); p++; }",
        "char line[100]; do { fgets(line,100,stdin); puts(line); } while(line[0]!='x');",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("#include <stdio.h>\nint main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                stdin: "abcde\nx\n\u{4}".to_owned(),
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
    for function in [
        "int puts(char *s) { static int count; s[0] = ++count==3 ? 'x' : 'a'; return 0; }",
        "char *fgets(char *s, int n, void *f) { static int count; s[0] = ++count==3 ? 'x' : 'a'; return s; }",
    ] {
        let call = if function.starts_with("int puts") {
            "puts(line)"
        } else {
            "fgets(line, 3, 0)"
        };
        let output = run_source_with_options("program.c", format!("{function}\nint main(void) {{ char line[3]={{'a',0,0}}; while(line[0]!='x') {call}; return 7; }}\n"), &RunOptions {
            execution_step_limit: Some(100_000), ..RunOptions::default()
        }).unwrap();
        assert!(
            output.execution_limit.is_none(),
            "false positive: {function}"
        );
        assert_eq!(output.exit_status, 7);
    }
}

#[test]
fn raw_pointer_slots_keep_indirect_loop_dependencies_live() {
    for setup in [
        "struct Box b; struct Box *p=&b; p->value=&i;",
        "struct Box *p=malloc(sizeof *p); p->value=&i;",
    ] {
        let source = format!(
            "#include <stdlib.h>\nstruct Box {{ int *value; }};\nint main(void) {{ int i=0; {setup} while(*p->value<10) i++; return i; }}\n"
        );
        let output = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {setup}");
        assert_eq!(output.exit_status, 10);
    }
}

#[test]
fn unreachable_flag_updates_cannot_end_the_loop() {
    for body in [
        "int i=0, done=0; while(!done) { --i; if(i==10) done=1; }",
        "int i=0, running=1; while(running) { i+=3; if(i==10) { running=0; } }",
        "int i=0, done=0; while(done==0) { --i; if(i>10) done=1; }",
        "double i=0; int done=0; do { i-=0.1; if(i>=1) done=1; } while(!done);",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} }}\n"),
            &RunOptions {
                capture_visualization: false,
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        let limit = output
            .execution_limit
            .unwrap_or_else(|| panic!("not detected: {body}"));
        assert_eq!(limit.reason, "unreachable-loop-exit", "{body}");
        assert!(limit.executed_steps < 300, "{body}");
    }
    for body in [
        "int i=0, done=0; while(!done) { ++i; if(i==10) done=1; }",
        "int i=0, done=0; while(!done) { --i; if(i==-3) done=1; if(i==10) done=1; }",
        "int i=0, done=0; int *p=&done; while(!done) { --i; *p=1; if(i==10) done=1; }",
        "int i=0, done=0; while(!done && i>-3) { --i; if(i==10) done=1; }",
        "int i=0, done=0; while(!done) { i+=3; if(i==3) done=1; }",
    ] {
        let output = run_source_with_options(
            "program.c",
            format!("int main(void) {{ {body} return 7; }}\n"),
            &RunOptions {
                execution_step_limit: Some(100_000),
                ..RunOptions::default()
            },
        )
        .unwrap();
        assert!(output.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(output.exit_status, 7, "{body}");
    }
}

#[test]
fn finite_condition_updates_match_an_independent_model_for_all_loop_forms() {
    let mut checked = 0;
    for unsigned in [false, true] {
        let starts: &[i128] = if unsigned {
            &[0, 2, u32::MAX as i128 - 2]
        } else {
            &[-3, 0, 3]
        };
        for &start in starts {
            for bound in [0i128, 2, 4] {
                for stride in [-2i128, -1, 0, 1, 2] {
                    for delta in [-1i128, 1] {
                        for prefix in [false, true] {
                            for op in ["<", "<=", ">", ">=", "!="] {
                                for form in ["while", "do", "for"] {
                                    let advance = |value: i128, change: i128| {
                                        let value = value + change;
                                        if unsigned {
                                            Some(value.rem_euclid(1i128 << 32))
                                        } else {
                                            (i32::MIN as i128..=i32::MAX as i128)
                                                .contains(&value)
                                                .then_some(value)
                                        }
                                    };
                                    let mut current = start;
                                    let mut iterations = 0;
                                    let mut endpoint = None;
                                    for _ in 0..64 {
                                        if form == "do" {
                                            let Some(next) = advance(current, stride) else {
                                                break;
                                            };
                                            current = next;
                                            iterations += 1;
                                        }
                                        let previous = current;
                                        let Some(next) = advance(current, delta) else {
                                            break;
                                        };
                                        current = next;
                                        let compared = if prefix { current } else { previous };
                                        let condition = match op {
                                            "<" => compared < bound,
                                            "<=" => compared <= bound,
                                            ">" => compared > bound,
                                            ">=" => compared >= bound,
                                            _ => compared != bound,
                                        };
                                        if !condition {
                                            endpoint = Some(current);
                                            break;
                                        }
                                        if form != "do" {
                                            let Some(next) = advance(current, stride) else {
                                                break;
                                            };
                                            current = next;
                                            iterations += 1;
                                        }
                                    }
                                    let Some(endpoint) = endpoint else {
                                        continue;
                                    };
                                    let ty = if unsigned { "unsigned" } else { "int" };
                                    let suffix = if unsigned { "u" } else { "" };
                                    let update = if delta == 1 { "++" } else { "--" };
                                    let counter = if prefix {
                                        format!("{update}i")
                                    } else {
                                        format!("i{update}")
                                    };
                                    let condition = format!("{counter} {op} {bound}{suffix}");
                                    let body = format!("i += ({stride}); ++count;");
                                    let loop_code = match form {
                                        "do" => format!("do {{ {body} }} while({condition});"),
                                        "for" => format!(
                                            "for(; {condition}; i += ({stride})) {{ ++count; }}"
                                        ),
                                        _ => format!("while({condition}) {{ {body} }}"),
                                    };
                                    let source = format!(
                                        "int main(void) {{ {ty} i={start}{suffix}; int count=0; {loop_code} return i=={endpoint}{suffix} && count=={iterations} ? 7:8; }}\n"
                                    );
                                    let output = run_source_with_options(
                                        "program.c",
                                        &source,
                                        &RunOptions {
                                            capture_visualization: false,
                                            execution_step_limit: Some(5000),
                                            execution_trace_following_limit: 1,
                                            ..RunOptions::default()
                                        },
                                    )
                                    .unwrap_or_else(|error| panic!("{source}\n{}", error.render()));
                                    assert!(
                                        output.execution_limit.is_none(),
                                        "false positive: {source}"
                                    );
                                    assert_eq!(output.exit_status, 7, "wrong endpoint: {source}");
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 2000, "only {checked} finite cases");
}

#[test]
fn delayed_exits_have_identical_results_with_loop_detection_enabled_and_disabled() {
    for body in [
        "int i=0,j=0; while(i<3) { if(++j%97==0) ++i; } printf(\"%d %d\",i,j);",
        "int i=0,j=0; do { if(++j%97==0) ++i; } while(i<3); printf(\"%d %d\",i,j);",
        "int a[3]={0}; while(a[0]<3) { a[2]++; a[1]=a[2]/97; a[0]=a[1]; } printf(\"%d\",a[0]);",
        "int i=0; int *p=&i; while(i<5) tick(p); printf(\"%d\",i);",
        "int i=0; while(i<5) { int t=next(); i=t/97; } printf(\"%d\",i);",
        "int i=0,j=0; while(i<5) { switch(++j%97) { case 0: ++i; break; default: continue; } } printf(\"%d\",i);",
        "int i=0,j=0; while(i<5) { for(int k=0;k<3;k++) ++j; i=j/97; } printf(\"%d\",i);",
        "int i=0; while(i<5) { if(i==2) { int i=7; printf(\"%d\",i); } ++i; }",
        "int i=0; while(i<5) { int j=0; do { ++j; } while(j<97); ++i; }",
        "double x=0; int i=0; while(x<5) { if(++i%2) x-=0.25; else x+=0.5; } printf(\"%g\",x);",
        "double x=0; while(x<1) x+=0.1; printf(\"%.17g\",x);",
        "int i=0; while(1) { i+=3; if(i>=100) break; } printf(\"%d\",i);",
        "int i=0,done=0; while(!done) { i+=3; if(i>=100) done=1; } printf(\"%d\",i);",
        "int n=0; while(scanf(\"%d\",&n)==1 && n<5) {} printf(\"%d\",n);",
        "char line[50]; do { fgets(line,50,stdin); } while(line[0]!='5'); puts(line);",
    ] {
        let source = format!(
            "#include <stdio.h>\nvoid tick(int *p) {{ static int n; if(++n%97==0) ++*p; }}\nint next(void) {{ static int n; return ++n; }}\nint main(void) {{ {body} return 7; }}\n"
        );
        let options = RunOptions {
            stdin: "1\n2\n3\n4\n5\n\u{4}".into(),
            ..RunOptions::default()
        };
        let baseline = run_source_with_options("program.c", &source, &options).unwrap();
        assert!(baseline.blocked.is_none(), "{body}");
        let detected = run_source_with_options(
            "program.c",
            &source,
            &RunOptions {
                execution_step_limit: Some(100_000),
                execution_trace_following_limit: 1,
                ..options
            },
        )
        .unwrap();
        assert!(detected.execution_limit.is_none(), "false positive: {body}");
        assert_eq!(format!("{detected:?}"), format!("{baseline:?}"), "{body}");
    }
}
