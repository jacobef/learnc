import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    initialInstructions: "Arrays and loops work well together: a loop can use its loop variable as an index, to visit every element of an array.\nClick $runLineButton to continue.",
    steps: [
        { code: "int scores[4] = {7, 3, 9, 5};\n" },
        { code: "int total = 0;\n" },
        {
            code: "for (int i = 0; i < 4; i++) {\n",
            instructions: [
                "Notice that the condition is $c{i < 4}, not $c{i <= 4}. $n{scores} has 4 elements, so its last index is 3.",
            ],
        },
        {
            code: "  total += scores[i];\n",
            editable: [false, true, false, true],
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "total")) {
                    return "$c{scores[i]} is the element of $n{scores} whose index is $n{i}'s value.";
                }
                return ctx.basicHint;
            },
        },
        { code: "}\n" },
        { code: "int best = scores[0];\n" },
        {
            code: "for (int i = 1; i < 4; i++) {\n  if (scores[i] > best) {\n    best = scores[i];\n  }\n}\n",
            editable: true,
            instructions: "This loop finds the largest element of $n{scores}. What should the program state look like after it finishes?",
            hints: (ctx) => {
                if (ctx.boxNamed("i")) {
                    return "$n{i} was created in the loop's setup, so it should be removed when the loop finishes.";
                }
                if (ctx.basicHintTopicIs("value", "best")) {
                    return "$n{best} starts as $c{scores[0]}, and is replaced every time the loop finds a larger element.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "for (int i = 0; i < 4; i++) {\n  scores[i] *= 2;\n}\n",
            editable: true,
            instructions: "What should the program state look like after this loop finishes?",
            hints: (ctx) => {
                if (ctx.boxNamed("i")) {
                    return "$n{i} was created in the loop's setup, so it should be removed when the loop finishes.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "80-array-sum.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
