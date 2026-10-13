import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    { code: "int a[4] = {3, 1, 4, 1};\n" },
    {
      code: "int* p = a;\n",
      editable: true,
      instructions: "Recall that $c{a} decays into $c{&a[0]} here.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "p")) {
          return "$c{a} decays into a pointer to its first element, so $n{p}'s value should be $c{a[0]}'s address.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "p[2] = 9;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "a[2]")) {
          return "$c{p[2]} means $c{*(p + 2)}, and $n{p} points to $c{a[0]}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "p++;\n",
      editable: true,
      instructions: "$c{p++;} means $c{p = p + 1;}, which moves $n{p} to the next element.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "p")) {
          return "$c{p + 1} points one element further, which is 4 bytes further, since each $t{int} takes up 4 bytes.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "*p = p[1] + 1;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "a[1]")) {
          return "$n{p} now points to $c{a[1]}, so $c{p[1]} is $c{a[2]}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "int sum = 0;\n" },
    {
      code: "for (int* q = a; q < a + 4; q++) {\n  sum += *q;\n}\n",
      editable: true,
      instructions:
        "Pointers into the same array can be compared: $c{q < a + 4} checks whether $n{q} points to an element before index 4. ($c{a + 4} points just past the end of the array. That's allowed, as long as it's never dereferenced.)\nWhat should the program state look like after this loop finishes?",
      hints: (ctx) => {
        if (ctx.boxNamed("q")) {
          return "$n{q} was created in the loop's setup, so it should be removed when the loop finishes.";
        }
        if (ctx.basicHintTopicIs("value", "sum")) {
          return "$n{q} visits each element in turn, and $c{sum += *q;} adds that element to $n{sum}.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "88-malloc.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
