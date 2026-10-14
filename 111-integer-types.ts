import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "$t{int} isn't the only integer type. There are integer types of several sizes, which can hold ranges of different sizes. From smallest to largest: $t{char}, $t{short}, $t{int}, $t{long}, and $t{long long}. C only promises a minimum size for each, and that each is at least as big as the one before it. Their actual sizes depend on the computer.\nClick $runLineButton to continue.",
  steps: [
    { code: "int main(void) {\n" },
    {
      code: "  int s = sizeof(short);\n",
      instructions: "A $t{short} is 2 bytes here, so it holds values from $v{-32768} to $v{32767}.",
    },
    {
      code: "  int l = sizeof(long);\n",
      instructions: "A $t{long} is 8 bytes on most 64-bit computers, like this one. (On Windows, it's 4.)",
    },
    {
      code: "  int ll = sizeof(long long);\n",
      instructions: "A $t{long long} is at least 8 bytes everywhere, so it holds values up to $v{9223372036854775807}.",
    },
    {
      code: "  long big = 2147483647;\n",
    },
    {
      code: "  big = big + 1;\n",
      editable: true,
      instructions: "This would overflow an $t{int}, but $n{big} is a $t{long}, and the result fits.",
      hints: basic,
    },
    {
      code: "  long long huge = 1000000LL * 1000000;\n",
      editable: true,
      instructions:
        "A number written in the code is an $t{int} if it fits, so $c{1000000 * 1000000} would be an $t{int} calculation, and would overflow. A suffix gives a number a bigger type: $c{L} for $t{long}, and $c{LL} for $t{long long}. Then the other side is converted to match, as with $t{int}s and $t{double}s.",
      hints: basic,
    },
    {
      code: "  int small = huge;\n",
      instructions:
        "Converting a value to a signed integer type it doesn't fit in isn't undefined behavior, but what happens depends on the compiler, which is called implementation-defined. Almost every compiler just keeps the lowest bytes, which here gives this strange negative number. Either way, the value is lost.",
    },
    {
      code: "  short tiny = 40000;\n",
      editable: true,
      instructions: "Keeping the lowest 2 bytes of $v{40000} gives $c{40000 - 65536}.",
      hints: basic,
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "112-unsigned.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
