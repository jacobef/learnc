import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Files work like stdin and stdout. $c{fopen} opens a file and returns a $t{FILE *}, a pointer to a struct the standard library uses to keep track of it. (Its members are private; you only ever use the pointer.) Then there's a version of each input and output function that takes the $t{FILE *}: $c{fprintf}, $c{fscanf}, $c{fgets}, $c{fputc}, and so on.\nIn fact, $c{stdin} and $c{stdout} are $t{FILE *}s too, opened automatically when the program starts.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdio.h>\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  FILE *out = fopen(\"scores.txt\", \"w\");\n",
            instructions: "The second argument is the mode: $c{\"w\"} creates the file (or empties it, if it exists) for writing, $c{\"r\"} opens it for reading, and $c{\"a\"} opens it for adding to the end.",
        },
        {
            code: "  if (out == NULL) {\n",
            instructions: "$c{fopen} returns $c{NULL} if the file can't be opened, for example if it doesn't exist or the program isn't allowed to access it. Always check.",
        },
        { code: "    return 1;\n" },
        { code: "  }\n" },
        { code: "  fprintf(out, \"%d %d\\n\", 90, 85);\n" },
        {
            code: "  fclose(out);\n",
            instructions: "$c{fclose} finishes writing and closes the file. Like $c{free}, every successful $c{fopen} needs a matching $c{fclose}.",
        },
        { code: "  FILE *in = fopen(\"scores.txt\", \"r\");\n" },
        { code: "  int first;\n" },
        { code: "  int second;\n" },
        { code: "  fscanf(in, \"%d %d\", &first, &second);\n" },
        { code: "  fclose(in);\n" },
        { code: "  printf(\"average: %d\\n\", (first + second) / 2);\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "142-command-line.html",
    workspace: { showOutput: true, hideStringLiterals: true },
});
