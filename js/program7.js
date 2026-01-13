"use strict";
/// <reference path="./shared-program-template.ts" />
{
    const createProgramTemplate = window.MB.createProgramTemplate;
    createProgramTemplate({
        initialInstructions: "This is where addresses become relevant.\nNo other instructions for this one. Good luck!",
        steps: [
            { code: "int deer;\n", editable: false },
            { code: "int hare;\n", editable: false },
            { code: "int* wolf;\n", editable: false },
            { code: "wolf = &deer;\n", editable: false },
            {
                code: "wolf = &hare;\n",
                editable: true,
                hints: (ctx) => {
                    const [wolf, hare] = ctx.getBoxesByName(ctx.boxes, "wolf", "hare");
                    if (wolf.value !== hare.address) {
                        return "$c{wolf = &deer;} set $n{wolf}'s value to $n{deer}'s address. What should $c{wolf = &hare;} do?";
                    }
                    return null;
                },
            },
            {
                code: "int** bear = &wolf;\n",
                editable: true,
                hints: (ctx) => {
                    const [bear, wolf] = ctx.getBoxesByName(ctx.boxes, "bear", "wolf");
                    if (!bear) {
                        return "You need to add the $n{bear} variable.";
                    }
                    if (bear.type !== "int**") {
                        return "$n{bear}'s type should be $t{int**}.";
                    }
                    if (bear.value !== wolf.address) {
                        return "Set $n{bear}'s value to $n{wolf}'s address.";
                    }
                    return null;
                },
            },
            {
                code: "int* fox = wolf;\n",
                editable: true,
                hints: (ctx) => {
                    const [fox, wolf] = ctx.getBoxesByName(ctx.boxes, "fox", "wolf");
                    if (!fox) {
                        return "You need to add the $n{fox} variable.";
                    }
                    if (fox.type !== "int*") {
                        return "$n{fox}'s type should be $t{int*}.";
                    }
                    if (fox.value !== wolf.value) {
                        return "$n{fox}'s value should be set to $n{wolf}'s value.";
                    }
                    return null;
                },
            },
        ],
        next: "program8.html",
        workspace: { allowVariableCreation: true },
    });
}
