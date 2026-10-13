import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "An array can't be passed to a function, but a pointer to its first element can. Since an array decays into exactly that pointer, passing an array looks easy.\nClick $runLineButton to continue.",
  steps: [
    { code: "int sum(int *values, int count) {\n" },
    { code: "  int total = 0;\n" },
    { code: "  for (int i = 0; i < count; i++) {\n    total += values[i];\n  }\n" },
    { code: "  return total;\n" },
    { code: "}\n\n" },
    { code: "void fill(int a[], int count, int value) {\n" },
    {
      code: "  for (int i = 0; i < count; i++) {\n    a[i] = value;\n  }\n",
      editable: true,
      instructions:
        "What should the program state look like after this loop finishes? Remember that $n{a} points to an element of $c{main}'s array.",
      hints: (ctx) => {
        if (ctx.boxNamed("i")) {
          return "$n{i} was created in the loop's setup, so it should be removed when the loop finishes.";
        }
        if (ctx.basicHintTopicIs("value")) {
          return "$c{a[i]} means $c{*(a + i)}, and $n{a} points to $c{data[0]}. So the loop changes $c{main}'s array, $n{data}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "}\n\n" },
    { code: "int main(void) {\n" },
    { code: "  int data[4] = {2, 4, 6, 8};\n" },
    { code: "  int s;\n" },
    {
      code: "  s = sum(data, 4);\n",
      instructions: [
        "$c{data} decays into $c{&data[0]}, so $n{values} is a pointer to $n{data}'s first element. The array itself isn't copied.\nNotice that $c{sum} also needs to be told how many elements there are: all it has is a pointer, which doesn't know how long the array is.",
      ],
    },
    {
      code: "  fill(data, 2, 9);\n",
      instructions: [
        "Look at $c{fill}'s first parameter: $c{int a[]}. In a parameter list, and only there, this is just another way to write $c{int *a}. $n{a} is a pointer, as its type shows.",
      ],
    },
    {
      code: "  s = sum(data + 2, 2);\n",
      editable: [true, false],
      instructions: [
        "A function that takes a pointer to an element can be given any element, not just the first. What should the program state look like right after entering $c{sum}? Also select where execution continues.",
      ],
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("value", "values")) {
          return "$c{data + 2} points to $c{data[2]}.";
        }
        if (ctx.basicHintTopicIs("count") || ctx.basicHintTopicIs("name")) {
          return "The call creates $c{sum}'s parameters, $n{values} and $n{count}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "96-functions-and-arrays.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
