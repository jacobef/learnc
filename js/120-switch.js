import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A $c{switch} statement is like a $c{goto} that picks its label based on an integer's value. Inside its block, $c{case} labels mark places to jump to, one for each value, and an optional $c{default} label catches every other value.\nClick $runLineButton to continue.",
    steps: [
        { code: "int main(void) {\n" },
        { code: "  int day = 7;\n" },
        { code: "  int weekend = 0;\n" },
        {
            code: "  switch (day) {\n",
            editable: true,
            instructions: "Execution jumps to just after the $c{case} label matching $n{day}'s value. Select where it continues.",
        },
        { code: "    case 1:\n" },
        { code: "    case 2:\n" },
        { code: "    case 3:\n" },
        { code: "    case 4:\n" },
        { code: "    case 5:\n" },
        { code: "      weekend = 0;\n" },
        { code: "      break;\n" },
        { code: "    case 6:\n" },
        { code: "    case 7:\n" },
        { code: "      weekend = 1;\n" },
        {
            code: "      break;\n",
            instructions: "$c{break} leaves the $c{switch}. Labels are just places to jump to, like $c{goto}'s labels; they don't end anything. So without a $c{break}, execution would keep going right into the next $c{case}'s code. This is called falling through.\nThat's how cases 1 to 5 share code: a $n{day} of $v{1} would jump to just after $c{case 1:}, and then run through the other labels to $c{weekend = 0;}. But forgetting a $c{break} is a classic bug.",
        },
        { code: "    default:\n" },
        { code: "      weekend = -1;\n" },
        { code: "  }\n" },
        { code: "  int code = 0;\n" },
        {
            code: "  switch (weekend) {\n    case 0:\n      code = 10;\n    case 1:\n      code += 20;\n    default:\n      code += 30;\n  }\n",
            editable: true,
            instructions: "What should the program state look like after this $c{switch}? Watch out: there are no $c{break}s.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "code")
                ? "Execution jumps to just after $c{case 1:}, then falls through into the $c{default:} code too."
                : ctx.basicHint,
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "121-conditional-operator.html",
});
