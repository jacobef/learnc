/// <reference path="./shared-program-template.ts" />

{
  const createProgramTemplate = window.MB!.createProgramTemplate!;

  createProgramTemplate({
    initialInstructions: "No instructions for this one. Good luck!",
    steps: [
      {
        code: "int hammer;\n",
        editable: false,
      },
      { code: "int drill = 1;\n", editable: false },
      { code: "hammer = drill;\n", editable: false },
      { code: "drill = 2;\n", editable: false },
      {
        code: "drill = hammer;\n",
        editable: true,
        hints: (ctx) => {
          const [drill, hammer] = ctx.getBoxesByName(
            ctx.boxes,
            "drill",
            "hammer",
          );
          const drillValue = drill!.value;
          const hammerValue = hammer!.value;
          if (drillValue.toLowerCase() === "hammer") {
            return `$n{drill}'s value should be set to $n{hammer}'s value, not the literal word "hammer".`;
          }
          if (hammerValue !== "1") {
            return "$c{drill = hammer;} should modify $n{drill}, not $n{hammer}. Click $resetButton and try again.";
          }
          if (drillValue !== "1") {
            return "$c{hammer = drill;} put $n{drill}'s value into $n{hammer}. What should $c{drill = hammer;} do?";
          }
          return null;
        },
      },
    ],
    next: "program7.html",
    workspace: { allowVariableCreation: true },
  });
}
