import { createExpressionEvalTemplate } from "./shared-expression-template.js";

createExpressionEvalTemplate({
  steps: [
    {
      expression: "5",
      instructions:
        "An expression is something that can be assigned to a variable. For example, $c{5} is an expression because it can appear in e.g. $c{int x = 5;}.",
    },
    { expression: "7/3" },
    {
      expression: "x+3",
      boxes: [
        { name: "x", type: "int", value: "5", address: "108" },
        { name: "y", type: "int*", value: "108", address: "112" },
      ],
      fixValueCategory: true,
      editable: true,
      hints: (ctx) => {
        if (ctx.enteredBox!.type !== "int") {
          return "$c{x} and $c{3} are both $t{int}s, so $c{x+3} should also be an $t{int}.";
        }
        return "$n{x}'s value is $v{5}, so what is the value of $c{x+3}?";
      },
    },
    {
      expression: "5.0",
      instructions: "A number with a decimal point has type $t{double}.",
    },
    {
      expression: "1.25+2.5",
      instructions:
        "An operation performed on $t{double}s always results in a $t{double}.",
    },
    {
      expression: "1.5-0.5",
      fixValueCategory: true,
      editable: true,
      hints: (ctx) => {
        const box = ctx.enteredBox!;
        if (box.type === "int") {
          return "$c{1.5} and $c{0.5} are both $t{double}s. An operation performed on $t{double}s should always result in a $t{double}.";
        }
        if (box.type === "") {
          return "The type should be $t{double}.";
        }
        if (box.type !== "double") {
          return `The type should be $t{double}, not $t{${box.type}}.`;
        }
        if (box.value === "1") {
          return "Since it's a $t{double}, you should enter $v{1.0}, not $v{1}.";
        }
        if (box.value === "") {
          return "The value shouldn't be empty.";
        }
        return `I'm not sure where you're getting $v{${box.value}} from.`;
      },
    },
    {
      expression: "0.5/4.0",
      instructions:
        "Unlike with $t{int}s, division with $t{double}s doesn't chop off the decimal part.",
    },
    {
      expression: "(1.0+2.0)/x",
      boxes: [{ name: "x", type: "double", value: "12.0", address: "402" }],
      fixValueCategory: true,
      editable: true,
      hints: (ctx) => {
        const box = ctx.enteredBox!;
        if (box.type === "") {
          return "The type should be $t{double}.";
        }
        if (box.type !== "double") {
          return `The type should be $t{double}, not $t{${box.type}}.`;
        }
        if (box.value === "") {
          return "The value shouldn't be empty.";
        }
        return `I'm not sure where you're getting $v{${box.value}} from.`;
      },
    },
    {
      expression: "(1+2)/12",
      fixValueCategory: true,
      editable: true,
      hints: (ctx) => {
        const box = ctx.enteredBox!;
        if (box.type !== "int") {
          return "$v{1}, $v{2}, and $v{12} are all $t{int}s.";
        }
        if (box.value === "0.25") {
          return "Remember that $t{int} division chops off the decimal part.";
        }
        if (box.value === "0.0") {
          return "Since it's an $t{int}, you should enter $v{0}, not $v{0.0}.";
        }
        if (box.value === "") {
          return "The value shouldn't be empty.";
        }
        return `I'm not sure where you're getting $v{${box.value}} from.`;
      },
    },
    {
      expression: "2/4.0",
      instructions:
        "When an operation is performed on a $t{double} and an $t{int}, the operation is performed as if both were $t{double}s. Here, $c{2/4.0} is performed as $c{2.0/4.0}, which is $v{0.5}.",
    },
    {
      expression: "0.5+4/8",
      instructions:
        "Due to order of operations, the $c{4/8} happens first. $c{4} and $c{8} are both $t{int}s, so the $c{4/8} evaluates to 0.",
    },
    {
      expression: "1+2/4.0-6/4",
      fixValueCategory: true,
      editable: true,
      hints: (ctx) => {
        const box = ctx.enteredBox!;
        if (box.value === "-1.3125" || box.value === "-1") {
          return "Remember order of operations. Division has higher precedence than addition and subtraction.";
        }
        if (box.value === "0" || box.value === "0.0") {
          return "When $c{2/4.0} is evaluated, its decimal part should stay, since $c{4.0} is a $t{double}. On the other hand, when $c{6/4} gets evaluated, its decimal part should be removed, since $c{6} and $c{4} are both $t{int}s.";
        }
        if (box.value === "") {
          return "The expression should be parsed as $c{1+(2/4.0)-(6/4)}.";
        }
        if (box.value !== "0.5") {
          return `I'm not sure where you're getting $v{${box.value}} from.`;
        }
        if (box.type === "int") {
          return "Recall that an operation performed on an $t{int} and a $t{double} is performed as if both were $t{double}s; and that the result of an operation performed on two $t{double}s always has type $t{double}. $c{4.0} is a $t{double}.";
        }
        if (box.type === "") {
          return "The type should be $t{double}.";
        }
        return `The type should be $t{double}, not $t{${box.type}}.`;
      },
    },
  ],
  next: "15-doubles.html",
});
