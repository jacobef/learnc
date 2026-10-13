import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "Almost every computer stores floating-point numbers the way the IEEE 754 standard describes. A $t{double}'s 64 bits are split into three parts: 1 sign bit (whether it's negative), 11 bits of exponent, and 52 bits of fraction. The value is like scientific notation in binary: $c{1.fraction × 2^exponent}. That's why $t{double}s can be huge, up to about $c{1.8 × 10^308}, or tiny, as small as about $c{5 × 10^-324}, while always having about 16 significant digits.\nIEEE 754 also has a few special values.\nClick $runLineButton to continue.",
  steps: [
    { code: "#include <math.h>\n\n" },
    { code: "int main(void) {\n" },
    {
      code: "  double zero = 0.0;\n",
    },
    {
      code: "  double negative_zero = -zero;\n",
      instructions: "Since the sign is a separate bit, there's a negative zero.",
    },
    {
      code: "  int equal = negative_zero == zero;\n",
      editable: true,
      instructions: "Negative zero and zero are different bits, but they compare as equal.",
      hints: basic,
    },
    {
      code: "  double infinity = INFINITY;\n",
      instructions:
        "There are also positive and negative infinity. $c{math.h} defines $c{INFINITY}. IEEE 754 says calculations that are too big, or divide by zero, give infinity. But the C standard doesn't require IEEE 754, so in C, dividing by zero or going outside a $t{double}'s range is still undefined behavior, and this tutorial treats it that way.",
    },
    {
      code: "  int bigger = infinity > 1e308;\n",
      editable: true,
      hints: basic,
    },
    {
      code: "  double nan = NAN;\n",
      instructions:
        "$c{NAN} is \"not a number\", the result of calculations like $c{0.0 / 0.0} in IEEE 754.",
    },
    {
      code: "  int self = nan == nan;\n",
      editable: true,
      instructions: "NaN is strange: it isn't equal to anything, not even itself. Any comparison with NaN is false, except $c{!=}.",
      hints: (ctx) =>
        ctx.basicHintTopicIs("value", "self") ? "Comparing NaN with anything, even NaN, is false." : ctx.basicHint,
    },
    {
      code: "  int check = isnan(nan);\n",
      instructions: "To check for NaN, use $c{isnan} from $c{math.h}.",
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "118-loop-control.html",
});
