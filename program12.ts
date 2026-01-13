/// <reference path="./shared-program-template.ts" />

{
  const createProgramTemplate = window.MB!.createProgramTemplate!;

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
          const cinder = ctx.getBoxByName(ctx.boxes, "cinder");
          if (!cinder) {
            return "Add the $n{cinder} variable.";
          }
          if (cinder.type !== "int") {
            return "$n{cinder}'s type should be $t{int}.";
          }
          if (cinder.value !== "11") {
            return "Compute $n{cinder} using multiplication before addition. $c{/* 5 */} is a comment.";
          }
          return null;
        },
      },
      { code: "int* flame = &spark;\n", editable: false },
      {
        code: "ember = cinder / 3 + - 1;\n",
        editable: true,
        hints: (ctx) => {
          const ember = ctx.getBoxByName(ctx.boxes, "ember");
          if (ember!.value !== "2") {
            return "The order of operations is $c{(cinder / 3) + (-1)}.";
          }
          return null;
        },
      },
      {
        code: "*flame = spark * 4;\n",
        editable: true,
        hints: (ctx) => {
          const spark = ctx.getBoxByName(ctx.boxes, "spark");
          if (spark!.value !== "24") {
            return "$n{*flame} refers to the variable whose address is $n{flame}'s value. That variable should be set to the current value of $n{spark} times 4.";
          }
          return null;
        },
      },
      { code: "int* smolder = &ember;\n", editable: false },
      { code: "int** blaze = &smolder;\n", editable: false },
      { code: "int*** inferno = &blaze;\n", editable: false },
      {
        code: "**inferno = &cinder;\n",
        editable: true,
        hints: (ctx) => {
          const [smolder, ember, cinder] = ctx.getBoxesByName(
            ctx.boxes,
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
          return null;
        },
      },
      {
        code: "int ash = **blaze-*flame / ember + (***inferno == 24);\n",
        editable: true,
        instructions: "Sorry.",
        hints: (ctx) => {
          const ash = ctx.getBoxByName(ctx.boxes, "ash");
          if (!ash) {
            return "Add the $n{ash} variable.";
          }
          if (ash.type !== "int") {
            return "$n{ash}'s type should be $t{int}.";
          }
          if (ash.value !== "-1") {
            return "The order of operations is $c{(**blaze - (*flame / ember)) + (***inferno == 24)}.";
          }
          return null;
        },
      },
    ],
    next: "sandbox.html?finished=1",
    workspace: { allowVariableCreation: true },
  });
}
