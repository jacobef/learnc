import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "int" }],
  checkStdout: true,
  testInputs: Array.from({ length: 9 }, (_, n) => [String(n)]),
  startInput: ["5"],
  prelude: "int putchar(int c);\n",
  main: ([n]) => `int main(void) {\n  stars(${n});\n  return 0;\n}`,
  solve: "void stars(int n) {\n  for (int i = 0; i < n; i++) {\n    putchar('*');\n  }\n  putchar('\\n');\n}",
  textareaMinLines: 7,
  instructions:
    "Write a function named $c{stars} that prints $n{n} asterisks ($c{*}), followed by a newline. For example, $c{stars(5)} should print $c{*****} and then a newline.",
  hints: (ctx) => {
    if (ctx.behavesLike("void stars(int n) {\n  for (int i = 0; i < n; i++) {\n    putchar('*');\n  }\n}")) {
      return "Don't forget the newline at the end.";
    }
    if (ctx.behavesLike("void stars(int n) {\n  for (int i = 0; i < n; i++) {\n    putchar('*');\n    putchar('\\n');\n  }\n}")) {
      return "Only one newline should be printed, after all of the asterisks.";
    }
    return null;
  },
  next: "69-print-number.html",
});
