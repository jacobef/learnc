"use strict";
/// <reference path="./shared-program-template.ts" />
{
    const createProgramTemplate = window.MB.createProgramTemplate;
    createProgramTemplate({
        initialInstructions: "No instructions for this one. Good luck!",
        steps: [
            {
                code: "int yin = 2;\n",
                editable: false,
            },
            { code: "int yang = 5;\n", editable: false },
            { code: "yin = yang + yin;\n", editable: false },
            {
                code: "yang = yang + 1;\n",
                editable: true,
                hints: (ctx) => {
                    if (ctx.basicHintTopicIs("value", "yang")) {
                        return "Take the current value of $n{yang}, then add 1. That result should be the new value of $n{yang}.";
                    }
                    return ctx.basicHint;
                },
            },
            {
                code: "yin = yin;\n",
                editable: true,
                hints: () => "This line literally means \"set $n{yin}'s value to $n{yin}'s current value\", so nothing should change.",
            },
            {
                code: "yang = yang-yang * 2;\n",
                editable: true,
                hints: (ctx) => {
                    if (ctx.basicHintTopicIs("value", "yang")) {
                        const yang = ctx.boxNamed("yang");
                        if (yang.value === "0") {
                            return "Haha, gotcha. Always remember order of operations.";
                        }
                        return "Multiply before subtracting, and update $n{yang} accordingly.";
                    }
                    return ctx.basicHint;
                },
            },
        ],
        next: "program12.html",
        workspace: { allowVariableCreation: true },
    });
}
