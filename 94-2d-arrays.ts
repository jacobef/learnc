import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  initialInstructions: "Click $runLineButton to continue.",
  steps: [
    {
      code: "int m[2][3] = {{1, 2, 3}, {4, 5, 6}};\n",
      instructions:
        "Read the declaration the usual way: $c{m[i][j]} is an $t{int}. So $c{m[i]} is an array of 3 $t{int}s, and $n{m} is an array of 2 of those arrays. Its type is $t{int[2][3]}.\nThe arrays inside $n{m} are its rows: $c{m[0]} is $c{\\{1, 2, 3\\}} and $c{m[1]} is $c{\\{4, 5, 6\\}}. In memory, they're stored one after the other.",
    },
    {
      code: "m[1][2] = 0;\n",
      editable: true,
      instructions: "$c{m[1][2]} means $c{(m[1])[2]}: element 2 of row 1.",
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "m[0][1] += m[1][0];\n",
      editable: true,
      hints: (ctx) => ctx.basicHint,
    },
    {
      code: "int *p = m[1];\n",
      editable: true,
      instructions:
        "$c{m[1]} is an array (row 1), so it decays into a pointer to its first element, $c{m[1][0]}.",
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "p")) {
          return "$n{p}'s value should be the address of $c{m[1][0]}.";
        }
        return ctx.basicHint;
      },
    },
    {
      code: "p[1] = 9;\n",
      editable: true,
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value")) {
          return "$n{p} points to $c{m[1][0]}, so $c{p[1]} is $c{m[1][1]}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "int total = 0;\n" },
    {
      code: "for (int i = 0; i < 2; i++) {\n  for (int j = 0; j < 3; j++) {\n    total += m[i][j];\n  }\n}\n",
      editable: true,
      instructions:
        "A loop inside another loop is called a nested loop. Here, the inner loop runs completely for each run of the outer loop's block, so together they visit every element of $n{m}.\nWhat should the program state look like after these loops finish?",
      hints: (ctx) => {
        if (ctx.boxNamed("i") || ctx.boxNamed("j")) {
          return "$n{i} and $n{j} were created in the loops' setups, so they should be removed when the loops finish.";
        }
        if (ctx.basicHintTopicIs("value", "total")) {
          return "$n{total} ends up as the sum of all of $n{m}'s elements.";
        }
        return ctx.basicHint;
      },
    },
  ],
  next: "95-array-parameters.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
