/// <reference path="./shared-program-template.ts" />

{
  const createProgramTemplate = window.MB!.createProgramTemplate!;

  createProgramTemplate({
    initialInstructions: "No instructions for this one. Good luck!",
    steps: [
      {
        code: "int north;\n",
        editable: true,
        hints: (ctx) => ctx.basicHint,
      },
      { code: "int east = 9;\n", editable: false },
      {
        code: "north = 5;\n",
        editable: true,
        hints: (ctx) => ctx.basicHint,
      },
      { code: "int south = -5;\n", editable: false },
      {
        code: "int west = -9;\n",
        editable: true,
        hints: (ctx) => ctx.basicHint,
      },
    ],
    next: "program4.html",
    workspace: { allowVariableCreation: true },
  });
}
