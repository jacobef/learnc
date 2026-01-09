{
  const { createProgramTemplate } = window.MB;

  createProgramTemplate({
    lines: [
      "int spark = 6;",
      "int ember; // will be set later",
      "int cinder = 2 + 3 /* 5 */ * (4 - 1);",
      "int* flame = &spark;",
      "ember = cinder / 3 + - 1;",
      "*flame = spark * 4;",
      "int* smolder = &ember;",
      "int** blaze = &smolder;",
      "int*** inferno = &blaze;",
      "**inferno = &cinder;",
      "int ash = **blaze-*flame / ember + (***inferno == 24);",
    ],
    editableSteps: [3, 5, 6, 10, 11],
    stepperFallback: false,
    next: "sandbox.html?finished=1",
    workspace: { allowAddAndDelete: true },
    instructions: (ctx) => {
      if (ctx.boundary === 0) return "No instructions for this one. Good luck!";
      if (ctx.boundary === 11) return "Sorry.";
      return null;
    },
    hints: {
      3: (ctx) => {
        const by = ctx.byName || {};
        if (!by.cinder) {
          return "Add the $n{cinder} variable.";
        }
        if (by.cinder.type !== "int") {
          return "$n{cinder}'s type should be $t{int}.";
        }
        if (by.cinder.value !== "11") {
          return "Compute $n{cinder} using multiplication before addition. $c{/* 5 */} is a comment.";
        }
        return null;
      },
      5: (ctx) => {
        const by = ctx.byName || {};
        if (by.ember.value !== "2") {
          return "The order of operations is $c{(cinder / 3) + (-1)}.";
        }
        return null;
      },
      6: (ctx) => {
        const by = ctx.byName || {};
        if (by.spark.value !== "24") {
          return "$n{*flame} refers to the variable whose address is $n{flame}'s value. That variable should be set to the current value of $n{spark} times 4.";
        }
        return null;
      },
      10: (ctx) => {
        const by = ctx.byName || {};
        if (by.smolder.value === by.ember.address) {
          return "$n{**inferno} refers to the variable whose address is $n{*inferno}'s value. $n{*inferno} refers to the variable whose address is $n{inferno}'s value.";
        }
        if (by.smolder.value !== by.cinder.address) {
          return `$n{**inferno} (aka $n{smolder}) should be set to $n{cinder}'s address. I'm not sure where $v{${by.smolder.value}} is coming from.`;
        }
        return null;
      },
      11: (ctx) => {
        const by = ctx.byName || {};
        if (!by.ash) {
          return "Add the $n{ash} variable.";
        }
        if (by.ash.type !== "int") {
          return "$n{ash}'s type should be $t{int}.";
        }
        if (by.ash.value !== "-1") {
          return "The order of operations is $c{(**blaze - (*flame / ember)) + (***inferno == 24)}.";
        }
        return null;
      },
    },
  });
}
