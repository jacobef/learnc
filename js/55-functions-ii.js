import { createProgramTemplate } from "./shared-program-template.js";
const callHint = (params) => (ctx) => {
    if (ctx.basicHintTopicIs("count") || ctx.basicHintTopicIs("name")) {
        return `The call creates the function's parameters: ${params}.`;
    }
    if (ctx.basicHintTopicIs("value")) {
        return "Each parameter starts as the value of the matching argument, in order.";
    }
    return ctx.basicHint;
};
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Click $runLineButton to continue.",
    steps: [
        { code: "void bump(int n) {\n" },
        {
            code: "  n = n + 1;\n",
            editable: true,
            instructions: "Watch carefully: does this change $c{main}'s $n{count}?",
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "count")) {
                    return "$n{n} is $c{bump}'s own variable. It started with a copy of $n{count}'s value, but it's a separate variable, so changing it doesn't change $n{count}.";
                }
                return ctx.basicHint;
            },
        },
        { code: "}\n\n" },
        { code: "int add(int x, int y) {\n" },
        { code: "  int sum = x + y;\n" },
        { code: "  return sum;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int count = 5;\n" },
        {
            code: "  bump(count);\n",
            instructions: [
                "$c{bump}'s return type is $t{void}, which means it doesn't return a value. It's called just for what it does, so its call isn't used in a larger expression.",
                "$c{bump} changed its parameter, but not $n{count}. Parameters are copies of the arguments, so a function can't change its caller's variables through its parameters.",
            ],
        },
        { code: "  int total;\n" },
        {
            code: "  total = add(count, 2);\n",
            editable: [true, false],
            instructions: [
                "A function can have several parameters, separated by commas. What should the program state look like right after entering $c{add}? Also select where execution continues.",
            ],
            hints: callHint("$n{x} and $n{y}"),
        },
        {
            code: "  total = add(add(1, 2), total);\n",
            editable: [false, false, true, false],
            instructions: [
                "To call the outer $c{add}, its arguments have to be evaluated first, and one of them is another call to $c{add}. So the inner call happens first.",
                "",
                "Now the outer call can happen. What should the program state look like right after entering $c{add} again, and where does execution continue?",
            ],
            hints: callHint("$n{x} and $n{y}"),
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "56-max-function.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
