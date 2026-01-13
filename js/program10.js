"use strict";
/// <reference path="./shared-program-template.ts" />
{
    const createProgramTemplate = window.MB.createProgramTemplate;
    createProgramTemplate({
        initialInstructions: "Click $runLineButton to continue.",
        steps: [
            {
                code: "int a = 5 + 2 * (3 - 1);\n",
                editable: false,
                instructions: "The order of operations you know from math class (PEMDAS) applies, and isn't affected by spacing.",
            },
            {
                code: "a = 1-3 * 4;\n",
                editable: true,
                instructions: "The order of operations you know from math class (PEMDAS) applies, and isn't affected by spacing.",
                hints: (ctx) => {
                    const a = ctx.getBoxByName(ctx.boxes, "a");
                    if (a.value === "-8") {
                        return "You're parsing it as $c{(1 - 3) * 4}, but it should be parsed as $c{1 - (3 * 4)}, since multiplication has higher precedence than subtraction.";
                    }
                    if (a.value !== "-11") {
                        return "Multiply before subtracting, then update $n{a}.";
                    }
                    return null;
                },
            },
            {
                code: "a = (1 - 3) * 4;\n",
                editable: true,
                instructions: "The order of operations you know from math class (PEMDAS) applies, and isn't affected by spacing.",
                hints: (ctx) => {
                    const a = ctx.getBoxByName(ctx.boxes, "a");
                    if (a.value !== "-8") {
                        return "Evaluate the parentheses first, then multiply, then update $n{a}.";
                    }
                    return null;
                },
            },
            { code: "int b = 9 / 3;\n", editable: false },
            {
                code: "a = 5 / 3;\n",
                editable: false,
                instructions: "Integer division rounds towards 0, i.e. it drops everything after the decimal point.",
            },
            {
                code: "a = -7 / 2;\n",
                editable: false,
                instructions: "Integer division rounds towards 0, i.e. it drops everything after the decimal point.",
            },
            {
                code: "a = 1/2 + 1/2;\n",
                editable: false,
                instructions: "Rounding happens at the division, so both 1/2s round to 0, so this becomes $c{a = 0 + 0;}, which is 0.",
            },
            {
                code: "a = 8 / -(2 + 1);\n",
                editable: true,
                instructions: "Integer division rounds towards 0, i.e. it drops everything after the decimal point.",
                hints: (ctx) => {
                    const a = ctx.getBoxByName(ctx.boxes, "a");
                    if (a.value !== "-2") {
                        return "Compute the parentheses first, then negate, then divide, then update $n{a}.";
                    }
                    return null;
                },
            },
            {
                code: "int c = b+1 == 4;\n",
                editable: false,
                instructions: '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".',
            },
            {
                code: "int d = b == 58;\n",
                editable: false,
                instructions: '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".',
            },
            {
                code: "int e = 11/3 == 3;\n",
                editable: true,
                instructions: '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".',
                hints: (ctx) => {
                    const e = ctx.getBoxByName(ctx.boxes, "e");
                    if (!e) {
                        return "Add the $n{e} variable.";
                    }
                    if (e.type !== "int") {
                        return "$n{e}'s type should be $t{int}.";
                    }
                    if (e.value === "11") {
                        return "You're parsing it as $c{11 / (3==3)}, but it should be parsed as $c{(11/3) == 3}. Division ($c{/}) has higher precedence than equality ($c{==}).";
                    }
                    if (e.value !== "1") {
                        return "Calculate $c{11 / 3}, then drop everything after the decimal point. Is that equal to 3?";
                    }
                    return null;
                },
            },
            {
                code: "int f = 9 / 2+1 == 3;\n",
                editable: true,
                instructions: '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".',
                hints: (ctx) => {
                    const f = ctx.getBoxByName(ctx.boxes, "f");
                    if (!f) {
                        return "Add the $n{f} variable.";
                    }
                    if (f.type !== "int") {
                        return "$n{f}'s type should be $t{int}.";
                    }
                    if (f.value === "1") {
                        return "You're probably parsing it as $c{(9 / (2+1)) == 3}, but it should be parsed as $c{((9/2) + 1) == 3}. Division has higher precedence than addition.";
                    }
                    if (f.value === "4") {
                        return "You're parsing it as $c{(9/2) + (1==3)}, but it should be parsed as $c{((9/2) + 1) == 3}. Division ($c{/}) has higher precedence than addition ($c{+}), which has higher precedence than equality ($c{==}).";
                    }
                    if (f.value !== "0") {
                        return "Evaluate division ($c{/}), then addition ($c{+}), then equality ($c{==}).";
                    }
                    return null;
                },
            },
            {
                code: "int g = 0 == 1 == 2;\n",
                editable: false,
                instructions: "$c{==} is left-associative, so this is parsed as $c{(0 == 1) == 2}.",
            },
            {
                code: "int h = (-2 / 3==1-1==1) - 3;\n",
                editable: true,
                instructions: "Remember that spacing doesn't affect order of operations.",
                hints: (ctx) => {
                    const h = ctx.getBoxByName(ctx.boxes, "h");
                    if (!h) {
                        return "Add the $n{h} variable.";
                    }
                    if (h.type !== "int") {
                        return "$n{h}'s type should be $t{int}.";
                    }
                    if (h.value !== "-2") {
                        return "The precedence order of the operations used in this line is, from highest to lowest: Parenthesis, division ($c{/}), subtraction ($c{-}), equality ($c{==}). Also recall that equality is left-associative, so $c{x == y == z} is parsed as $c{(x == y) == z}.";
                    }
                    return null;
                },
            },
        ],
        next: "program11.html",
        workspace: { allowVariableCreation: true },
    });
}
