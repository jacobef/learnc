import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const program = "int main(void) {\n  int result = square(3) + cube(2);\n  return 0;\n}\n";
createCodeOutputChallengeTemplate({
    outputs: [{ name: "result", type: "int" }],
    implicitMain: false,
    files: [
        { path: "powers.h", source: "int square(int n);\nint cube(int n);\n" },
        {
            path: "powers.c",
            source: "#include \"powers.h\"\n\nint square(int n) {\n  return n * n;\n}\n\nint cube(int n) {\n  return n * n * n;\n}\n",
        },
    ],
    fileName: "main.c",
    startCode: program,
    solve: `#include "powers.h"\n\n${program}`,
    textareaMinLines: 7,
    instructions: "Writing out declarations in every file that needs them would be repetitive and error-prone. Instead, a file's declarations are usually collected in a matching header file, like $c{powers.h} here. Every file that uses those functions includes the header, and so does the file that defines them. That way, if a definition ever stops matching its declaration, the compiler catches it.\nMake $c{main.c} work by including $c{powers.h}.",
    next: "74-define.html",
});
