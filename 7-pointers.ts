import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions:
    "This is where addresses become relevant.\nNo other instructions for this one. Good luck!",
  steps: [
    { code: "int deer;\n", editable: false },
    { code: "int hare;\n", editable: false },
    { code: "int* wolf;\n", editable: false },
    { code: "wolf = &deer;\n", editable: false },
    {
      code: "wolf = &hare;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "wolf")) {
          return "$c{wolf = &deer;} set $n{wolf}'s value to $n{deer}'s address. What should $c{wolf = &hare;} do?";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int** bear;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "bear")) {
          const [bear, wolf] = ctx.boxesNamed("bear", "wolf");
          if (bear!.value === wolf!.value) {
            return "$n{bear} is being initialized with $c{&wolf}, not $c{wolf}. Set $n{bear}'s value to $n{wolf}'s address, not $n{wolf}'s value.";
          }
          return "Set $n{bear}'s value to $n{wolf}'s address.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "bear = &wolf;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "bear")) {
          const [bear, wolf] = ctx.boxesNamed("bear", "wolf");
          if (bear!.value === wolf!.value) {
            return "$n{bear} is being assigned $c{&wolf}, not $c{wolf}. Set $n{bear}'s value to $n{wolf}'s address, not $n{wolf}'s value.";
          }
          return "Set $n{bear}'s value to $n{wolf}'s address.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "int* fox = wolf;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "fox")) {
          const [fox, wolf] = ctx.boxesNamed("fox", "wolf");
          if (fox!.value === wolf!.address) {
            return "$n{fox} is being assigned to $c{wolf}, not $c{&wolf}. Set $n{fox}'s value to $n{wolf}'s value, not $n{wolf}'s address.";
          }
          return "Set $n{fox}'s value to $n{wolf}'s value.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "8-dereferencing-i.html",
  workspace: { allowVariableCreation: true },
});
