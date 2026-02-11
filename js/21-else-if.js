import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "The first $c{if} / $c{else if} block with a nonzero expression will run. If all the expressions are zero, the $c{else} block will run.\nIf you know Python, $c{else if} is C's equivalent to Python's $c{elif}.",
    steps: [
        { code: "int this = 78;\n" },
        { code: "if (this < 60) {\n" },
        { code: "  this = 1;\n" },
        { code: "} else if (this < 70) {\n" },
        { code: "  this = 2;\n" },
        { code: "} else if (this < 80) {\n" },
        { code: "  this = 3;\n" },
        { code: "} else if (this < 90) {\n" },
        { code: "  this = 4;\n" },
        { code: "} else {\n" },
        { code: "  this = 5;\n" },
        { code: "}\n" },
        { code: "int that = 0;\n" },
        {
            code: `if (that) {
  this = this + that;
} else if (this == that) {
  this = this - that;
} else {
  this = this * that;
}
`,
            editable: true,
            instructions: "What should the program state look like after this entire statement is run?",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "this")) {
                    return "$c{that} is 0, and $c{this == that} is also 0, so the $c{else} block is the one that should run.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "22-review-ii.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
