import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
createCodeOutputChallengeTemplate({
    inputs: [{ name: "text", type: "char[]" }],
    checkStdout: true,
    testInputs: ['"Hello"', '""', '"C Boxes"', '"two\\nlines"', '"a"'].map((s) => [s]),
    startInput: ['"Hello"'],
    prelude: "int putchar(int c);\n",
    main: ([text]) => `int main(void) {\n  my_puts(${text});\n  return 0;\n}`,
    solve: "void my_puts(char *s) {\n  for (int i = 0; s[i] != '\\0'; i++) {\n    putchar(s[i]);\n  }\n  putchar('\\n');\n}",
    textareaMinLines: 7,
    instructions: "Write a function named $c{my_puts} that takes a pointer to the first character of a string, prints the string's characters, then prints a newline.",
    hints: (ctx) => {
        if (ctx.behavesLike("void my_puts(char *s) {\n  for (int i = 0; s[i] != '\\0'; i++) {\n    putchar(s[i]);\n  }\n}")) {
            return "Don't forget the newline at the end.";
        }
        return null;
    },
    next: "105-puts.html",
});
