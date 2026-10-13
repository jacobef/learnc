import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A few more ways to control loops.\nClick $runLineButton to continue.",
    steps: [
        { code: "int main(void) {\n" },
        { code: "  int n = 0;\n" },
        { code: "  do {\n" },
        {
            code: "    n += 3;\n",
            instructions: "A $c{do}-$c{while} loop checks its condition after each run of its block, instead of before. So the block always runs at least once, without checking anything first. The $c{do} line itself doesn't do anything; it just marks where the block starts.",
        },
        {
            code: "  } while (n < 2);\n",
            editable: true,
            instructions: "Now the condition is checked, after the block has run once. Select where execution continues.",
        },
        { code: "  int total = 0;\n" },
        { code: "  for (int i = 1; i <= 10; i++) {\n" },
        { code: "    if (i % 2 == 0) {\n" },
        {
            code: "      continue;\n",
            instructions: [
                "$c{continue} skips the rest of the loop's block and goes on to the next run of the loop. In a $c{for} loop, that means the update, $c{i++}, and then the condition.",
                "",
            ],
        },
        { code: "    }\n" },
        { code: "    if (total > 10) {\n", editable: [false, false, false, false, true] },
        {
            code: "      break;\n",
            instructions: "$c{break} ends the loop immediately, without running the update or checking the condition.",
        },
        { code: "    }\n" },
        {
            code: "    total += i;\n",
            editable: [false, true, false, true],
            hints: basic,
        },
        { code: "  }\n" },
        {
            code: "  int result = total;\n",
            editable: true,
            instructions: "Which odd numbers got added before the loop ended?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "result")
                ? "$c{1 + 3 + 5 + 7} is $v{16}, which is more than $v{10}, so the loop broke when $n{i} was $v{9}."
                : ctx.basicHint,
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "119-goto.html",
    workspace: { allowVariableCreation: true },
});
