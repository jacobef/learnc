use crate::browser::{CboxesImplicitMain, SourceDisplay, SourceDisplayMap};
use crate::diag::{
    Diagnostic, DiagnosticDisplayAnnotation, DiagnosticDisplayRange, DiagnosticRuntimeContext,
    Severity,
};
use crate::interpreter::{
    ProgramBlocked, ProgramExecutionLimit, ProgramExpressionResult, ProgramOutput,
    ProgramSourceLocation, ProgramSourceRange, ProgramStateBox, ProgramTraceEvent,
    ProgramTypeHelpNode, ProgramTypeInfo, ProgramValueLiteral,
};
use crate::token;

pub(super) fn cboxes_implicit_main_json(
    mut json: String,
    implicit_main: &CboxesImplicitMain,
) -> String {
    debug_assert!(json.ends_with('}'));
    json.pop();
    json.push_str(",\"implicitMainApplied\":");
    json.push_str(if implicit_main.applied {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"implicitMainNotice\":");
    if let Some(notice) = &implicit_main.notice {
        json.push_str(&cboxes_json_string(notice));
    } else {
        json.push_str("null");
    }
    json.push('}');
    json
}

pub(super) fn cboxes_success_json(
    result: &ProgramOutput,
    source_display: &SourceDisplayMap,
) -> String {
    format!(
        "{{\"ok\":true,\"stdout\":{},\"stderr\":{},\"exitStatus\":{},\"state\":{},\"trace\":{},\"mainClose\":{},\"blocked\":{},\"executionLimit\":{}}}",
        cboxes_json_string(&result.stdout),
        cboxes_json_string(&result.stderr),
        result.exit_status,
        cboxes_state_json(&result.state),
        cboxes_trace_json(&result.trace, source_display),
        cboxes_source_location_json(&result.main_close, source_display),
        cboxes_blocked_json(result.blocked.as_ref(), source_display),
        cboxes_execution_limit_json(result.execution_limit.as_ref(), source_display),
    )
}

pub(super) fn cboxes_expression_success_json(result: &ProgramExpressionResult) -> String {
    format!(
        "{{\"ok\":true,\"result\":{}}}",
        cboxes_expression_result_json(result)
    )
}

fn cboxes_expression_result_json(result: &ProgramExpressionResult) -> String {
    let address = result
        .address
        .map(|address| cboxes_json_string(&address.to_string()))
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\"kind\":{},\"type\":{},\"value\":{},\"displayValue\":{},\"exactValue\":{},\"address\":{},\"name\":{},\"valueLiteral\":{},\"typeInfo\":{}}}",
        cboxes_json_string(&result.kind),
        cboxes_json_string(&result.ty),
        cboxes_json_string(&result.value),
        cboxes_json_string(&result.display_value),
        cboxes_json_string(&result.exact_value),
        address,
        cboxes_json_string(&result.name),
        cboxes_value_literal_json(result.value_literal.as_ref()),
        cboxes_type_info_json(&result.type_info),
    )
}

fn cboxes_type_info_json(info: &ProgramTypeInfo) -> String {
    let size = info
        .size
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let align = info
        .align
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let help = info
        .help
        .as_deref()
        .map(cboxes_json_string)
        .unwrap_or_else(|| "null".to_owned());
    let help_tree = info
        .help_tree
        .as_ref()
        .map(cboxes_type_help_node_json)
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\"kind\":{},\"help\":{},\"helpTypeNames\":{},\"helpTree\":{},\"pointerDepth\":{},\"arrayShape\":{},\"pointeeArrayShape\":{},\"size\":{},\"align\":{}}}",
        cboxes_json_string(&info.kind),
        help,
        cboxes_string_array_json(&info.help_type_names),
        help_tree,
        info.pointer_depth,
        cboxes_usize_array_json(&info.array_shape),
        cboxes_usize_array_json(&info.pointee_array_shape),
        size,
        align,
    )
}

fn cboxes_type_help_node_json(node: &ProgramTypeHelpNode) -> String {
    let type_name = node
        .type_name
        .as_deref()
        .map(cboxes_json_string)
        .unwrap_or_else(|| "null".to_owned());
    let mut children = String::from("[");
    for (index, child) in node.children.iter().enumerate() {
        if index > 0 {
            children.push(',');
        }
        children.push_str(&format!(
            "{{\"relation\":{},\"node\":{}}}",
            cboxes_json_string(&child.relation),
            cboxes_type_help_node_json(&child.node),
        ));
    }
    children.push(']');
    format!(
        "{{\"kind\":{},\"label\":{},\"typeName\":{},\"children\":{}}}",
        cboxes_json_string(&node.kind),
        cboxes_json_string(&node.label),
        type_name,
        children,
    )
}

fn cboxes_string_array_json(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&cboxes_json_string(value));
    }
    out.push(']');
    out
}

fn cboxes_value_literal_json(literal: Option<&ProgramValueLiteral>) -> String {
    let Some(literal) = literal else {
        return "null".to_owned();
    };
    format!(
        "{{\"kind\":{},\"hasSuffix\":{}}}",
        cboxes_json_string(&literal.kind),
        literal.has_suffix,
    )
}

pub(super) fn cboxes_value_literal_text(tokens: &[token::Token]) -> Option<String> {
    use token::TokenKind;

    match tokens {
        [
            token::Token {
                kind: TokenKind::Number(text),
                ..
            },
            token::Token {
                kind: TokenKind::Eof,
                ..
            },
        ]
        | [
            token::Token {
                kind: TokenKind::Plus | TokenKind::Minus,
                ..
            },
            token::Token {
                kind: TokenKind::Number(text),
                ..
            },
            token::Token {
                kind: TokenKind::Eof,
                ..
            },
        ] => Some(text.clone()),
        _ => None,
    }
}

pub(super) fn cboxes_diagnostic_json(
    diag: &Diagnostic,
    source_display: &SourceDisplayMap,
) -> String {
    let rendered = diag.render();
    let kind = if diag.severity() == Severity::UndefinedBehavior {
        "ub"
    } else {
        "compile"
    };
    let range = diag
        .display_range()
        .and_then(|range| cboxes_display_range(source_display, range));
    let annotations = diag
        .display_annotations()
        .iter()
        .filter_map(|annotation| {
            cboxes_display_range(source_display, &annotation.range).map(|range| {
                DiagnosticDisplayAnnotation {
                    id: annotation.id.clone(),
                    range,
                }
            })
        })
        .collect::<Vec<_>>();
    cboxes_error_json_with_annotations(kind, &rendered, range, &annotations, diag.runtime_context())
}

fn cboxes_display_range(
    source_display: &SourceDisplayMap,
    range: &DiagnosticDisplayRange,
) -> Option<DiagnosticDisplayRange> {
    let file = range.path.to_string_lossy();
    let display = source_display
        .get(file.as_ref())
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = range.start_line.saturating_sub(display.line_offset);
    if start_line == display.line_count && display.normalized_final_newline {
        let eof_line = display.line_count.saturating_sub(1);
        return Some(DiagnosticDisplayRange {
            path: range.path.clone(),
            start_line: eof_line,
            start_column: display.eof_column,
            end_line: eof_line,
            end_column: display.eof_column,
        });
    }
    if start_line >= display.line_count {
        return None;
    }
    Some(DiagnosticDisplayRange {
        start_line,
        end_line: range
            .end_line
            .saturating_sub(display.line_offset)
            .max(start_line)
            .min(display.line_count.saturating_sub(1)),
        ..range.clone()
    })
}

pub(super) fn cboxes_error_json(kind: &str, message: &str) -> String {
    cboxes_error_json_with_annotations(kind, message, None, &[], None)
}

fn cboxes_error_json_with_annotations(
    kind: &str,
    message: &str,
    range: Option<DiagnosticDisplayRange>,
    annotations: &[DiagnosticDisplayAnnotation],
    runtime_context: Option<&DiagnosticRuntimeContext>,
) -> String {
    let (file, line, col, end_line, end_col) = range
        .map(|range| {
            (
                cboxes_json_string(&range.path.to_string_lossy()),
                range.start_line.to_string(),
                range.start_column.to_string(),
                range.end_line.to_string(),
                range.end_column.to_string(),
            )
        })
        .unwrap_or_else(|| {
            (
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
            )
        });
    let mut annotations_json = String::from("[");
    for (index, annotation) in annotations.iter().enumerate() {
        let range = &annotation.range;
        if index > 0 {
            annotations_json.push(',');
        }
        annotations_json.push_str(&format!(
            "{{\"id\":{},\"file\":{},\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{}}}",
            cboxes_json_string(&annotation.id),
            cboxes_json_string(&range.path.to_string_lossy()),
            range.start_line,
            range.start_column,
            range.end_line,
            range.end_column,
        ));
    }
    annotations_json.push(']');
    let runtime_context_json = cboxes_runtime_context_json(runtime_context);
    format!(
        "{{\"ok\":false,\"kind\":{},\"message\":{},\"file\":{},\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{},\"annotations\":{},\"runtimeContext\":{}}}",
        cboxes_json_string(kind),
        cboxes_json_string(message),
        file,
        line,
        col,
        end_line,
        end_col,
        annotations_json,
        runtime_context_json,
    )
}

fn cboxes_runtime_context_json(context: Option<&DiagnosticRuntimeContext>) -> String {
    let Some(context) = context else {
        return "null".to_owned();
    };
    let line_execution_count = context
        .line_execution_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\"executedSteps\":{},\"lineExecutionCount\":{},\"state\":{}}}",
        context.executed_steps,
        line_execution_count,
        cboxes_state_json(&context.state),
    )
}

fn cboxes_state_json(state: &[ProgramStateBox]) -> String {
    let mut out = String::from("[");
    for (index, item) in state.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        out.push_str("\"name\":");
        out.push_str(&cboxes_json_string(&item.name));
        out.push_str(",\"type\":");
        out.push_str(&cboxes_json_string(&item.ty));
        out.push_str(",\"value\":");
        out.push_str(&cboxes_json_string(&item.value));
        out.push_str(",\"displayValue\":");
        out.push_str(&cboxes_json_string(&item.display_value));
        out.push_str(",\"exactValue\":");
        out.push_str(&cboxes_json_string(&item.exact_value));
        out.push_str(",\"address\":");
        if let Some(address) = item.address {
            out.push_str(&cboxes_json_string(&address.to_string()));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"arrayRoot\":");
        if let Some(root) = &item.array_root {
            out.push_str(&cboxes_json_string(root));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"arrayShape\":");
        out.push_str(&cboxes_usize_array_json(&item.array_shape));
        out.push_str(",\"arrayIndices\":");
        out.push_str(&cboxes_usize_array_json(&item.array_indices));
        out.push_str(",\"aggregateRoot\":");
        if let Some(root) = &item.aggregate_root {
            out.push_str(&cboxes_json_string(root));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"aggregatePath\":");
        out.push_str(&cboxes_string_array_json(&item.aggregate_path));
        out.push_str(",\"aggregateKind\":");
        if let Some(kind) = &item.aggregate_kind {
            out.push_str(&cboxes_json_string(kind));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"aliases\":");
        out.push_str(&cboxes_string_array_json(&item.aliases));
        out.push_str(",\"typeInfo\":");
        out.push_str(&cboxes_type_info_json(&item.type_info));
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_display_lines(
    source_display: &SourceDisplayMap,
    file: &str,
    start_line: usize,
    end_line: usize,
) -> Option<(usize, usize)> {
    let display = source_display
        .get(file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = start_line.saturating_sub(display.line_offset);
    (start_line < display.line_count).then(|| {
        (
            start_line,
            end_line
                .saturating_sub(display.line_offset)
                .min(display.line_count.saturating_sub(1)),
        )
    })
}

fn cboxes_trace_json(trace: &[ProgramTraceEvent], source_display: &SourceDisplayMap) -> String {
    let mut out = String::from("[");
    let mut wrote_event = false;
    for event in trace {
        let Some((start_line, end_line)) = cboxes_display_lines(
            source_display,
            &event.file,
            event.start_line,
            event.end_line,
        ) else {
            continue;
        };
        if wrote_event {
            out.push(',');
        }
        wrote_event = true;
        out.push('{');
        out.push_str("\"kind\":");
        out.push_str(&cboxes_json_string(&event.kind));
        out.push_str(",\"file\":");
        out.push_str(&cboxes_json_string(&event.file));
        out.push_str(",\"startLine\":");
        out.push_str(&start_line.to_string());
        out.push_str(",\"endLine\":");
        out.push_str(&end_line.to_string());
        out.push_str(",\"state\":");
        out.push_str(&cboxes_state_json(&event.state));
        out.push_str(",\"skippedRange\":");
        out.push_str(&cboxes_source_range_json(
            event.skipped_range.as_ref(),
            source_display,
        ));
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_source_range_json(
    range: Option<&ProgramSourceRange>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(range) = range else {
        return "null".to_owned();
    };
    let Some((start_line, end_line)) = cboxes_display_lines(
        source_display,
        &range.file,
        range.start_line,
        range.end_line,
    ) else {
        return "null".to_owned();
    };
    format!(
        "{{\"file\":{},\"startLine\":{},\"startColumn\":{},\"endLine\":{},\"endColumn\":{}}}",
        cboxes_json_string(&range.file),
        start_line,
        range.start_column,
        end_line,
        range.end_column,
    )
}

fn cboxes_source_location_json(
    location: &ProgramSourceLocation,
    source_display: &SourceDisplayMap,
) -> String {
    let Some((line, _)) =
        cboxes_display_lines(source_display, &location.file, location.line, location.line)
    else {
        return "null".to_owned();
    };
    format!(
        "{{\"file\":{},\"line\":{}}}",
        cboxes_json_string(&location.file),
        line
    )
}

fn cboxes_blocked_json(
    blocked: Option<&ProgramBlocked>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(blocked) = blocked else {
        return "null".to_owned();
    };
    let Some((start_line, end_line)) = cboxes_display_lines(
        source_display,
        &blocked.file,
        blocked.start_line,
        blocked.end_line,
    ) else {
        return "null".to_owned();
    };
    format!(
        "{{\"file\":{},\"startLine\":{},\"endLine\":{},\"function\":{},\"state\":{}}}",
        cboxes_json_string(&blocked.file),
        start_line,
        end_line,
        cboxes_json_string(&blocked.function),
        cboxes_state_json(&blocked.state)
    )
}

fn cboxes_execution_limit_json(
    execution_limit: Option<&ProgramExecutionLimit>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(execution_limit) = execution_limit else {
        return "null".to_owned();
    };
    let Some((start_line, end_line)) = cboxes_display_lines(
        source_display,
        &execution_limit.file,
        execution_limit.start_line,
        execution_limit.end_line,
    ) else {
        return "null".to_owned();
    };
    format!(
        "{{\"file\":{},\"startLine\":{},\"endLine\":{},\"tracePosition\":{}}}",
        cboxes_json_string(&execution_limit.file),
        start_line,
        end_line,
        execution_limit.trace_position,
    )
}

fn cboxes_usize_array_json(values: &[usize]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn cboxes_json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
