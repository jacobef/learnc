import { createExpressionEvalTemplate } from "./shared-expression-template.js";

const xy = "int x = 7;\nint y = 0;";

createExpressionEvalTemplate({
  initialInstructions:
    "If the left side of $c{&&} is zero, its right side is never evaluated, since the result will be $v{0} no matter what. Similarly, if the left side of $c{||} is non-zero, its right side is never evaluated, since the result will be $v{1} no matter what. This is called short-circuiting.\nShort-circuiting is often used to avoid undefined behavior.",
  steps: [
    {
      expression: "y != 0 && x / y > 2",
      setup: xy,
      instructions:
        "$c{y != 0} is $v{0}, so $c{x / y > 2} is never evaluated, and the result is $v{0}. Without short-circuiting, this would divide by zero.",
    },
    {
      expression: "y == 0 || x / y > 2",
      setup: xy,
      editable: true,
      fixValueCategory: true,
      explanation:
        "$c{y == 0} is $v{1}, so the right side is skipped, and the result is $v{1}.",
      hints: (ctx) => {
        if (ctx.mode === "undefined") {
          return "$c{y == 0} is $v{1}, so the right side of the $c{||} is never evaluated.";
        }
        return "$c{y == 0} is $v{1}. What does that mean for the whole $c{||}?";
      },
    },
    {
      expression: "x / y > 2 && y != 0",
      setup: xy,
      editable: true,
      fixValueCategory: true,
      explanation:
        "The left side is always evaluated first, and here the left side divides by zero. Short-circuiting can only skip the right side.",
      hints: (ctx) => {
        if (ctx.mode !== "undefined") {
          return "The left side is always evaluated first. Here, the left side is $c{x / y > 2}.";
        }
        return null;
      },
    },
    {
      expression: "y != 0 || x / y > 2",
      setup: xy,
      editable: true,
      fixValueCategory: true,
      explanation:
        "$c{y != 0} is $v{0}, so $c{||} has to evaluate its right side to find out the result. The right side divides by zero.",
      hints: (ctx) => {
        if (ctx.mode !== "undefined") {
          return "$c{y != 0} is $v{0}. The right side of $c{||} is only skipped if the left side is non-zero.";
        }
        return null;
      },
    },
    {
      expression: "y && x % y",
      setup: "int x = 7;\nint y = 2;",
      editable: true,
      fixValueCategory: true,
      explanation:
        "$n{y} is $v{2}, so the right side is evaluated. $c{7 % 2} is $v{1}, so both sides are non-zero, and the result is $v{1}.",
      hints: (ctx) => {
        if (ctx.mode === "undefined") {
          return "Look at $n{y}'s value again.";
        }
        return "$n{y} is $v{2}, so the right side is evaluated. $c{x % y} is $v{1}.";
      },
    },
    {
      expression: "u > 0 && u < 10",
      setup: "int u;",
      editable: true,
      fixValueCategory: true,
      explanation:
        "The left side, $c{u > 0}, uses $n{u}'s value, which it doesn't have yet. Short-circuiting can't help, since the left side is always evaluated.",
      hints: (ctx) => {
        if (ctx.mode !== "undefined") {
          return "Short-circuiting can only skip the right side. The left side, $c{u > 0}, is always evaluated, and $n{u} doesn't have a value yet.";
        }
        return null;
      },
    },
    {
      expression: "!y && x / y",
      setup: xy,
      editable: true,
      fixValueCategory: true,
      explanation:
        "$c{!y} is $v{1}, so $c{&&} has to evaluate its right side to find out the result. The right side divides by zero.",
      hints: (ctx) => {
        if (ctx.mode !== "undefined") {
          return "$c{!y} is $v{1}, not $v{0}, so the right side of the $c{&&} is evaluated.";
        }
        return null;
      },
    },
    {
      expression: "y == 0 || x / y > 1 && x % y == 0",
      setup: xy,
      editable: true,
      fixValueCategory: true,
      explanation:
        "$c{&&} has higher precedence than $c{||}, so this is parsed as $c{y == 0 || (x / y > 1 && x % y == 0)}. $c{y == 0} is $v{1}, so the entire right side of the $c{||} is skipped, and the result is $v{1}.",
      hints: (ctx) => {
        if (ctx.mode === "undefined") {
          return "$c{&&} has higher precedence than $c{||}, so this is parsed as $c{y == 0 || (x / y > 1 && x % y == 0)}. $c{y == 0} is $v{1}, so the entire right side of the $c{||} is skipped.";
        }
        return "This is parsed as $c{y == 0 || (x / y > 1 && x % y == 0)}.";
      },
    },
  ],
  next: "44-compound-assignment.html",
  workspace: { allowUndefinedAnswer: true },
});
