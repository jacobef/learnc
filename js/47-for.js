import { createProgramTemplate } from "./shared-program-template.js";
const updateRule = "When the end of a $c{for} loop's block is reached, the update ($c{i++}) runs. Then execution goes back to the condition, just like in a $c{while} loop.";
createProgramTemplate({
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        { code: "int total = 0;\n" },
        { code: "int i;\n" },
        {
            code: "i = 1;\nwhile (i <= 3) {\n  total += i;\n  i++;\n}\n",
            instructions: "In $i{45. Sum} and $i{46. Factorial}, you probably wrote a loop like this one. A single variable ($n{i}) controls the loop, and it's handled in three places: it's set up before the loop (line 3), checked by the loop's condition (line 4), and updated at the end of the loop's block (line 6).\nThis pattern is so common that C has a shortcut for it, the $c{for} loop. Lines 8-10 do exactly the same thing as lines 3-7.",
        },
        {
            code: "for (i = 1; i <= 3; i++) {\n",
            editable: [false, false, true, true],
            instructions: [
                "A $c{for} loop's header has three parts, separated by semicolons: a setup ($c{i = 1}), a condition ($c{i <= 3}), and an update ($c{i++}).\nThe first time the header is reached, the setup runs, and then the condition is checked, just like a $c{while} loop's condition.",
                "After the first time, only the condition is checked. The setup doesn't run again.",
                "Select the line boundary where execution will resume.",
                "Select the line boundary where execution will resume.",
            ],
        },
        {
            code: "  total += i;\n",
            editable: [false, false, true],
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "}\n",
            editable: [false, true, false],
            instructions: [updateRule, updateRule],
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "i")) {
                    return "The update, $c{i++}, runs at the end of the block.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "for (int j = 10; j > 0; j -= 4) {\n  total--;\n}\n",
            editable: true,
            instructions: "The setup can also create a variable, like $c{int j = 10}. A variable created in the setup is removed when the loop finishes.\nWhat should the program state look like after this $c{for} loop finishes?",
            hints: (ctx) => {
                if (ctx.boxNamed("j")) {
                    return "$n{j} was created in the loop's setup, so it should be removed when the loop finishes.";
                }
                if (ctx.basicHintTopicIs("value", "total")) {
                    const total = ctx.boxNamed("total");
                    if (total.value === "10") {
                        return "The loop runs one more time than that. $n{j} takes the values $v{10}, $v{6}, and $v{2} while the block runs.";
                    }
                    return "Each time the block runs, $n{total} decreases by 1. How many times does it run? Keep track of $n{j}: it starts at $v{10} and decreases by 4 after each run of the block.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "for (i = 0; i < 5; i += 2) {\n  total++;\n}\n",
            editable: true,
            instructions: "What should the program state look like after this $c{for} loop finishes?\nIf you ever forget how a $c{for} loop works, the $referenceButton shows the $c{while} loop it's equivalent to.",
            hints: (ctx) => {
                const i = ctx.boxNamed("i");
                if (!i) {
                    return "$n{i} was created before the loop, not in its setup, so it shouldn't be removed.";
                }
                if (i.value === "4") {
                    return "The update ($c{i += 2}) runs at the end of every run of the block, including the last one. The loop only stops once the condition ($c{i < 5}) is checked and found to be zero.";
                }
                if (ctx.basicHintTopicIs("value", "i")) {
                    return "Keep track of $n{i}: it starts at $v{0} and increases by 2 after each run of the block. The loop stops once $c{i < 5} is zero.";
                }
                if (ctx.basicHintTopicIs("value", "total")) {
                    return "Each time the block runs, $n{total} increases by 1. How many times does it run?";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "48-power.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
