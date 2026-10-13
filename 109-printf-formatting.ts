import { createProgramTemplate } from "./shared-program-template.js";

const output = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "Between the $c{%} and the letter, a conversion specification can say more about how to format the value.\nClick $runLineButton to continue.",
  steps: [
    { code: "#include <stdio.h>\n\n" },
    { code: "int main(void) {\n" },
    { code: "  double pi = 3.14159;\n" },
    {
      code: "  printf(\"%.2f\\n\", pi);\n",
      instructions: "$c{.2} is the precision: for $c{%f}, the number of digits after the decimal point. The value is rounded to fit.",
    },
    {
      code: "  printf(\"%.3f\\n\", pi);\n",
      editable: true,
      hints: output,
    },
    {
      code: "  printf(\"[%5d]\\n\", 42);\n",
      instructions: "A number right after the $c{%} is the minimum width. If the value is shorter, it's padded with spaces on the left, so numbers in a column line up on the right.",
    },
    {
      code: "  printf(\"[%-5d]\\n\", 42);\n",
      instructions: "A $c{-} pads on the right instead.",
    },
    {
      code: "  printf(\"[%03d]\\n\", 7);\n",
      editable: true,
      instructions: "A width starting with $c{0} pads with zeros instead of spaces.",
      hints: output,
    },
    {
      code: "  printf(\"[%6.1f]\\n\", pi);\n",
      editable: true,
      instructions: "Width and precision can be combined. The width counts every character, including the decimal point.",
      hints: output,
    },
    {
      code: "  printf(\"%x\\n\", 255);\n",
      instructions: "$c{%x} prints an integer in hexadecimal (base 16), where the digits are $c{0} to $c{9} and then $c{a} to $c{f}. 255 is $c{15 * 16 + 15}.",
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "110-fizzbuzz.html",
  workspace: { showOutput: true, hideStringLiterals: true },
});
