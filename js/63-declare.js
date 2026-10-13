import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    outputs: [{ name: "result", type: "int" }],
    main: () => "int main(void) {\n  int result = twice(5) + half(9.0);\n  return 0;\n}\n\nint twice(int n) {\n  return 2 * n;\n}\n\nint half(double x) {\n  return x / 2;\n}",
    solve: "int twice(int n);\nint half(double x);",
    textareaMinLines: 4,
    instructions: "$c{main} calls $c{twice} and $c{half} before they're defined, so this doesn't compile. Fix it by adding declarations above $c{main}, without moving or copying the definitions.",
    hints: (ctx) => {
        if (ctx.currentResult.kind === "wrong-output-value") {
            return "Make sure each declaration's return type and parameter types match the definition.";
        }
        return null;
    },
    next: "64-function-errors.html",
});
