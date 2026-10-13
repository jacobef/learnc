import { createProgramTemplate } from "./shared-program-template.js";

const remainderInstructions =
  "$c{x % y} is the remainder when $c{x} is divided by $c{y}. For example, 17 divided by 5 is 3 with a remainder of 2, so $c{17 % 5} is $v{2}.\n$c{%} has the same precedence as $c{*} and $c{/}, and only works on $t{int}s, not $t{double}s.";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    { code: "int a = 17 % 5;\n", instructions: remainderInstructions },
    {
      code: "int b = 12 % 4;\n",
      instructions: "12 divided by 4 is exactly 3, with nothing left over, so the remainder is $v{0}.",
    },
    {
      code: "int c = 3 % 8;\n",
      editable: true,
      instructions: remainderInstructions,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "c")) {
          const c = ctx.boxNamed("c")!;
          if (c.value === "0") {
            return "$c{3 / 8} is $v{0}, but $c{%} gives what's left over, not the result of the division. If you divide 3 into groups of 8, how much is left over?";
          }
          return "8 goes into 3 zero times, with 3 left over.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "a = 1 + 9 % 4 * 3;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "a")) {
          const a = ctx.boxNamed("a")!;
          if (a.value === "10") {
            return "You're parsing it as $c{1 + (9 % (4 * 3))}, but $c{%} and $c{*} have the same precedence and are left-associative, so it should be parsed as $c{1 + ((9 % 4) * 3)}.";
          }
          if (a.value === "6") {
            return "You're parsing it as $c{((1 + 9) % 4) * 3}, but $c{%} has higher precedence than $c{+}, so it should be parsed as $c{1 + ((9 % 4) * 3)}.";
          }
          return "This is parsed as $c{1 + ((9 % 4) * 3)}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "b = -7 % 3;\n",
      instructions:
        "If the left side is negative, calculate the remainder as if it were positive, then make the result negative. Here, $c{7 % 3} is $v{1}, so $c{-7 % 3} is $v{-1}.",
    },
    {
      code: "c = -9 % 4;\n",
      editable: true,
      instructions:
        "If the left side is negative, calculate the remainder as if it were positive, then make the result negative.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "c")) {
          const c = ctx.boxNamed("c")!;
          if (c.value === "1") {
            return "$c{9 % 4} is $v{1}, but the left side is negative, so the result should be negative too.";
          }
          if (c.value === "3") {
            return "In math, remainders are sometimes defined to never be negative, but C doesn't work that way. Calculate $c{9 % 4}, then make the result negative.";
          }
          return "Calculate $c{9 % 4}, then make the result negative.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int d = 14 % 7 == 0;\n",
      instructions:
        "A common use of $c{%}: $c{x % y == 0} checks whether $c{x} is divisible by $c{y}.",
    },
    {
      code: "int e = 7 % 2 == 1 + 26 % 10 / 3;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "e")) {
          return "This is parsed as $c{(7 % 2) == (1 + ((26 % 10) / 3))}.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "37-odd.html",
  workspace: { allowVariableCreation: true },
});
