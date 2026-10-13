import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const program = "int main(void) {\n  say_ok();\n  return 0;\n}\n\nvoid say_ok(void) {\n  putchar('O');\n  putchar('K');\n  putchar('\\n');\n}\n";
createCodeOutputChallengeTemplate({
    checkStdout: true,
    implicitMain: false,
    files: [{ path: "say.h", source: "int putchar(int c);\nvoid say_ok(void);\n" }],
    fileName: "main.c",
    startCode: program,
    solve: `#include "say.h"\n\n${program}`,
    textareaMinLines: 13,
    instructions: "Before compiling, C runs your code through a preprocessor, which handles lines that start with $c{#}, called directives. The most common one is $c{#include \"file\"}, which the preprocessor replaces with the entire contents of that file.\nThis project has another file, $c{say.h}, containing the declarations of $c{putchar} and $c{say_ok}. $c{main.c} calls $c{say_ok} before $c{say_ok} is defined, without declaring it first, so it doesn't compile. Fix it by adding $c{#include \"say.h\"} on its own line, at the top of $c{main.c}.",
    next: "71-standard-headers.html",
});
