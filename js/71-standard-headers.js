import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const program = "int main(void) {\n  putchar('H');\n  putchar('i');\n  putchar('\\n');\n  return 0;\n}\n";
createCodeOutputChallengeTemplate({
    checkStdout: true,
    implicitMain: false,
    startCode: program,
    solve: `#include <stdio.h>\n\n${program}`,
    textareaMinLines: 9,
    instructions: "Declaring standard library functions by hand gets tedious, and it's easy to get a declaration wrong. So C comes with header files that declare them for you. $c{putchar} is declared in $c{stdio.h} (short for \"standard input and output\"), along with many other functions for input and output.\nA standard header is included with angle brackets instead of quotes: $c{#include <stdio.h>}. Make this program compile by including it.",
    hints: (ctx) => ctx.text.includes('"stdio.h"')
        ? "Standard headers use angle brackets: $c{#include <stdio.h>}."
        : null,
    next: "72-multiple-files.html",
});
