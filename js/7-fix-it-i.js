import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    outputs: [
        { name: "apple", type: "int" },
        { name: "pear", type: "int" },
    ],
    startCode: "int apple = 3\nint pear;\npear = apple;\napple = 7\nplum = 2;\n",
    solve: "int apple = 3;\nint pear;\npear = apple;\napple = 7;\n",
    textareaMinLines: 6,
    instructions: "This code should end with $n{apple} equal to $v{7} and $n{pear} equal to $v{3}, but it has several compile errors. Fix them.\nThe compiler stops at the first error it finds, so after you fix one, the next one will show up.",
    hints: (ctx) => ctx.currentResult.kind === "wrong-output-value"
        ? "The code compiles now, but the values aren't right yet. Look at what each line does."
        : null,
    next: "8-assignment-ii.html",
});
