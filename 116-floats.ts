import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "There are two common floating-point types: $t{double}, which you know, and $t{float}, which takes up 4 bytes instead of 8, and is less precise. ($t{double} is short for \"double precision\".) A number with a decimal point is a $t{double}, unless it ends in $c{f}, which makes it a $t{float}.\nNeither can hold every number exactly. Each holds a binary number with a fixed number of significant digits: about 7 decimal digits for a $t{float}, and about 16 for a $t{double}. Anything else is rounded to the nearest number that fits.\nClick $runLineButton to continue.",
  steps: [
    { code: "int main(void) {\n" },
    {
      code: "  float f = 0.1f;\n",
      instructions:
        "$v{0.1} can't be written exactly in binary, just like $c{1/3} can't be written exactly in decimal. So $n{f} holds the nearest $t{float}. The program state shows it rounded to the shortest number that would round to the same $t{float}; click $b{exact} to see its exact value.",
    },
    {
      code: "  double d = 0.1;\n",
      instructions: "A $t{double} gets much closer to $v{0.1}, but still isn't exact. Click $b{exact} to compare.",
    },
    {
      code: "  float big = 16777217;\n",
      instructions:
        "A $t{float} has 24 binary digits of precision, so it can hold every integer up to $c{2^24}, which is $v{16777216}, but not all of the ones after that. $v{16777217} gets rounded.",
    },
    {
      code: "  double sum = 0.1 + 0.2;\n",
      instructions:
        "Rounding errors can add up. $v{0.1} and $v{0.2} are both rounded, and so is their sum, which ends up slightly different from the $t{double} nearest to $v{0.3}.",
    },
    {
      code: "  int same = sum == 0.3;\n",
      editable: true,
      instructions: "So is $n{sum} equal to $c{0.3}?",
      hints: (ctx) =>
        ctx.basicHintTopicIs("value", "same")
          ? "$n{sum} is $v{0.30000000000000004}, which isn't the same $t{double} as $c{0.3}."
          : ctx.basicHint,
    },
    {
      code: "  int close = sum - 0.3 < 0.000001 && 0.3 - sum < 0.000001;\n",
      editable: true,
      instructions:
        "Because of this, floating-point numbers should usually be compared by checking whether they're close enough, rather than exactly equal.",
      hints: basic,
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "117-ieee.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
