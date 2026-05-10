import { createCodeEditorTemplate } from "./shared-code-editor.js";
import { missingSemicolonHint } from "./shared-code-editor-hints.js";
const targetName = "cloud";
createCodeEditorTemplate({
    startCode: "int rain;",
    textareaMinLines: 1,
    allowNewLines: true,
    targetState: [{ name: targetName, type: "int", value: "", address: "<i>(any)</i>" }],
    next: "5-code-editing-ii.html",
    instructions: 'Until now, you have been editing the program state to match the code. Now you will be editing the code to match the program state. Edit the line so that "Your code\'s final state" matches the "Target final state", then press $checkButton.',
    hints: (ctx) => {
        const semicolonHint = missingSemicolonHint(ctx.findMissingSemicolonLines(ctx.text));
        if (semicolonHint)
            return semicolonHint;
        const currentState = ctx.applyUserProgram();
        if (!currentState) {
            return `This line does not compile yet. You need to create a variable named $n{${targetName}}. Look back at the earlier levels if you forget how this is done.`;
        }
        if (currentState.length > 1) {
            return `You're declaring ${currentState.length} variables, but you only need 1.`;
        }
        const onlyVar = currentState[0];
        if (!onlyVar) {
            return `Create one variable named $n{${targetName}}.`;
        }
        if (onlyVar.name !== targetName) {
            return `Rename $n{${onlyVar.name}} to $n{${targetName}}.`;
        }
        const cloud = onlyVar;
        if (cloud.type !== "int") {
            return `$n{${targetName}} should have type $t{int}.`;
        }
        if (cloud.value !== "") {
            return `$n{${targetName}} should not have a value.`;
        }
        return `Keep the line to a simple $t{int} $n{${targetName}}; declaration.`;
    },
});
