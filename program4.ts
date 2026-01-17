import { createCodeEditorTemplate } from "./shared-code-editor.js";

createCodeEditorTemplate({
  startCode: "int rain;",
  textareaMinLines: 1,
  allowNewLines: true,
  targetState: [
    { name: "cloud", type: "int", value: "", address: "<i>(any)</i>" },
  ],
  next: "program5.html",
  instructions:
    'Until now, you have been editing the program state to match the code. Now you will be editing the code to match the program state. Edit the line so that "Your code\'s final state" matches the "Target final state", then press $checkButton.',
  hints: (ctx) => {
    const missingLines = ctx.findMissingSemicolonLines(ctx.text || "");
    if (missingLines.length) {
      const lineList = missingLines.map(String);
      let formatted = lineList[0];
      if (lineList.length === 2) {
        formatted = `${lineList[0]} and ${lineList[1]}`;
      } else if (lineList.length > 2) {
        formatted = `${lineList.slice(0, -1).join(", ")}, and ${lineList[lineList.length - 1]}`;
      }
      return `You need ${missingLines.length === 1 ? "a semicolon" : "semicolons"} at the end of line${missingLines.length === 1 ? " " : "s "}${formatted}.`;
    }

    const currentState = ctx.applyUserProgram();
    if (!Array.isArray(currentState)) {
      return "Your program has a problem that isn't covered by a hint, sorry. You can look at the earlier programs if you forget the syntax.";
    }

    if (currentState.length > 1) {
      return `You're declaring ${currentState.length} variables, but you only need 1.`;
    }
    const onlyVar = currentState[0];
    if (!onlyVar) {
      return "You need to create a variable named $n{cloud}.";
    }
    if (onlyVar.name !== "cloud") {
      return `Your variable's name should be $n{cloud}, not $n{${onlyVar.name}}.`;
    }
    const cloud = onlyVar;
    if (cloud.type !== "int") {
      return "$n{cloud}'s type should be $t{int}.";
    }
    if (String(cloud.value || "") !== "") {
      return "$n{cloud} should be empty—don't assign it a value.";
    }
    return "Keep the line to a simple declaration ending with a semicolon.";
  },
});
