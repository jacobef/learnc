import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Until now, every program in this tutorial has secretly been wrapped in $c{int main(void) \\{ ... return 0; \\}}, and the tutorial hid that from you. From now on, programs will be written out in full, because they're going to contain more than one function.\nA function is a named piece of code that can be run from elsewhere in the program. $c{main} is a function too, and every C program starts running at the beginning of $c{main}.\nClick $runLineButton to continue.",
    steps: [
        { code: "int square(int n) {\n" },
        { code: "  int result = n * n;\n", editable: [false, true] },
        { code: "  return result;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  int a = 3;\n",
            instructions: "Execution started here, at the beginning of $c{main}, skipping right past the definition of $c{square} above. Defining a function doesn't run it.",
        },
        { code: "  int b;\n" },
        {
            code: "  b = square(a);\n",
            instructions: [
                "This is a function call: it runs $c{square}. First, a new variable is created for each of $c{square}'s parameters, here $n{n}, holding the value passed in, called the argument. Here the argument is $c{a}, so $n{n} starts as $v{3}.\nWhile $c{square} runs, $c{main}'s variables still exist, but their names can't be used, so they're shown faded.",
                "When a function returns, its variables are removed, and the call expression, $c{square(a)}, evaluates to the returned value.",
            ],
        },
        {
            code: "  b = square(b + 1);\n",
            editable: [true, false],
            instructions: [
                "What should the program state look like right after entering $c{square}? Also select the line boundary where execution continues.",
                "",
            ],
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("count") || ctx.basicHintTopicIs("name")) {
                    return "Calling $c{square} creates its parameter, $n{n}.";
                }
                if (ctx.basicHintTopicIs("value", "n")) {
                    return "$n{n} starts as the argument's value: $c{b + 1}, where $n{b} is $c{main}'s variable.";
                }
                return ctx.basicHint;
            },
        },
        {
            code: "  return 0;\n",
            instructions: "When $c{main} returns, the program ends. Returning $v{0} from $c{main} tells whoever ran the program that it finished successfully.",
        },
        { code: "}\n" },
    ],
    next: "55-functions-ii.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
