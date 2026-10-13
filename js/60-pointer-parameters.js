import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Since parameters are copies, a function can't change its caller's variables directly. But it can if it's given their addresses.\nClick $runLineButton to continue.",
    steps: [
        { code: "void reset(int* p) {\n" },
        {
            code: "  *p = 0;\n",
            editable: true,
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "p")) {
                    return "$c{*p = 0;} changes the variable $n{p} points to, not $n{p}. Click $resetButton and try again.";
                }
                return ctx.basicHint;
            },
        },
        { code: "}\n\n" },
        { code: "void increase(int* target, int amount) {\n" },
        {
            code: "  *target += amount;\n",
            editable: true,
            hints: (ctx) => ctx.basicHint,
        },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int score = 7;\n" },
        {
            code: "  reset(&score);\n",
            instructions: [
                "$n{p} is a copy of $c{&score}: $n{score}'s address. Use $showAliasesButton to see $n{score}'s other name.",
                "The parameter $n{p} is gone, but the change it made through $n{*p} remains.",
            ],
        },
        { code: "  score = 4;\n" },
        {
            code: "  increase(&score, 3);\n",
            editable: [true, false],
            instructions: ["What should the program state look like right after entering $c{increase}? Also select where execution continues."],
            hints: (ctx) => {
                if (ctx.basicHintTopicIs("value", "target")) {
                    return "$n{target} starts as a copy of $c{&score}, which is $n{score}'s address.";
                }
                return ctx.basicHint;
            },
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "61-swap.html",
    workspace: { allowVariableCreation: true, allowVariableDeletion: true, showOtherNames: true },
});
