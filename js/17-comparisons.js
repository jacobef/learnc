import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        {
            code: "int x = 2 != 3;\n",
            editable: false,
            instructions: "$c{!=} is the opposite of $c{==}.",
        },
        {
            code: "x = x != 0;\n",
            editable: true,
        },
        {
            code: "x = 2 > 2;\n",
            editable: true,
            instructions: "$c{x > y} evaluates to 1 if $n{x}'s value is greater than $n{y}'s, and 0 otherwise.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "x")) {
                    return "2 is not greater than 2.";
                }
            },
        },
        {
            code: "x = 2 >= 2;\n",
            editable: true,
            instructions: "$c{x >= y} evaluates to 1 if $n{x}'s value is greater than or equal to $n{y}'s, and 0 otherwise.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "x")) {
                    return "2 is equal to 2.";
                }
            },
        },
        {
            code: "x = (x > x) - (x >= x) + (x < x) - (x <= x);\n",
            editable: true,
            instructions: "You can probably guess what $c{<} and $c{<=} do.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "x")) {
                    return "$n{x}'s value isn't greater than itself, nor is it less than itself, but it is equal to itself.";
                }
            },
        },
        {
            code: "x = (2 < 9 + 8 != 1)\n+ 2 * (0 < 7 == 3 < 3)\n+ 4 * (3 - 1 < 5 == 1);\n",
            editable: true,
            instructions: "This one is hard and not super important. If you'd rather skip it, feel free to jump to 18 in the sidebar.\n\n$c{>}, $c{>=}, $c{<}, and $c{<=} have higher precedence than $c{==} and $c{!=}, but lower precedence than addition and subtraction.",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "x")) {
                    const x = ctx.boxNamed("x");
                    if (x.value === "10") {
                        return "You're parsing it as if the precedence order was, highest to lowest: [$c{< <= > >=}], [$c{== !=}], [$c{+ -}]. For example, you've parsed line 6 as $c{(2 < 9) + (8 != 1)}. But the order should be: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 6 should be parsed as $c{(2 < (9 + 8)) != 1}.";
                    }
                    if (x.value === "2") {
                        return "You're parsing it as if the precedence order was, highest to lowest: [$c{+ -}], [$c{== !=}], [$c{< <= > >=}]. For example, you've parsed line 6 as $c{2 < ((9 + 8) != 1)}. But the order should be: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 6 should be parsed as $c{(2 < (9 + 8)) != 1}.";
                    }
                    if (x.value === "16") {
                        return "You're parsing it as if the precedence order was, highest to lowest: [$c{== !=}], [$c{< <= > >=}], [$c{+ -}]. For example, you've parsed line 6 as $c{(2 < 9) + (8 != 1)}. But the order should be: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 6 should be parsed as $c{(2 < (9 + 8)) != 1}.";
                    }
                    if (x.value === "6") {
                        return "You're parsing it as if [$c{< <= > >=}] and [$c{== !=}] have the same precedence (both below [$c{+ -}]). For example, you've parsed line 7 as $c{((0 < 7) == 3) < 3}. But the order should be, highest to lowest: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 7 should be parsed as $c{(0 < 7) == (3 < 3)}.";
                    }
                    if (x.value === "7") {
                        return "You're parsing it as if [$c{+ -}], [$c{< <= > >=}], and [$c{== !=}] all have the same precedence. For example, you've parsed line 6 as $c{((2 < 9) + 8) != 1}. But the order should be, highest to lowest: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 6 should be parsed as $c{(2 < (9 + 8)) != 1}.";
                    }
                    if (x.value === "12") {
                        return "You're parsing it as if [$c{< <= > >=}] and [$c{== !=}] have the same precedence (both above [$c{+ -}]). For example, you've parsed line 6 as $c{(2 < 9) + (8 != 1)}. But the order should be, highest to lowest: [$c{+ -}], [$c{< <= > >=}], [$c{== !=}]. So, for example, line 6 should be parsed as $c{(2 < (9 + 8)) != 1}.";
                    }
                    if (x.value === "0") {
                        return "You're probably only looking at line 6. Lines 7 and 8 are part of the statement too.";
                    }
                    if (x.value === "") {
                        return "$n{x}'s value shouldn't be empty.";
                    }
                    return `I'm not sure where you're getting $v{${x.value}} from.`;
                }
            },
        },
    ],
    next: "18-if.html",
    workspace: { allowVariableCreation: false, allowVariableDeletion: false },
});
