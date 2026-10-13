import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "n", type: "int" }],
    outputs: [{ name: "total", type: "int" }],
    testInputs: Array.from({ length: 11 }, (_, n) => [String(n)]),
    startInput: ["4"],
    startCode: "int total = 0\nInt i = 1;\nwhile (i <= n) {\n  total = total + i;\n  i = i + 1;\n}\ntotl = total * 2;\n",
    solve: "int total = 0;\nint i = 1;\nwhile (i <= n) {\n  total = total + i;\n  i = i + 1;\n}\ntotal = total * 2;\n",
    textareaMinLines: 8,
    instructions: "This code is supposed to set $n{total} to twice $c{1 + 2 + ... + n}. But it has several compile errors. Fix them.\nThe compiler only reports one error at a time, so after you fix one, the next one will show up.",
    hints: (ctx) => ctx.currentResult.kind === "wrong-output-value"
        ? "The code compiles now, but $n{total} isn't right yet. Look at what each line does."
        : null,
    next: "43-short-circuiting.html",
});
