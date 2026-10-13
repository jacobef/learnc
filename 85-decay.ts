import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const setup = "int a[3] = {4, 8, 15};\nint* p = a + 1;";

function selectHint(expression: string, target: string, why: string) {
  return (ctx: { mode: string }) =>
    ctx.mode === "selected"
      ? `${why} So $c{${expression}} refers to $c{${target}}.`
      : `$c{${expression}} refers to a variable. Click $fromStateButton and select it.`;
}

createExpressionEvalTemplate({
  initialInstructions:
    "Here's the strange part. When an array is used in almost any expression, it's automatically converted into a pointer to its first element. This is called decay: in $c{a + 1}, $c{a} decays into $c{&a[0]}.\nIn $i{78. Indexing}, $c{a} on its own was shown as referring to the whole array. That's what the name itself refers to, but as soon as it's used as part of a larger expression, it decays. (One exception is $c{sizeof a}, which we'll get to.)\nThis is also how $c{p} was set up here: $c{int* p = a + 1;} means $c{int* p = &a[0] + 1;}.",
  steps: [
    {
      expression: "a + 0",
      setup,
      instructions: "$c{a} decays into $c{&a[0]}, so $c{a + 0} is a pointer to $c{a[0]}.",
    },
    {
      expression: "a + 2",
      setup,
      editable: true,
      hints: (ctx) =>
        ctx.mode === "entered"
          ? "$c{a} decays into a pointer to $c{a[0]}, so $c{a + 2} is an $t{int*} pointing to $c{a[2]}."
          : "$c{a + 2} is a pointer value. Click $typeBoxButton.",
    },
    {
      expression: "*a",
      setup,
      editable: true,
      hints: selectHint("*a", "a[0]", "$c{a} decays into a pointer to $c{a[0]}."),
    },
    {
      expression: "*(a + 2)",
      setup,
      editable: true,
      explanation:
        "This is in fact exactly what $c{a[2]} means. In C, $c{x[i]} is defined as $c{*(x + i)}: the array decays into a pointer to its first element, the index moves it forward with pointer arithmetic, and $c{*} gives the element. That's why indices start at 0: $c{a[0]} is $c{*(a + 0)}, the first element itself.",
      hints: selectHint("*(a + 2)", "a[2]", "$c{a + 2} points to $c{a[2]}."),
    },
    {
      expression: "p[1]",
      setup,
      editable: true,
      explanation:
        "Since $c{x[i]} just means $c{*(x + i)}, brackets work on any pointer, not just arrays.",
      hints: selectHint("p[1]", "a[2]", "$c{p[1]} means $c{*(p + 1)}, and $n{p} points to $c{a[1]}."),
    },
    {
      expression: "p[-1]",
      setup,
      editable: true,
      explanation:
        "$c{p[-1]} means $c{*(p - 1)}, which is $c{a[0]}. A negative index is fine, as long as it stays inside the array.",
      hints: selectHint("p[-1]", "a[0]", "$c{p[-1]} means $c{*(p - 1)}, and $n{p} points to $c{a[1]}."),
    },
    {
      expression: "2[a]",
      setup,
      editable: true,
      explanation:
        "$c{2[a]} means $c{*(2 + a)}, which is the same as $c{*(a + 2)}, so this is $c{a[2]}. It's a famous quirk of C, and it shows that brackets really are just shorthand for pointer arithmetic. Never write it in real code.",
      hints: selectHint("2[a]", "a[2]", "$c{2[a]} means $c{*(2 + a)}."),
    },
    {
      expression: "a[1] + *a",
      setup,
      editable: true,
      hints: (ctx) =>
        ctx.mode === "entered"
          ? "$c{a[1]} is $v{8}, and $c{*a} is $c{a[0]}, which is $v{4}."
          : "This adds two elements' values, so it's a value, not a variable. Click $typeBoxButton.",
    },
  ],
  next: "86-array-sizes.html",
  workspace: { allowUndefinedAnswer: true },
});
