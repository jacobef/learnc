use super::*;

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
