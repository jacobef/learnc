import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "No instructions for this one, but the $referenceButton might come in handy. Good luck!",
  steps: [
    { code: "int bolt = 17;\n" },
    {
      code: "int nut = bolt % 5;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "nut")) {
          const nut = ctx.boxNamed("nut")!;
          if (nut.value === "3") {
            return "$c{17 / 5} is $v{3}, but $c{%} gives the remainder, not the result of the division.";
          }
          return "17 divided by 5 is 3, with a remainder of what?";
        }
        return ctx.basicHint;
      },
    },
    { code: "int* gear = &nut;\n" },
    { code: "double spring = 2.5;\n" },
    {
      code: "*gear += bolt / 4;\n",
      editable: true,
      hints: (ctx) => {
        const [gear, nut] = ctx.boxesNamed("gear", "nut");
        if (gear!.value !== nut!.address) {
          return "$c{*gear += bolt / 4;} changes $n{*gear}, not $n{gear}. Click $resetButton and try again.";
        }
        if (ctx.basicHintTopicIs("value", "nut")) {
          return "$n{*gear} refers to $n{nut}. $c{*gear += bolt / 4;} means $c{*gear = *gear + bolt / 4;}, and $c{bolt / 4} uses $t{int} division.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "spring *= !(*gear % 3) + 1;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "spring")) {
          const spring = ctx.boxNamed("spring")!;
          if (spring.value === "2.5") {
            return "$c{*gear % 3} is $v{0}, so $c{!(*gear % 3)} is $v{1}, not $v{0}.";
          }
          return "This means $c{spring = spring * (!(*gear % 3) + 1);}. Start with $c{*gear % 3}, where $n{*gear} refers to $n{nut}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "if (nut == 6 || bolt > 20 && spring < 5.0) {\n",
      editable: true,
      instructions: "Select the line boundary where execution will resume.",
    },
    { code: "  gear = &bolt;\n" },
    {
      code: "  bolt--;\n",
      editable: true,
      hints: (ctx) => ctx.basicHint,
    },
    { code: "}\n" },
    {
      code: "for (int washer = 0; washer < 3; washer++) {\n  *gear += washer;\n}\n",
      editable: true,
      instructions: "What should the program state look like after this $c{for} loop finishes?",
      hints: (ctx) => {
        if (ctx.boxNamed("washer")) {
          return "$n{washer} was created in the loop's setup, so it should be removed when the loop finishes.";
        }
        if (ctx.basicHintTopicIs("value", "nut")) {
          return "$n{gear} was changed to $c{&bolt} inside the $c{if} block, so $n{*gear} now refers to $n{bolt}, not $n{nut}. Click $resetButton and try again.";
        }
        if (ctx.basicHintTopicIs("value", "bolt")) {
          return "$n{*gear} refers to $n{bolt}. The block runs with $n{washer} equal to $v{0}, then $v{1}, then $v{2}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int rivet = bolt % nut == 1 && spring;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "rivet")) {
          return "This is parsed as $c{((bolt % nut) == 1) && spring}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int screw = rivet || bolt / (nut - 6);\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "screw")) {
          return "$n{rivet} is non-zero, so the right side of the $c{||} is skipped. (Which is good, since $c{nut - 6} is $v{0}.)";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "51-gcd.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
