"use strict";
/// <reference path="./shared-code-editor.ts" />
{
    const { createCodeEditorTemplate } = window.MB;
    function firstRedeclaration(statements) {
        const seen = new Set();
        for (const stmt of statements) {
            if (stmt.kind === "decl" ||
                stmt.kind === "declAssign" ||
                stmt.kind === "declAssignVar" ||
                stmt.kind === "declAssignRef") {
                if (seen.has(stmt.name))
                    return stmt.name;
                seen.add(stmt.name);
            }
        }
        return null;
    }
    createCodeEditorTemplate({
        startCode: "",
        textareaMinLines: 4,
        targetState: [
            { name: "apple", type: "int", value: "10", address: "<i>(any)</i>" },
            { name: "berry", type: "int", value: "5", address: "<i>(any)</i>" },
        ],
        next: "program6.html",
        instructions: "Write the code yourself.",
        hints: (ctx) => {
            const { applyUserProgram, targetState, findMissingSemicolonLines, text, tokenizeProgram, parseStatements, } = ctx;
            const currentState = applyUserProgram();
            const expected = targetState || [];
            if (currentState === null) {
                const missingLines = findMissingSemicolonLines(text || "");
                if (missingLines.length) {
                    const lineList = missingLines.map(String);
                    let formatted = lineList[0];
                    if (lineList.length === 2) {
                        formatted = `${lineList[0]} and ${lineList[1]}`;
                    }
                    else if (lineList.length > 2) {
                        formatted = `${lineList.slice(0, -1).join(", ")}, and ${lineList[lineList.length - 1]}`;
                    }
                    return `You need ${missingLines.length === 1 ? "a semicolon" : "semicolons"} at the end of line${missingLines.length === 1 ? " " : "s "}${formatted}.`;
                }
                return "Your program has a problem that isn't covered by a hint, sorry. You can look at the earlier programs if you forget the syntax.";
            }
            const missingLines = findMissingSemicolonLines(text || "");
            if (missingLines.length) {
                const lineList = missingLines.map(String);
                let formatted = lineList[0];
                if (lineList.length === 2) {
                    formatted = `${lineList[0]} and ${lineList[1]}`;
                }
                else if (lineList.length > 2) {
                    formatted = `${lineList.slice(0, -1).join(", ")}, and ${lineList[lineList.length - 1]}`;
                }
                return `You need ${missingLines.length === 1 ? "a semicolon" : "semicolons"} at the end of line${missingLines.length === 1 ? " " : "s "}${formatted}.`;
            }
            const tokens = tokenizeProgram(text || "");
            const parsedStatements = parseStatements(text || "");
            const redecl = firstRedeclaration(parsedStatements);
            if (redecl) {
                return `You declared $n{${redecl}} more than once.`;
            }
            const hasTokens = tokens.some((t) => !(t.type === "sym" && t.value === ";"));
            const declNames = new Set(parsedStatements
                .filter((s) => s.kind === "decl" ||
                s.kind === "declAssign" ||
                s.kind === "declAssignVar" ||
                s.kind === "declAssignRef")
                .map((s) => s.name));
            if (!hasTokens || !declNames.has("apple")) {
                return "You need to declare a variable named $n{apple}. Look at how variables were declared in the earlier programs.";
            }
            if (!declNames.has("berry")) {
                return "You also need to declare a variable named $n{berry}.";
            }
            if (Array.isArray(currentState)) {
                const byName = Object.fromEntries(currentState.map((b) => [b.name, b]));
                for (const exp of expected) {
                    const actual = byName[exp.name];
                    if (!actual)
                        continue;
                    const actualVal = actual.value;
                    const expectedVal = exp.value;
                    if (actualVal !== "" && actualVal !== expectedVal) {
                        return `$n{${exp.name}}'s value should be $v{${expectedVal}}, not $v{${actualVal}}.`;
                    }
                }
            }
            const appleAssign = parsedStatements.find((s) => (s.kind === "assign" || s.kind === "declAssign") &&
                s.name === "apple" &&
                String(s.value) === "10");
            if (!appleAssign) {
                return "$n{apple} needs to end up with a value. Check the target final state.";
            }
            const berryAssign = parsedStatements.find((s) => (s.kind === "assign" || s.kind === "declAssign") &&
                s.name === "berry" &&
                String(s.value) === "5");
            if (!berryAssign) {
                return "$n{berry} also needs to end up with a value. Check the target final state.";
            }
            return "Keep lines to simple declarations or assignments ending with semicolons.";
        },
    });
}
