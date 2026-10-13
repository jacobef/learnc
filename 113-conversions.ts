import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup = "char a = 'A';\nchar b = 2;\nshort s = 1000;\nunsigned int u = 3;\ndouble d = 2.5;";

const typeHint = (type: string, why: string) => (ctx: { mode: string }) =>
  ctx.mode === "entered" ? `${why} So the result is an $t{${type}}.` : "This is a value, not a variable. Click $typeBoxButton.";

createExpressionEvalTemplate({
  initialInstructions:
    "C converts values automatically in calculations, following two rules. First, integer promotion: anything smaller than an $t{int}, like a $t{char} or a $t{short}, is converted to an $t{int} before any calculation. Second, when an operator's two sides have different types, both are converted to the larger type, which you've seen with $t{int}s and $t{double}s. (If one side is unsigned and the other isn't, and they're the same size, both become unsigned.)\nFor each expression, enter its type and value.",
  steps: [
    {
      expression: "a + b",
      setup,
      instructions: "Both $t{char}s are promoted to $t{int}s, so the result is an $t{int}, even though both sides were $t{char}s.",
    },
    {
      expression: "s * 100",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "Since $n{s} is promoted to an $t{int}, the calculation can't overflow a $t{short}.",
      hints: typeHint("int", "$n{s} is promoted to an $t{int}."),
    },
    {
      expression: "b + d",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: typeHint("double", "$n{b} is promoted to an $t{int}, and then converted to a $t{double} to match $n{d}."),
    },
    {
      expression: "7 / 2 * d",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "Conversions happen one operator at a time: $c{7 / 2} is done with $t{int}s first, giving $v{3}, and only then is $v{3} converted to multiply with $n{d}.",
      hints: typeHint("double", "$c{7 / 2} is $v{3}, and then $c{3 * d} is a $t{double} calculation."),
    },
    {
      expression: "u - 5",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "$v{5} is converted to an $t{unsigned int}, and the subtraction wraps around below $v{0}.",
      hints: typeHint("unsigned int", "$v{5} is converted to match $n{u}, and the result wraps around."),
    },
    {
      expression: "a == 65",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: typeHint("int", "A comparison always gives an $t{int}: $v{1} or $v{0}. $c{'A'} is $v{65}."),
    },
  ],
  next: "114-casts.html",
});
