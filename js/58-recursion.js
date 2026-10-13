import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A function can call itself. This is called recursion. It works like any other call: each call gets its own copies of the function's parameters and variables, which exist until that call returns.\nRecursion is useful when a problem can be broken into a smaller version of the same problem. For example, the factorial of $n{n} is $c{n * (n - 1) * ... * 1}, which is $n{n} times the factorial of $c{n - 1}.\nClick $runLineButton to continue.",
    steps: [
        { code: "int factorial(int n) {\n" },
        { code: "  if (n <= 1) {\n" },
        {
            code: "    return 1;\n",
            instructions: "This is the third call, so there are three copies of $n{n}; the other two are faded, because only the current call's variables can be used by name. This call doesn't call $c{factorial} again. Every recursive function needs a case like this, called a base case, or it would keep calling itself forever.\nNow each call returns to the one that called it, which can then finish computing $c{n * factorial(n - 1)} with its own $n{n}.",
        },
        { code: "  }\n" },
        { code: "  return n * factorial(n - 1);\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  int result = factorial(3);\n",
            editable: [true, false],
            instructions: [
                "What should the program state look like right after entering $c{factorial}? Also select where execution continues.",
                "Back in $c{main}, all three calls have returned, and their copies of $n{n} are gone.",
            ],
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("count") || ctx.basicHintTopicIs("name")) {
                    return "Calling $c{factorial} creates its parameter, $n{n}.";
                }
                return ctx.basicHint;
            },
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "59-recursive-sum.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
