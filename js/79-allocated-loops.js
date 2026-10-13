import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Since an index can be any $t{int} expression, a loop can use its loop variable as an index, to visit every element of an array. And since $c{malloc} is called while the program runs, the array's size doesn't have to be known in advance: it can be any value computed by the program.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdlib.h>\n" },
        { code: "int n = 4;\n" },
        { code: "int* squares = malloc(n * sizeof(int));\n" },
        { code: "for (int i = 0; i < n; i++) {\n" },
        {
            code: "  squares[i] = i * i;\n",
            editable: [false, true, false, true],
            hints: (ctx) => ctx.basicHint,
        },
        { code: "}\n" },
        { code: "int total = 0;\n" },
        {
            code: "for (int i = 0; i < n; i++) {\n  total += squares[i];\n}\n",
            editable: true,
            instructions: "What should the program state look like after this whole loop runs?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "total") ? "$n{total} adds up all of the elements: $c{0 + 1 + 4 + 9}." : ctx.basicHint,
        },
    ],
    next: "80-array-sum.html",
});
