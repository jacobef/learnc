import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    { code: "int coins = 10;\n" },
    {
      code: "coins += 5;\n",
      instructions: "$c{x += y;} is shorthand for $c{x = x + y;}.",
    },
    {
      code: "coins -= 3;\n",
      editable: true,
      instructions: "Similarly, $c{x -= y;} is shorthand for $c{x = x - y;}.",
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "coins *= 2 + 1;\n",
      editable: true,
      instructions: "$c{*=}, $c{/=}, and $c{%=} work the same way.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "coins")) {
          const coins = ctx.boxNamed("coins")!;
          if (coins.value === "25") {
            return "$c{coins *= 2 + 1;} means $c{coins = coins * (2 + 1);}, not $c{coins = coins * 2 + 1;}. The whole right side is computed first.";
          }
          return "$c{coins *= 2 + 1;} means $c{coins = coins * (2 + 1);}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "coins /= 5;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "coins")) {
          return "$c{coins /= 5;} means $c{coins = coins / 5;}. Remember that $t{int} division drops the decimal part.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "coins %= 4;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "coins")) {
          return "$c{coins %= 4;} means $c{coins = coins % 4;}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "coins++;\n",
      instructions: "$c{x++;} is shorthand for $c{x += 1;}. This is called incrementing.",
    },
    {
      code: "coins--;\n",
      editable: true,
      instructions: "$c{x--;} is shorthand for $c{x -= 1;}. This is called decrementing.",
      hints: (ctx) => ctx.basicHint,
    },
    { code: "double gems = 1.5;\n" },
    {
      code: "gems *= coins;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "gems")) {
          return "$c{gems *= coins;} means $c{gems = gems * coins;}. Multiplying a $t{double} by an $t{int} gives a $t{double}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "gems++;\n",
      editable: true,
      hints: (ctx) => ctx.basicHint,
    },
    { code: "int* purse = &coins;\n" },
    {
      code: "*purse += 10;\n",
      editable: true,
      hints: (ctx) => {
        const [coins, purse] = ctx.boxesNamed("coins", "purse");
        if (purse!.value !== coins!.address) {
          return "$c{*purse += 10;} means $c{*purse = *purse + 10;}. It changes $n{*purse}, not $n{purse}. Click $resetButton and try again.";
        }
        if (ctx.basicHintTopicIs("value", "coins")) {
          return "$c{*purse += 10;} means $c{*purse = *purse + 10;}. $n{*purse} refers to the variable whose address is $n{purse}'s value.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "++coins;\n",
      editable: true,
      instructions:
        "$c{++x;} also increments $n{x}. There is a difference between $c{x++} and $c{++x}, but it only matters when they're used inside a larger expression, which we won't be doing for a while.",
      hints: (ctx) => ctx.basicHint,
    },
  ],
  next: "45-sum.html",
  workspace: { allowVariableCreation: true },
});
