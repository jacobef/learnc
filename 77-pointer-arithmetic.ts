import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup =
  "#include <stdlib.h>\nint* q = malloc(4 * sizeof(int));\n*q = 10;\n*(q + 1) = 20;\n*(q + 2) = 30;\n*(q + 3) = 40;\nint* p = q + 1;";

function selectHint(expression: string, target: string) {
  return (ctx: { mode: string }) =>
    ctx.mode === "selected"
      ? `$n{p} points to element $v{[1]}, so $c{${expression}} refers to element $v{${target}}.`
      : `$c{${expression}} refers to an element. Click $fromStateButton and select it.`;
}

createExpressionEvalTemplate({
  initialInstructions:
    "To reach the other elements of an array, C lets you add an integer to a pointer to an element. $c{p + 1} points to the element after the one $n{p} points to, $c{p + 2} to the one after that, and so on. Subtracting goes backwards. This is called pointer arithmetic.\nThe result has to stay inside the array, or point just past its last element; going any further is undefined behavior, even without using the pointer.\nHere, $n{q} points to element $v{[0]} of an array of 4 $t{int}s from $c{malloc}, and $n{p} points to element $v{[1]}. For each expression, select what it refers to, enter its type and value, or click $undefinedButton.",
  steps: [
    {
      expression: "p + 1",
      setup,
      instructions:
        "$c{p + 1} points to element $v{[2]}. Notice that its value is $n{p}'s value plus 4, not plus 1: adding 1 to a pointer moves it forward by one element, and each $t{int} takes up 4 bytes.",
    },
    {
      expression: "*(p + 1)",
      setup,
      instructions: "So $c{*(p + 1)} refers to element $v{[2]}.",
    },
    {
      expression: "*(p + 2)",
      setup,
      editable: true,
      hints: selectHint("*(p + 2)", "[3]"),
    },
    {
      expression: "*(p - 1)",
      setup,
      editable: true,
      hints: selectHint("*(p - 1)", "[0]"),
    },
    {
      expression: "p + 2",
      setup,
      editable: true,
      hints: (ctx) =>
        ctx.mode === "entered"
          ? "$c{p + 2} is a pointer, so its type is $t{int*}. Its value is the address of the element 2 after element $v{[1]}."
          : "$c{p + 2} doesn't refer to an element; it's a pointer value. Click $typeBoxButton.",
    },
    {
      expression: "*p + 1",
      setup,
      editable: true,
      explanation:
        "Like $c{-}, $c{*} has higher precedence than $c{+}, so this is $c{(*p) + 1}, which adds 1 to the value of the element $n{p} points to.",
      hints: (ctx) =>
        ctx.mode === "entered"
          ? "$c{*} has higher precedence than $c{+}, so this is $c{(*p) + 1}."
          : "Is this $c{*(p + 1)} or $c{(*p) + 1}? $c{*} has higher precedence than $c{+}.",
    },
    {
      expression: "*(p + 3)",
      setup,
      editable: true,
      explanation:
        "$c{p + 3} points just past the end of the array. A pointer like that is allowed to exist, but there's no element there, so using $c{*} on it is undefined behavior.",
      hints: (ctx) =>
        ctx.mode === "undefined" ? null : "$n{p} points to element $v{[1]}. Which element would be 3 after that?",
    },
    {
      expression: "q - 1",
      setup,
      editable: true,
      explanation:
        "$c{q - 1} would point before the first element. Just computing a pointer like that is undefined behavior, even if it's never used.",
      hints: (ctx) =>
        ctx.mode === "undefined" ? null : "$n{q} points to the first element. What's before it?",
    },
  ],
  next: "78-subscripts.html",
  workspace: { allowUndefinedAnswer: true },
});
