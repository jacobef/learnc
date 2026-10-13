import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "text", type: "char[]" }],
  outputs: [{ name: "n", type: "int" }],
  testInputs: ['""', '"a"', '"hello"', '"C Boxes"', '"  two spaces"', '"abc\\0def"'].map((s) => [s]),
  startInput: ['"hello"'],
  main: ([text]) => `int main(void) {\n  int n = length(${text});\n  return 0;\n}`,
  solve: "int length(char *s) {\n  int count = 0;\n  while (s[count] != '\\0') {\n    count++;\n  }\n  return count;\n}",
  textareaMinLines: 8,
  instructions:
    "Write a function named $c{length} that takes a pointer to the first character of a string, and returns the string's length.\nNotice that $c{main} passes a string literal directly. Like any array, it decays into a pointer to its first element.",
  hints: (ctx) => {
    if (ctx.currentResult.kind === "wrong-output-value") {
      return "Count characters until you reach $c{'\\0'}. Remember that $c{s[i]} works on a pointer too.";
    }
    return null;
  },
  next: "104-my-puts.html",
});
