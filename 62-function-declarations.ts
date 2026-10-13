import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "The compiler reads a file from top to bottom, and a function has to be declared before it's called. So far, that's been taken care of by defining each function above $c{main}.\nBut a function can also be declared without being defined, by writing just its first line, followed by a semicolon. This is called a function declaration, or a prototype. It tells the compiler the function's name, its return type, and its parameters' types: everything needed to check a call to it. The function's body can then come later.\nIn this program, line 1 declares $c{cube}, and its definition comes after $c{main}, at the bottom of the program. (You might have to scroll down to see it.)\nClick $runLineButton to continue.",
  steps: [
    { code: "int cube(int n);\n\n" },
    { code: "int main(void) {\n" },
    { code: "  int a;\n" },
    {
      code: "  a = cube(3);\n",
      editable: [true, false],
      instructions: ["Calling $c{cube} works the same as before. What should the program state look like right after entering $c{cube}? Also select where execution continues: $c{cube}'s body is at the bottom."],
      hints: (ctx) => {
        if (ctx.basicHintTopicIs("count") || ctx.basicHintTopicIs("name")) {
          return "The call creates $c{cube}'s parameter, $n{n}.";
        }
        return ctx.basicHint;
      },
    },
    { code: "  return 0;\n" },
    { code: "}\n\n" },
    { code: "int cube(int n) {\n" },
    {
      code: "  return n * n * n;\n",
      instructions: "Calling $c{cube} jumped to its definition, even though the definition comes after $c{main} in the file.",
    },
    { code: "}\n" },
  ],
  next: "63-declare.html",
  workspace: { allowVariableCreation: true, allowVariableDeletion: true },
});
