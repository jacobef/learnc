import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "$c{typedef} gives a new name to an existing type. It's written exactly like a variable declaration, with $c{typedef} in front, and the name being declared becomes a name for the type the variable would have had.\nClick $runLineButton to continue.",
  steps: [
    { code: "typedef int score;\n" },
    { code: "typedef struct {\n  double x;\n  double y;\n} vec;\n\n" },
    { code: "typedef int row[3];\n\n" },
    { code: "int main(void) {\n" },
    {
      code: "  score s = 90;\n",
      instructions:
        "$c{typedef int score;} would declare a variable $n{score} of type $t{int}, so it makes $t{score} a name for $t{int}. It's still the same type, which is why the program state shows $t{int}.",
    },
    {
      code: "  vec v = {1.5, 2.0};\n",
      instructions:
        "Line 2 makes $t{vec} a name for a struct type that has no name of its own, so it's written without the $c{struct} keyword. This is very common.",
    },
    {
      code: "  row r = {1, 2, 3};\n",
      instructions: "$c{typedef int row[3];} would declare $n{row} as an array of 3 $t{int}s, so $t{row} is a name for $t{int[3]}.",
    },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "133-enums.html",
});
