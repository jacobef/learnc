import { createProgramTemplate } from "./shared-program-template.js";

createProgramTemplate({
  implicitMain: false,
  initialInstructions:
    "When a program is run from a terminal, it can be given words after its name, like $c{./greet Ada Lovelace}. These are called command-line arguments, and $c{main} can receive them by taking two parameters instead of $c{void}: $n{argc}, how many strings there are, and $n{argv}, an array of pointers to them. $c{argv[0]} is normally the program's own name, with the arguments after it, so for $c{./greet Ada Lovelace}, $n{argc} is $v{3}.\nIn this tutorial, programs are run without any extra arguments.\nClick $runLineButton to continue.",
  steps: [
    { code: "#include <stdio.h>\n\n" },
    {
      code: "int main(int argc, char *argv[]) {\n",
    },
    {
      code: "  for (int i = 0; i < argc; i++) {\n",
      instructions: [
        "In a parameter list, $c{char *argv[]} means $c{char **argv}: a pointer to the first element of an array of pointers.",
        "",
      ],
    },
    { code: "    printf(\"argument %d: %s\\n\", i, argv[i]);\n" },
    { code: "  }\n" },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "157-review-v.html",
  workspace: { showOutput: true, hideStringLiterals: true },
});
