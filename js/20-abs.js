import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Here's our first program that performs an actually meaningful computation: it calculates the absolute value of $n{x}. Here, $n{x} happens to be $v{-91}, but it would work for any value.",
    steps: [
        { code: "int x = -91;\n" },
        { code: "int abs_x;\n" },
        {
            code: "if (x >= 0) {\n",
            editable: true,
            instructions: "Select the line boundary where execution will resume.",
        },
        { code: "  abs_x = x;\n" },
        { code: "} else {\n" },
        {
            code: "  abs_x = -x;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "abs_x")) {
                    return "What is -(-91)?";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "}\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
    ],
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
    next: "21-cubed.html",
});
