import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Feel free to use a calculator if it helps, but blindly copying from it will not always give you the right answer here.",
    steps: [
        {
            code: "int a = 5 + 2 * (3 - 1);\n",
            editable: false,
            instructions: "Order of operations, from highest to lowest: parenthesis, then multiplication and division, then addition and subtraction.",
        },
        {
            code: "a = 1-3 * 4;\n",
            editable: true,
            instructions: "Order of operations, from highest to lowest: parenthesis, then multiplication and division, then addition and subtraction.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a")) {
                    const a = ctx.boxNamed("a");
                    if (a.value === "-8") {
                        return "You're parsing it as $c{(1-3) * 4}, but it should be parsed as $c{1-(3 * 4)}, since multiplication has higher precedence than subtraction.";
                    }
                    return "Multiply before subtracting, then update $n{a}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "a = (1 - 3) * 4;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a")) {
                    return "Evaluate the parentheses first, then multiply, then update $n{a}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int b = 12 / 4;\n",
            editable: false,
        },
        {
            code: "a = 5 / 3;\n",
            editable: false,
            instructions: "Division drops the decimal part. 5 divided by 3 is 1.666..., which becomes $v{1}.",
        },
        {
            code: "a = -7 / 2;\n",
            editable: false,
            instructions: "Division drops the decimal part. -7 divided by 2 is -3.5, which becomes $v{-3}.\n\nTrivia, feel free to ignore: When used as negation (e.g. $c{-7}) as opposed to subtraction (e.g. $c{9-8}), the $c{-} operator has higher precedence than multiplication and division, so this is parsed as $c{(-7) / 2}, not $c{-(7 / 2)}. In this case it doesn't affect the final result, but in very rare cases that we'll encounter in much later programs, it does.",
        },
        {
            code: "a = 1/2 + 3/4;\n",
            editable: false,
            instructions: "The decimal part is dropped by each division itself, not at the end. Both $c{1/2} (0.5) and $c{3/4} (0.75) immediately round to 0, so this becomes $c{a = 0 + 0;}.",
        },
        {
            code: "a = 8 / -(2 + 1);\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "a")) {
                    return "Compute the parentheses first, then negate, then divide, then update $n{a}.";
                }
                return ctx.basicHint;
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
                if (ctx.basicHintTopicIs("value", "e")) {
                    const e = ctx.boxNamed("e");
                    if (e.value === "11") {
                        return "You're parsing it as $c{11/(3 == 3)}, but it should be parsed as $c{(11/3) == 3}. Division ($c{/}) has higher precedence than equality ($c{==}).";
                    }
                    return "Calculate $c{11 / 3}, then drop the decimal part. Is that equal to 3?";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "int f = 9 / 2+1 == 3;\n",
            editable: true,
            instructions: '$c{x == y} evaluates to 1 if x and y have equal values, and 0 if they don\'t. $c{==} has lower precedence than addition and subtraction.\n\nThe name of this operation is "equality".',
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "f")) {
                    const f = ctx.boxNamed("f");
                    if (f.value === "1") {
                        return "You're probably parsing it as $c{(9 / (2+1)) == 3}, but it should be parsed as $c{((9 / 2)+1) == 3}. Division has higher precedence than addition.";
                    }
                    if (f.value === "4") {
                        return "You're parsing it as $c{(9 / 2)+(1 == 3)}, but it should be parsed as $c{((9 / 2)+1) == 3}. Division ($c{/}) has higher precedence than addition ($c{+}), which has higher precedence than equality ($c{==}).";
                    }
                    return "Evaluate division ($c{/}), then addition ($c{+}), then equality ($c{==}).";
                }
                return ctx.basicHint;
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
                if (ctx.basicHintTopicIs("value", "h")) {
                    return "The precedence order of the operations used in this line is, from highest to lowest: Parenthesis, division ($c{/}), subtraction ($c{-}), equality ($c{==}). Also recall that equality is left-associative, so $c{x == y == z} is parsed as $c{(x == y) == z}.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "h = 9/2/2-3/7;\n",
            editable: true,
            instructions: "Division is left-associative.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "h")) {
                    const h = ctx.boxNamed("h");
                    if (h.value === "9") {
                        return "You're parsing it as $c{(9/(2/2))-(3/7)}, but division is left-associative, not right-associative, so it should be parsed as $c{((9/2)/2)-(3/7)}.";
                    }
                    if (h.value === "0") {
                        return "You're probably parsing it as $c{((9/2)/(2-3))/7}, but division has higher precedence than subtraction, so it should be parsed as $c{((9/2)/2)-(3/7)}.";
                    }
                    if (h.value === "1") {
                        return "You're probably removing the decimal part at the end, but you should instead remove it after each division. Go back to line 7 if you're confused.";
                    }
                    if (h.value === "3") {
                        return "After each division, you're probably rounding to the nearest integer, but you should be removing the decimal part instead. Go back to line 5 if you're confused.";
                    }
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "program11.html",
    workspace: { allowVariableCreation: true },
});
