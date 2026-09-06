import type {
  BoxState,
  CTypeHelpNode,
  CTypeInfo,
  CTypeKind,
} from "./shared-core-utils.js";
import { rustDiagnostic, type CInterpreterError } from "./shared-c-diagnostics.js";
import { WasiProcExit } from "./shared-c-wasm-bridge.js";
import type {
  CExpressionResult,
  CProgramBlocked,
  CProgramExecutionLimit,
  CProgramResult,
  CProgramSourceLocation,
  CProgramSourceRange,
  CProgramTraceEvent,
} from "./shared-c-interpreter-types.js";

type JsonObject = Record<string, unknown>;
type InterpreterSubject = "program" | "expression";

function schemaError(path: string, expected: string): never {
  throw new Error(`invalid interpreter response at ${path}: expected ${expected}`);
}

function objectAt(value: unknown, path: string): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return schemaError(path, "an object");
  }
  return value as JsonObject;
}

function exactKeysAt(
  object: JsonObject,
  path: string,
  allowedKeys: readonly string[],
): void {
  const allowed = new Set(allowedKeys);
  for (const key of Object.keys(object)) {
    if (!allowed.has(key)) {
      schemaError(`${path}.${key}`, "no unexpected field");
    }
  }
}

function arrayAt(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) return schemaError(path, "an array");
  return value;
}

function stringAt(value: unknown, path: string): string {
  if (typeof value !== "string") return schemaError(path, "a string");
  return value;
}

function booleanAt(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") return schemaError(path, "a boolean");
  return value;
}

function numberAt(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return schemaError(path, "a finite number");
  }
  return value;
}

function nonnegativeIntegerAt(value: unknown, path: string): number {
  const number = numberAt(value, path);
  if (!Number.isInteger(number) || number < 0) {
    return schemaError(path, "a nonnegative integer");
  }
  return number;
}

function nullableStringAt(value: unknown, path: string): string | null {
  return value === null ? null : stringAt(value, path);
}

function nullableNumberAt(value: unknown, path: string): number | null {
  return value === null ? null : numberAt(value, path);
}

function stringArrayAt(value: unknown, path: string): string[] {
  return arrayAt(value, path).map((item, index) =>
    stringAt(item, `${path}[${index}]`)
  );
}

function integerArrayAt(value: unknown, path: string): number[] {
  return arrayAt(value, path).map((item, index) =>
    nonnegativeIntegerAt(item, `${path}[${index}]`)
  );
}

function decodeTypeHelpNode(
  value: unknown,
  path: string,
  depth = 0,
): CTypeHelpNode {
  if (depth > 24) return schemaError(path, "a type-help tree no deeper than 24");
  const node = objectAt(value, path);
  exactKeysAt(node, path, ["kind", "label", "typeName", "children"]);
  const kind = stringAt(node.kind, `${path}.kind`);
  if (!["pointer", "array", "function", "type"].includes(kind)) {
    return schemaError(`${path}.kind`, "a known type-help node kind");
  }
  return {
    kind: kind as CTypeHelpNode["kind"],
    label: stringAt(node.label, `${path}.label`),
    typeName: nullableStringAt(node.typeName, `${path}.typeName`),
    children: arrayAt(node.children, `${path}.children`).map((rawChild, index) => {
      const childPath = `${path}.children[${index}]`;
      const child = objectAt(rawChild, childPath);
      exactKeysAt(child, childPath, ["relation", "node"]);
      return {
        relation: stringAt(child.relation, `${childPath}.relation`),
        node: decodeTypeHelpNode(child.node, `${childPath}.node`, depth + 1),
      };
    }),
  };
}

function decodeTypeInfo(value: unknown, path: string): CTypeInfo {
  const info = objectAt(value, path);
  exactKeysAt(info, path, [
    "kind",
    "help",
    "helpTypeNames",
    "helpTree",
    "pointerDepth",
    "arrayShape",
    "pointeeArrayShape",
    "size",
    "align",
  ]);
  const kind = stringAt(info.kind, `${path}.kind`);
  const kinds: CTypeKind[] = [
    "void",
    "integer",
    "floating",
    "complex",
    "pointer",
    "array",
    "aggregate",
    "function",
    "va-list",
    "unknown",
  ];
  if (!kinds.includes(kind as CTypeKind)) {
    return schemaError(`${path}.kind`, "a known C type kind");
  }
  return {
    kind: kind as CTypeKind,
    help: nullableStringAt(info.help, `${path}.help`),
    helpTypeNames: stringArrayAt(info.helpTypeNames, `${path}.helpTypeNames`),
    helpTree:
      info.helpTree === null
        ? null
        : decodeTypeHelpNode(info.helpTree, `${path}.helpTree`),
    pointerDepth: nonnegativeIntegerAt(info.pointerDepth, `${path}.pointerDepth`),
    arrayShape: integerArrayAt(info.arrayShape, `${path}.arrayShape`),
    pointeeArrayShape: integerArrayAt(
      info.pointeeArrayShape,
      `${path}.pointeeArrayShape`,
    ),
    size: nullableNumberAt(info.size, `${path}.size`),
    align: nullableNumberAt(info.align, `${path}.align`),
  };
}

function decodeBox(value: unknown, path: string): BoxState {
  const box = objectAt(value, path);
  exactKeysAt(box, path, [
    "name",
    "type",
    "value",
    "displayValue",
    "exactValue",
    "address",
    "arrayRoot",
    "arrayShape",
    "arrayIndices",
    "aggregateRoot",
    "aggregatePath",
    "aggregateKind",
    "aliases",
    "typeInfo",
  ]);
  const aggregateKind = nullableStringAt(box.aggregateKind, `${path}.aggregateKind`);
  if (aggregateKind !== null && aggregateKind !== "struct" && aggregateKind !== "union") {
    return schemaError(`${path}.aggregateKind`, '"struct", "union", or null');
  }
  return {
    name: stringAt(box.name, `${path}.name`),
    type: stringAt(box.type, `${path}.type`),
    value: stringAt(box.value, `${path}.value`),
    displayValue: stringAt(box.displayValue, `${path}.displayValue`),
    exactValue: stringAt(box.exactValue, `${path}.exactValue`),
    address: nullableStringAt(box.address, `${path}.address`),
    arrayRoot: nullableStringAt(box.arrayRoot, `${path}.arrayRoot`),
    arrayShape: integerArrayAt(box.arrayShape, `${path}.arrayShape`),
    arrayIndices: integerArrayAt(box.arrayIndices, `${path}.arrayIndices`),
    aggregateRoot: nullableStringAt(box.aggregateRoot, `${path}.aggregateRoot`),
    aggregatePath: stringArrayAt(box.aggregatePath, `${path}.aggregatePath`),
    aggregateKind,
    aliases: stringArrayAt(box.aliases, `${path}.aliases`),
    typeInfo: decodeTypeInfo(box.typeInfo, `${path}.typeInfo`),
  };
}

function decodeState(value: unknown, path: string): BoxState[] {
  return arrayAt(value, path).map((item, index) =>
    decodeBox(item, `${path}[${index}]`)
  );
}

function decodeSourceRange(value: unknown, path: string): CProgramSourceRange | null {
  if (value === null) return null;
  const range = objectAt(value, path);
  exactKeysAt(range, path, [
    "file",
    "startLine",
    "startColumn",
    "endLine",
    "endColumn",
  ]);
  return {
    file: stringAt(range.file, `${path}.file`),
    startLine: nonnegativeIntegerAt(range.startLine, `${path}.startLine`),
    startColumn: nonnegativeIntegerAt(range.startColumn, `${path}.startColumn`),
    endLine: nonnegativeIntegerAt(range.endLine, `${path}.endLine`),
    endColumn: nonnegativeIntegerAt(range.endColumn, `${path}.endColumn`),
  };
}

function decodeTrace(value: unknown, path: string): CProgramTraceEvent[] {
  return arrayAt(value, path).map((rawEvent, index) => {
    const eventPath = `${path}[${index}]`;
    const event = objectAt(rawEvent, eventPath);
    exactKeysAt(event, eventPath, [
      "kind",
      "file",
      "startLine",
      "endLine",
      "state",
      "skippedRange",
    ]);
    return {
      kind: stringAt(event.kind, `${eventPath}.kind`),
      file: stringAt(event.file, `${eventPath}.file`),
      startLine: nonnegativeIntegerAt(event.startLine, `${eventPath}.startLine`),
      endLine: nonnegativeIntegerAt(event.endLine, `${eventPath}.endLine`),
      state: decodeState(event.state, `${eventPath}.state`),
      skippedRange: decodeSourceRange(
        event.skippedRange,
        `${eventPath}.skippedRange`,
      ),
    };
  });
}

function decodeSourceLocation(
  value: unknown,
  path: string,
): CProgramSourceLocation | null {
  if (value === null) return null;
  const location = objectAt(value, path);
  exactKeysAt(location, path, ["file", "line"]);
  return {
    file: stringAt(location.file, `${path}.file`),
    line: nonnegativeIntegerAt(location.line, `${path}.line`),
  };
}

function decodeBlocked(value: unknown, path: string): CProgramBlocked | null {
  if (value === null) return null;
  const blocked = objectAt(value, path);
  exactKeysAt(blocked, path, [
    "file",
    "startLine",
    "endLine",
    "function",
    "state",
  ]);
  return {
    file: stringAt(blocked.file, `${path}.file`),
    startLine: nonnegativeIntegerAt(blocked.startLine, `${path}.startLine`),
    endLine: nonnegativeIntegerAt(blocked.endLine, `${path}.endLine`),
    function: stringAt(blocked.function, `${path}.function`),
    state: decodeState(blocked.state, `${path}.state`),
  };
}

function decodeExecutionLimit(
  value: unknown,
  path: string,
): CProgramExecutionLimit | null {
  if (value === null) return null;
  const limit = objectAt(value, path);
  exactKeysAt(limit, path, [
    "file",
    "startLine",
    "endLine",
    "tracePosition",
  ]);
  return {
    file: stringAt(limit.file, `${path}.file`),
    startLine: nonnegativeIntegerAt(limit.startLine, `${path}.startLine`),
    endLine: nonnegativeIntegerAt(limit.endLine, `${path}.endLine`),
    tracePosition: nonnegativeIntegerAt(
      limit.tracePosition,
      `${path}.tracePosition`,
    ),
  };
}

function decodeImplicitMain(root: JsonObject, expected: boolean) {
  const hasApplied = Object.prototype.hasOwnProperty.call(
    root,
    "implicitMainApplied",
  );
  const hasNotice = Object.prototype.hasOwnProperty.call(
    root,
    "implicitMainNotice",
  );
  if (hasApplied !== expected || hasNotice !== expected) {
    return schemaError(
      "$",
      expected
        ? "both implicit-main fields"
        : "no implicit-main fields",
    );
  }
  return {
    implicitMainApplied:
      !expected
        ? undefined
        : booleanAt(root.implicitMainApplied, "$.implicitMainApplied"),
    implicitMainNotice:
      !expected
        ? undefined
        : nullableStringAt(root.implicitMainNotice, "$.implicitMainNotice"),
  };
}

function decodeError(
  root: JsonObject,
  implicitMainExpected: boolean,
): CInterpreterError {
  const keys = [
    "ok",
    "kind",
    "message",
    "file",
    "line",
    "column",
    "endLine",
    "endColumn",
    "annotations",
    "runtimeContext",
  ];
  if (implicitMainExpected) {
    keys.push("implicitMainApplied", "implicitMainNotice");
  }
  exactKeysAt(root, "$", keys);
  if (root.ok !== false) return schemaError("$.ok", "false");
  const kind = stringAt(root.kind, "$.kind");
  if (kind !== "compile" && kind !== "ub") {
    return schemaError("$.kind", '"compile" or "ub"');
  }
  const annotations = arrayAt(root.annotations, "$.annotations").map(
    (rawAnnotation, index) => {
      const path = `$.annotations[${index}]`;
      const annotation = objectAt(rawAnnotation, path);
      exactKeysAt(annotation, path, [
        "id",
        "file",
        "line",
        "column",
        "endLine",
        "endColumn",
      ]);
      return {
        id: stringAt(annotation.id, `${path}.id`),
        file: nullableStringAt(annotation.file, `${path}.file`),
        line: nullableNumberAt(annotation.line, `${path}.line`),
        column: nullableNumberAt(annotation.column, `${path}.column`),
        endLine: nullableNumberAt(annotation.endLine, `${path}.endLine`),
        endColumn: nullableNumberAt(annotation.endColumn, `${path}.endColumn`),
      };
    },
  );
  const runtimeContext =
    root.runtimeContext === null
      ? null
      : (() => {
          const context = objectAt(root.runtimeContext, "$.runtimeContext");
          exactKeysAt(context, "$.runtimeContext", [
            "executedSteps",
            "lineExecutionCount",
            "state",
          ]);
          return {
            executedSteps: nonnegativeIntegerAt(
              context.executedSteps,
              "$.runtimeContext.executedSteps",
            ),
            lineExecutionCount:
              context.lineExecutionCount === null
                ? null
                : nonnegativeIntegerAt(
                    context.lineExecutionCount,
                    "$.runtimeContext.lineExecutionCount",
                  ),
            state: decodeState(context.state, "$.runtimeContext.state"),
          };
        })();
  return {
    ok: false,
    kind,
    message: stringAt(root.message, "$.message"),
    file: nullableStringAt(root.file, "$.file"),
    line: nullableNumberAt(root.line, "$.line"),
    column: nullableNumberAt(root.column, "$.column"),
    endLine: nullableNumberAt(root.endLine, "$.endLine"),
    endColumn: nullableNumberAt(root.endColumn, "$.endColumn"),
    annotations,
    runtimeContext,
    ...decodeImplicitMain(root, implicitMainExpected),
  };
}

function bridgeFailureMessage(
  subject: InterpreterSubject,
  error?: unknown,
): string {
  const base = `The interpreter stopped while checking this ${subject}`;
  if (error instanceof WasiProcExit || !(error instanceof Error) || !error.message) {
    return `${base}.`;
  }
  return `${base}: ${error.message}`;
}

function compileFailure(message: string): CProgramResult {
  return {
    kind: "compile",
    diagnostic: {
      kind: "compile",
      message,
      range: { startLine: 0, startCol: 0, endLine: 0, endCol: 1 },
    },
  };
}

function crashProgramDiagnostic(error: unknown): CProgramResult {
  return compileFailure(bridgeFailureMessage("program", error));
}

function crashExpressionDiagnostic(error: unknown): CExpressionResult {
  return compileFailure(
    bridgeFailureMessage("expression", error),
  ) as CExpressionResult;
}

function normalizeProgramResult(
  raw: unknown,
  diagnosticSource: string,
  implicitMainExpected = false,
): CProgramResult {
  const root = objectAt(raw, "$");
  const ok = booleanAt(root.ok, "$.ok");
  const implicit = decodeImplicitMain(root, implicitMainExpected);
  if (!ok) {
    const error = decodeError(root, implicitMainExpected);
    return {
      kind: error.kind,
      diagnostic: rustDiagnostic(error, diagnosticSource),
      ...implicit,
    };
  }
  const keys = [
    "ok",
    "stdout",
    "stderr",
    "exitStatus",
    "state",
    "trace",
    "mainClose",
    "blocked",
    "executionLimit",
  ];
  if (implicitMainExpected) {
    keys.push("implicitMainApplied", "implicitMainNotice");
  }
  exactKeysAt(root, "$", keys);
  return {
    kind: "ok",
    state: decodeState(root.state, "$.state"),
    trace: decodeTrace(root.trace, "$.trace"),
    mainClose: decodeSourceLocation(root.mainClose, "$.mainClose"),
    blocked: decodeBlocked(root.blocked, "$.blocked"),
    executionLimit: decodeExecutionLimit(root.executionLimit, "$.executionLimit"),
    stdout: stringAt(root.stdout, "$.stdout"),
    stderr: stringAt(root.stderr, "$.stderr"),
    exitStatus: numberAt(root.exitStatus, "$.exitStatus"),
    ...implicit,
  };
}

function normalizeExpressionResult(
  raw: unknown,
  diagnosticSource: string,
  implicitMainExpected = false,
): CExpressionResult {
  const root = objectAt(raw, "$");
  const ok = booleanAt(root.ok, "$.ok");
  if (!ok) {
    const error = decodeError(root, implicitMainExpected);
    return {
      kind: error.kind,
      diagnostic: rustDiagnostic(error, diagnosticSource),
    };
  }
  const keys = ["ok", "result"];
  if (implicitMainExpected) {
    keys.push("implicitMainApplied", "implicitMainNotice");
  }
  exactKeysAt(root, "$", keys);
  decodeImplicitMain(root, implicitMainExpected);
  const result = objectAt(root.result, "$.result");
  exactKeysAt(result, "$.result", [
    "kind",
    "type",
    "value",
    "displayValue",
    "exactValue",
    "address",
    "name",
    "valueLiteral",
    "typeInfo",
  ]);
  const valueLiteral =
    result.valueLiteral === null
      ? null
      : (() => {
          const literal = objectAt(result.valueLiteral, "$.result.valueLiteral");
          exactKeysAt(literal, "$.result.valueLiteral", ["kind", "hasSuffix"]);
          const kind = stringAt(literal.kind, "$.result.valueLiteral.kind");
          if (kind !== "integer" && kind !== "floating") {
            return schemaError(
              "$.result.valueLiteral.kind",
              '"integer" or "floating"',
            );
          }
          return {
            kind: kind as "integer" | "floating",
            hasSuffix: booleanAt(
              literal.hasSuffix,
              "$.result.valueLiteral.hasSuffix",
            ),
          };
        })();
  return {
    kind: "ok",
    result: {
      kind: stringAt(result.kind, "$.result.kind"),
      type: stringAt(result.type, "$.result.type"),
      value: stringAt(result.value, "$.result.value"),
      displayValue: stringAt(result.displayValue, "$.result.displayValue"),
      exactValue: stringAt(result.exactValue, "$.result.exactValue"),
      address:
        nullableStringAt(result.address, "$.result.address") ?? "",
      name: stringAt(result.name, "$.result.name") || undefined,
      valueLiteral,
      typeInfo: decodeTypeInfo(result.typeInfo, "$.result.typeInfo"),
    },
  };
}

function interpreterDiagnosticFile(
  raw: unknown,
  implicitMainExpected: boolean,
): string | null {
  const root = objectAt(raw, "$");
  return booleanAt(root.ok, "$.ok")
    ? null
    : decodeError(root, implicitMainExpected).file;
}

export {
  crashExpressionDiagnostic,
  crashProgramDiagnostic,
  interpreterDiagnosticFile,
  normalizeExpressionResult,
  normalizeProgramResult,
};
