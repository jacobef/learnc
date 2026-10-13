import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "C uses $t{int}s for true and false: comparisons give $v{1} or $v{0}, and conditions treat anything non-zero as true. But there's also a type just for this, $t{bool}, which can only hold $v{0} or $v{1}. The header $c{stdbool.h} defines $t{bool}, and the names $c{true} and $c{false} for $v{1} and $v{0}. (In the newest version of C, they're built in, and the header isn't needed.) The program state shows $t{bool}'s real name, $t{_Bool}.\nClick $runLineButton to continue.",
  steps: [
    { code: "#include <stdbool.h>\n\n" },
    { code: "int main(void) {\n" },
    { code: "  bool ready = false;\n" },
    {
      code: "  bool done = 5;\n",
      editable: true,
      instructions: "Converting any value to a $t{bool} gives $v{1} if it's non-zero, and $v{0} if it's zero.",
      hints: basic,
    },
    { code: "  int x = -3;\n" },
    {
      code: "  bool positive = x > 0;\n",
      editable: true,
      hints: basic,
    },
    {
      code: "  ready = !ready;\n",
      editable: true,
      hints: basic,
    },
    {
      code: "  int count = true + done + positive;\n",
      editable: true,
      instructions: "In a calculation, a $t{bool} is just a small integer.",
      hints: basic,
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "116-floats.html",
});
