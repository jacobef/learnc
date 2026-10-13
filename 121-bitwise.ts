import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup = "int a = 12;\nint b = 10;";

const valueHint = (why: string) => (ctx: { mode: string }) =>
  ctx.mode === "entered" ? why : "Click $typeBoxButton.";

createExpressionEvalTemplate({
  initialInstructions:
    "Integers are stored in binary, and the bitwise operators work on each binary digit, or bit, separately. Here, $n{a} is $v{12}, which is $c{1100} in binary ($c{8 + 4}), and $n{b} is $v{10}, which is $c{1010} ($c{8 + 2}).\n$c{a & b} has a 1 bit wherever both have a 1 bit, $c{a | b} wherever either does, and $c{a ^ b} (\"exclusive or\") wherever exactly one does. $c{~a} flips every bit. $c{a << n} shifts the bits left by $c{n} places, which multiplies by $c{2^n}, and $c{a >> n} shifts them right, which divides by $c{2^n}.\nFor each expression, enter its type and value.",
  steps: [
    {
      expression: "a & b",
      setup,
      instructions: "$c{1100 & 1010} is $c{1000}: only the 8 bit is in both. That's $v{8}.",
    },
    {
      expression: "a | b",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("$c{1100 | 1010} is $c{1110}: $c{8 + 4 + 2}."),
    },
    {
      expression: "a ^ b",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("$c{1100 ^ 1010} is $c{0110}: the bits that are in exactly one of them."),
    },
    {
      expression: "1 << 4",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("Shifting $v{1} left 4 places gives $c{10000} in binary, which is $c{2^4}."),
    },
    {
      expression: "a >> 2",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("$c{1100} shifted right 2 places is $c{11}, which is $v{3}."),
    },
    {
      expression: "(a & 4) != 0",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "This is how to check whether a particular bit is set. Note the parentheses: $c{&} has lower precedence than $c{!=}, which surprises everyone.",
      hints: valueHint("$n{a} is $c{1100}, so it has the 4 bit."),
    },
    {
      expression: "~0",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation:
        "Flipping every bit of $v{0} gives all 1 bits. Signed integers are stored so that all 1 bits means $v{-1}: this is called two's complement, and it's what makes adding negative numbers work like adding positive ones.",
      hints: valueHint("All 1 bits, in an $t{int}, is $v{-1}."),
    },
    {
      expression: "1 << 31",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "$c{2^31} is too big for an $t{int}, so this overflows. Bitwise operations are usually done on unsigned types for this reason: $c{1u << 31} is fine.",
      hints: (ctx) => (ctx.mode === "undefined" ? null : "Does $c{2^31} fit in an $t{int}?"),
    },
  ],
  next: "122-power-of-two.html",
  workspace: { allowUndefinedAnswer: true },
});
