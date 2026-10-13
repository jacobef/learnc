import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    stdin: "12 30\nAda\n",
    initialInstructions: "Besides standard output, every program has standard input, or stdin. When a program runs in a terminal, stdin is what the user types. Here, it's already been typed in, as shown above the output.\n$c{scanf} is the reverse of $c{printf}: it reads input according to a format string, and stores what it reads in the variables it's given pointers to.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdio.h>\n\n" },
        { code: "int main(void) {\n" },
        { code: "  int a;\n" },
        { code: "  int b;\n" },
        {
            code: "  int read = scanf(\"%d %d\", &a, &b);\n",
            editable: true,
            instructions: "This reads two $t{int}s. A space in the format string skips any amount of whitespace, including newlines. $c{scanf} returns how many values it stored, which is how a program can tell whether the input was valid. What should the program state look like?",
            hints: (ctx) => ctx.basicHintTopicIs("value", "read") ? "$c{scanf} stored two values." : ctx.basicHint,
        },
        { code: "  printf(\"%d\\n\", a + b);\n" },
        {
            code: "  int c = getchar();\n",
            editable: true,
            instructions: "$c{getchar} reads one character. $c{scanf} stopped right after $c{30}, so the next character is the newline after it, which catches a lot of programs by surprise.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "c") ? "The newline character's code is $v{10}." : ctx.basicHint,
        },
        { code: "  char name[20];\n" },
        {
            code: "  fgets(name, sizeof name, stdin);\n",
            instructions: "$c{fgets} reads a whole line, including its newline, into an array, but never more than the size it's given (leaving room for the $c{'\\0'}). That makes it safer for reading strings than $c{scanf}.",
        },
        {
            code: "  printf(\"Hi, %s\", name);\n",
            editable: true,
            hints: basic,
        },
        {
            code: "  return 0;\n",
            instructions: "When there's no input left, $c{getchar} returns $c{EOF} (\"end of file\"), a negative number, and $c{fgets} returns $c{NULL}.",
        },
        { code: "}\n" },
    ],
    next: "141-files.html",
    workspace: { showOutput: true, hideStringLiterals: true },
});
