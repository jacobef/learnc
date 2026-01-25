import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "No instructions for this one. Good luck!",
  steps: [
    {
      code: "int spark = 6;\n",
      editable: false,
    },
    { code: "int ember; // will be set later\n", editable: false },
    {
      code: "int cinder = 2 + 3 /* 5 */ * (4 - 1);\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "cinder")) {
          const cinder = ctx.boxNamed("cinder")!;
          if (cinder.value === "15") {
            return "You're parsing it as $c{(2 + 3) * (4 - 1)}, but it should be parsed as $c{2 + (3 * (4 - 1))}. Multiplication has higher precedence than addition.";
          }
          return "Compute $n{cinder} using multiplication before addition. $c{/* 5 */} is a comment.";
        }
        return ctx.basicHint;
      },
    },
    { code: "int* flame = &spark;\n", editable: false },
    {
      code: "ember = cinder / 3 + - 1;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "ember")) {
          return "This is parsed as $c{(cinder / 3) + (-1)}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "*flame = spark * 4;\n",
      editable: true,
      hints: (ctx) => {
        const spark = ctx.boxNamed("spark");
        if (spark!.value !== "24") {
          return "$n{*flame} refers to the variable whose address is $n{flame}'s value. That variable should be set to the current value of $n{spark} times 4.";
        }
        return ctx.basicHint;
      },
    },
    { code: "int* smolder = &ember;\n", editable: false },
    { code: "int** blaze = &smolder;\n", editable: false },
    { code: "int*** inferno = &blaze;\n", editable: false },
    {
      code: "**inferno = &cinder;\n",
      editable: true,
      hints: (ctx) => {
        const [smolder, ember, cinder] = ctx.boxesNamed(
          "smolder",
          "ember",
          "cinder",
        );
        if (smolder!.value === ember!.address) {
          return "$n{**inferno} refers to the variable whose address is $n{*inferno}'s value. $n{*inferno} refers to the variable whose address is $n{inferno}'s value.";
        }
        if (smolder!.value !== cinder!.address) {
          return `$n{**inferno} (aka $n{smolder}) should be set to $n{cinder}'s address. I'm not sure where $v{${smolder!.value}} is coming from.`;
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int ash = **blaze-*flame / ember + (***inferno == 24);\n",
      editable: true,
      instructions: "Sorry.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "ash")) {
          return "The order of operations is $c{(**blaze-(*flame / ember)) + (***inferno == 24)}.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "14-expressions-doubles.html",
  workspace: { allowVariableCreation: true },
});
