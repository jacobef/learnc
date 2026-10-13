import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "A $c{switch} statement jumps to one of several places, depending on an integer's value. Each place is marked with a $c{case} label, and an optional $c{default} label catches every other value.\nClick $runLineButton to continue.",
  steps: [
    { code: "int main(void) {\n" },
    { code: "  int day = 7;\n" },
    { code: "  int weekend = 0;\n" },
    { code: "  switch (day) {\n" },
    { code: "    case 1:\n" },
    { code: "    case 2:\n" },
    { code: "    case 3:\n" },
    { code: "    case 4:\n" },
    { code: "    case 5:\n" },
    { code: "      weekend = 0;\n" },
    { code: "      break;\n" },
    { code: "    case 6:\n" },
    { code: "    case 7:\n" },
    {
      code: "      weekend = 1;\n",
      instructions:
        "The $c{switch} jumped straight to the $c{case} label matching $n{day}'s value, $c{case 7:}, right past the code for the other cases.",
    },
    { code: "      break;\n" },
    { code: "    default:\n" },
    { code: "      weekend = -1;\n" },
    { code: "  }\n" },
    {
      code: "  int code = 0;\n",
      instructions:
        "The $c{break} on line 15 left the $c{switch}. Without it, execution would have kept going into the next $c{case}'s code. This is called falling through. That's how cases 1 to 5 share code: a $c{day} of $v{1} would jump to $c{case 1:} and fall through the other labels to $c{weekend = 0;}. But forgetting a $c{break} is a classic bug.",
    },
    {
      code: "  switch (weekend) {\n    case 0:\n      code = 10;\n    case 1:\n      code += 20;\n    default:\n      code += 30;\n  }\n",
      editable: true,
      instructions: "What should the program state look like after this $c{switch}? Watch out: there are no $c{break}s.",
      hints: (ctx) =>
        ctx.basicHintTopicIs("value", "code")
          ? "Execution jumps to $c{case 1:}, then falls through into the $c{default:} code too."
          : ctx.basicHint,
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "120-conditional-operator.html",
});
