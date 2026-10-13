import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup =
  "#include <stdlib.h>\nint* q = malloc(4 * sizeof(int));\n*q = 10;\n*(q + 1) = 20;\n*(q + 2) = 30;\n*(q + 3) = 40;\nint* p = q + 1;\nint i = 2;";

function selectHint(expression: string, meaning: string, target: string) {
  return (ctx: { mode: string }) =>
    ctx.mode === "selected"
      ? `$c{${expression}} means $c{${meaning}}, which is element $v{${target}}.`
      : `$c{${expression}} refers to an element. Click $fromStateButton and select it.`;
}

createExpressionEvalTemplate({
  initialInstructions:
    "Writing $c{*(p + i)} all the time gets tiresome, so C has a shorthand for it: $c{p[i]}. The two mean exactly the same thing. The number in brackets is called the index, or subscript.\nAs before, $n{q} points to element $v{[0]}, and $n{p} points to element $v{[1]}. For each expression, select what it refers to, enter its type and value, or click $undefinedButton.",
  steps: [
    {
      expression: "q[2]",
      setup,
      instructions: "$c{q[2]} means $c{*(q + 2)}. Since $n{q} points to element $v{[0]}, $c{q[i]} is element $v{[i]}, which is why elements are numbered from 0.",
    },
    {
      expression: "q[i]",
      setup,
      editable: true,
      hints: selectHint("q[i]", "*(q + i)", "[2]"),
    },
    {
      expression: "p[1]",
      setup,
      editable: true,
      hints: selectHint("p[1]", "*(p + 1)", "[2]"),
    },
    {
      expression: "p[-1]",
      setup,
      editable: true,
      explanation: "An index can be negative, as long as the result stays inside the array.",
      hints: selectHint("p[-1]", "*(p - 1)", "[0]"),
    },
    {
      expression: "q[i + 1] - q[0]",
      setup,
      editable: true,
      hints: (ctx) =>
        ctx.mode === "entered"
          ? "$c{q[i + 1]} is element $v{[3]}, and $c{q[0]} is element $v{[0]}."
          : "This subtracts two elements' values, so it's a value, not an element. Click $typeBoxButton.",
    },
    {
      expression: "q[4]",
      setup,
      editable: true,
      explanation: "The array only has 4 elements, numbered $v{0} to $v{3}, so $c{q[4]} is past its end.",
      hints: (ctx) => (ctx.mode === "undefined" ? null : "How many elements does the array have, and what are they numbered?"),
    },
    {
      expression: "2[q]",
      setup,
      editable: true,
      explanation:
        "Since $c{x[y]} just means $c{*(x + y)}, and $c{q + 2} is the same as $c{2 + q}, $c{2[q]} means the same as $c{q[2]}. This is a famous quirk of C. Never write it in real code.",
      hints: selectHint("2[q]", "*(2 + q)", "[2]"),
    },
  ],
  next: "79-allocated-loops.html",
  workspace: { allowUndefinedAnswer: true },
});
