import { createProgramTemplate } from "./shared-program-template.js";

// A hint about one variable's value, for when that's what's wrong.
const valueHint = (name: string, hint: string) => (ctx: {
  basicHint: string | null;
  basicHintTopicIs: (kind: "count" | "name" | "value", name?: string) => boolean;
}) => (ctx.basicHintTopicIs("value", name) ? hint : ctx.basicHint);

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "Code that controls hardware sets, clears, flips, and tests single bits all the time, so it's worth knowing how to do each by heart. All four start the same way: to work with bit $n{n}, make a value with only that bit set, $c{1 << n}.\nClick $runLineButton to continue.",
  steps: [
    { code: "unsigned char lights = 0;\n\n" },
    { code: "int main(void) {\n" },
    { code: "  int n = 3;\n" },
    {
      code: "  lights |= 1 << n;\n",
      instructions: "To set bit $n{n} (turn it on), OR with $c{1 << n}.",
    },
    {
      code: "  lights |= 1 << 6;\n",
      editable: true,
      hints: valueHint("lights", "$c{1 << 6} has only bit 6 set, so ORing with it turns on bit 6 and leaves the other bits as they were."),
    },
    {
      code: "  lights &= ~(1 << n);\n",
      instructions:
        "To clear bit $n{n} (turn it off), AND with $c{~(1 << n)}: every bit set except bit $n{n}.",
    },
    {
      code: "  lights ^= 1 << 0;\n",
      editable: true,
      instructions: "To toggle bit $n{n} (flip it), XOR with $c{1 << n}.",
      hints: valueHint("lights", "Bit 0 is off, so flipping it turns it on. The other bits stay as they were."),
    },
    {
      code: "  lights ^= 1 << 0;\n",
      editable: true,
      hints: valueHint("lights", "Bit 0 is on now, so flipping it again turns it back off."),
    },
    {
      code: "  int on = (lights >> 6) & 1;\n",
      editable: true,
      instructions:
        "And to test bit $n{n}, shift it down to bit 0, and AND with $v{1}. ($c{(lights & (1 << n)) != 0} works too.)",
      hints: valueHint("on", "$c{lights >> 6} moves bit 6 down to bit 0, and $c{& 1} keeps just that bit. Is bit 6 of $n{lights} on?"),
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "133-mirror.html",
  workspace: { lights: "lights" },
});
