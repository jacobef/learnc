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
        {
            code: `while (fruit <= 2) {
  feedback = 2 * feedback;
  fruit = fruit + 1;
}
`,
            editable: true,
            instructions: "What should the program state look like after this $c{while} loop finishes?",
            hints: (ctx) => {
                const [feedback, fruit] = ctx.boxesNamed("feedback", "fruit");
                if (!feedback || !fruit)
                    return ctx.basicHint;
                if (ctx.basicHintTopicIs("value", "feedback") || ctx.basicHintTopicIs("value", "fruit")) {
                    if (feedback.value === "-4" && fruit.value === "2") {
                        return "The $c{while} loop runs one more time than that; the condition is $c{fruit <= 2}, not $c{fruit < 2}.";
                    }
                    if (feedback.value === "3" && fruit.value === "-8") {
                        return "You have the values for $n{feedback} and $n{fruit} flipped.";
                    }
                    if (feedback.value === "8" && fruit.value === "3") {
                        return "Almost, but $n{feedback} should be $v{-8}, not $v{8}; multiplying by 2 shouldn't change its sign.";
                    }
                    if (feedback.value === "-8") {
                        return "Your value for $n{feedback} is correct, but your value for $n{fruit} isn't.";
                    }
                    if (fruit.value === "3") {
                        return "Your value for $n{fruit} is correct, but your value for $n{feedback} isn't. $n{feedback} should get doubled every time the loop runs.";
                    }
                    return "I'm not sure where you're getting those values for $n{feedback} and $n{fruit} from.";
                }
                return ctx.basicHint;
            },
        },
    ],
    next: "29-fibonacci.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
