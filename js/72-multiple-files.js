import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const program = "int main(void) {\n  int result = square(7);\n  return 0;\n}\n";
createCodeOutputChallengeTemplate({
    outputs: [{ name: "result", type: "int" }],
    implicitMain: false,
    files: [{ path: "square.c", source: "int square(int n) {\n  return n * n;\n}\n" }],
    fileName: "main.c",
    startCode: program,
    solve: `int square(int n);\n\n${program}`,
    textareaMinLines: 7,
    instructions: "A program can be split into several $c{.c} files. Each one is compiled separately, and then the compiled pieces are combined into one program. Combining them is called linking.\nWhile the compiler is compiling $c{main.c}, it can't see $c{square.c} at all. So $c{main.c} needs a declaration of $c{square}: a promise that a function with that name and type is defined somewhere, possibly in another file. Then the linker connects the call to the definition. Add the declaration to make the program work.",
    next: "73-headers.html",
});
