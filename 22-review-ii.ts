import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "No instructions for this one. Good luck!",
  steps: [
    { code: "double tide = 7.0;\n" },
    { code: "int wave = 4;\n" },
    {
      code: "int reef = 10 /* + 5 */;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "reef")) {
          const reef = ctx.boxNamed("reef")!;
          if (reef.value === "15") {
            return "$c{/* + 5 */} is a comment.";
          }
          return "$c{/* + 5 */} is a comment, so this is just $c{int reef = 10;}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "int* shore = &wave;\n" },
    { code: "{\n" },
    {
      code: "  double foam = *shore + 0.5;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "foam")) {
          const foam = ctx.boxNamed("foam")!;
          if (foam.value === "4" || foam.value === "4.0") {
            return "$n{*shore} is $n{wave}, whose value is $v{4}. Adding an $t{int} to a $t{double} gives a $t{double}: $v{4} + $v{0.5} = $v{4.5}.";
          }
          return "$n{*shore} refers to the variable whose address is $n{shore}'s value.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "  if (foam >= 5.0) {\n    reef = wave;\n  } else if (foam > 4.0) {\n    shore = &reef;\n  } else {\n    *shore = tide;\n  }\n",
      editable: true,
      hints: (ctx) => {
        const [reef, shore, wave] = ctx.boxesNamed("reef", "shore", "wave");
        if (reef!.value === wave!.value) {
          return "$n{foam} is $v{4.5}. Is $c{4.5 >= 5.0} true?";
        }
        if (wave!.value === "7") {
          return "$c{foam >= 5.0} is false (0), but $c{foam > 4.0} is true (1), so the $c{else if} block runs, not the $c{else} block.";
        }
        if (shore!.value !== reef!.address) {
          return "$c{foam >= 5.0} is false (0), but $c{foam > 4.0} is true (1). The $c{else if} block sets $n{shore} to $c{&reef}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "  *shore = 9 / foam;\n",
      editable: true,
      hints: (ctx) => {
        const [wave, reef] = ctx.boxesNamed("wave", "reef");
        if (wave!.value !== "4") {
          return "$n{shore} was changed to $c{&reef} in the previous step, so $n{*shore} now refers to $n{reef}, not $n{wave}. Click $resetButton and try again.";
        }
        if (reef!.value === "10") {
          return "$n{*shore} refers to the variable whose address is $n{shore}'s value. After the previous step, $n{shore} points to $n{reef}.";
        }
        if (reef!.value === "2.0") {
          return "$n{reef} is an $t{int}, so the decimal part should be dropped.";
        }
        if (ctx.basicHintTopicIs("value", "reef")) {
          return "$c{9} ($t{int}) divided by $n{foam} ($v{4.5}, $t{double}) uses $t{double} division: $c{9.0 / 4.5} = $v{2.0}. Then assigning to $n{reef} ($t{int}) drops the decimal.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "}\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.boxNamed("foam")) {
          return "$n{foam} was created inside this block, so it should be removed.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int sand = *shore * reef;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "sand")) {
          const sand = ctx.boxNamed("sand")!;
          if (sand.value === "20") {
            return "$n{reef}'s value was changed by $c{*shore = 9 / foam;}. It's no longer $v{10}.";
          }
          return "$n{*shore} refers to $n{reef}. So $c{*shore * reef} is $c{reef * reef}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "double* drift = &tide;\n" },
    {
      code: "*drift = sand + 1 / 2;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "tide")) {
          const tide = ctx.boxNamed("tide")!;
          if (tide.value === "4.5") {
            return "$c{1 / 2} is $t{int} division, which gives $v{0}, not $v{0.5}.";
          }
          if (tide.value === "4") {
            return "$n{tide} is a $t{double}, so $v{4} should be stored as $v{4.0}.";
          }
          return "$c{1 / 2} uses $t{int} division ($v{0}). $n{*drift} refers to $n{tide}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int coral = (reef != wave) + (*drift <= 4.0) + (sand - wave > -1);\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "coral")) {
          return "Evaluate each comparison: $c{reef != wave} ($v{2} != $v{4}), $c{*drift <= 4.0} ($v{4.0} <= $v{4.0}), $c{sand - wave > -1} ($v{4} - $v{4} > $v{-1}). Then add the results.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "sandbox.html?finished=1",
  isLast: true,
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
