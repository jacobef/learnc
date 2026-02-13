import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "A $c{while} loop will run repeatedly until its expression is zero.",
    steps: [
        { code: "int fruit = 2;\n" },
        { code: "while (fruit) {\n" },
        { code: "  int belt = fruit - 1;\n" },
        { code: "  fruit = belt;\n" },
        { code: "}\n" },
        { code: "int feedback = 3;\n" },
        {
            code: "while (feedback >= 0) {\n",
            editable: true,
            instructions: "Select the line boundary where execution will resume.",
        },
        {
            code: "  feedback = feedback - fruit - 2;\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "}\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
    ],
    next: "sandbox.html?finished=1",
    isLast: true,
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
