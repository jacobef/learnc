import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "Code that controls hardware does these four things all the time, so it's worth knowing them by heart. To work with bit $n{n}, make a byte with only that bit set: $c{1 << n}.\nClick $runLineButton to continue.",
  steps: [
    { code: "unsigned char lights = 0;\n\n" },
    { code: "int main(void) {\n" },
    { code: "  int n = 3;\n" },
    {
      code: "  lights |= 1 << n;\n",
      instructions: "To set bit $n{n} (turn it on), OR with $c{1 << n}.",
    },
    { code: "  lights |= 1 << 6;\n", editable: true, hints: basic },
    {
      code: "  lights &= ~(1 << n);\n",
      instructions:
        "To clear bit $n{n} (turn it off), AND with $c{~(1 << n)}: every bit set except bit $n{n}.",
    },
    {
      code: "  lights ^= 1 << 0;\n",
      editable: true,
      instructions: "To toggle bit $n{n} (flip it), XOR with $c{1 << n}.",
      hints: basic,
    },
    { code: "  lights ^= 1 << 0;\n", editable: true, hints: basic },
    {
      code: "  int on = (lights >> 6) & 1;\n",
      editable: true,
      instructions:
        "And to test bit $n{n}, shift it down to bit 0, and AND with $v{1}. ($c{(lights & (1 << n)) != 0} works too.)",
      hints: basic,
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "133-mirror.html",
  workspace: { lights: "lights" },
});
