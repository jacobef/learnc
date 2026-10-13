import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [
        { name: "a", type: "int" },
        { name: "b", type: "int" },
    ],
    outputs: [{ name: "result", type: "int" }],
    testInputs: [["3", "7"], ["7", "3"], ["5", "5"], ["-2", "-9"], ["-9", "-2"], ["0", "-1"], ["100", "99"]],
    startInput: ["3", "7"],
    main: ([a, b]) => `int main(void) {\n  int result = max(${a}, ${b});\n  return 0;\n}`,
    solve: "int max(int a, int b) {\n  if (a > b) {\n    return a;\n  }\n  return b;\n}",
    textareaMinLines: 7,
    instructions: "Write a function named $c{max} that takes two $t{int}s and returns the larger one. The $c{main} below calls it; it's locked, and changes with each test input.\nA function can have more than one $c{return} statement. Whichever one runs first ends the function.",
    hints: (ctx) => {
        if (ctx.behavesLike("int max(int a, int b) {\n  if (a < b) {\n    return a;\n  }\n  return b;\n}")) {
            return "That returns the smaller one.";
        }
        return null;
    },
    next: "57-exit-status.html",
});
