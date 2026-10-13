import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A few more ways to control loops.\nClick $runLineButton to continue.",
    steps: [
        { code: "int main(void) {\n" },
        { code: "  int n = 0;\n" },
        {
            code: "  do {\n",
            instructions: "A $c{do}-$c{while} loop checks its condition after each run of its block, instead of before. So the block always runs at least once.",
        },
        { code: "    n += 3;\n" },
        {
            code: "  } while (n < 2);\n",
            instructions: "The condition is checked now, after the block has run once. It's false, so the loop ends.",
        },
        { code: "  int total = 0;\n" },
        {
            code: "  for (int i = 1; i <= 10; i++) {\n",
        },
        {
            code: "    if (i % 2 == 0) {\n",
            instructions: [
                "",
                "Now $n{i} is $v{2}, so this $c{if}'s block runs. The $c{continue} in it skips the rest of the loop's block and goes on to the next run of the loop. In a $c{for} loop, that means the update, $c{i++}, then the condition.",
            ],
        },
        { code: "      continue;\n" },
        { code: "    }\n" },
        {
            code: "    if (total > 10) {\n",
            instructions: [
                "",
                "",
                "",
                "",
                "Now $n{total} is more than $v{10}, so this $c{if}'s block runs. The $c{break} in it ends the loop immediately, without checking the condition.",
            ],
        },
        { code: "      break;\n" },
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
            instructions: "Which odd numbers got added before the loop broke?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "result")
                ? "$c{1 + 3 + 5 + 7} is $v{16}, which is more than $v{10}, so the loop broke when $n{i} was $v{9}."
                : ctx.basicHint,
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "119-switch.html",
    workspace: { allowVariableCreation: true },
});
