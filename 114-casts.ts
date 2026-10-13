import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup = "double d = 3.99;\nint n = 7;";

const valueHint = (why: string) => (ctx: { mode: string }) =>
  ctx.mode === "entered" ? why : "This is a value, not a variable. Click $typeBoxButton.";

createExpressionEvalTemplate({
  initialInstructions:
    "A conversion can also be asked for explicitly, with a cast: a type in parentheses before a value, like $c{(double)n}. It converts the value to that type, by the same rules as storing it in a variable of that type.\nFor each expression, enter its type and value.",
  steps: [
    {
      expression: "(int)d",
      setup,
      instructions: "Converting a $t{double} to an integer type drops the part after the decimal point, without rounding.",
    },
    {
      expression: "(double)n / 2",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "A cast has higher precedence than $c{/}, so this converts $n{n} to a $t{double} first, and then the division isn't an integer division. This is the usual way to divide two $t{int}s without losing the fraction.",
      hints: valueHint("The cast applies to $n{n} first, giving $v{7.0}, so this divides a $t{double} by $v{2}."),
    },
    {
      expression: "(double)(n / 2)",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "Here the division happens first, with $t{int}s, so the fraction is already gone by the time it's converted.",
      hints: valueHint("The parentheses make $c{n / 2} happen first: $v{3}, as an integer division."),
    },
    {
      expression: "(int)-d",
      setup,
      editable: true,
      fixValueCategory: true,
      explanation: "Dropping the fraction moves negative numbers towards zero, so this is $v{-3}, not $v{-4}.",
      hints: valueHint("$c{-d} is $v{-3.99}, and converting it to an $t{int} drops the $c{.99}."),
    },
    {
      expression: "(char)(n + 'a')",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("$c{'a'} is $v{97}, so $c{n + 'a'} is $v{104}, the code for $c{'h'}."),
    },
    {
      expression: "(unsigned char)-1",
      setup,
      editable: true,
      fixValueCategory: true,
      hints: valueHint("Converting to an unsigned type wraps around: $v{-1} becomes the largest $t{unsigned char}."),
    },
  ],
  next: "115-bool.html",
});
