import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  checkStdout: true,
  inputs: [{ name: "n", type: "int" }],
  testInputs: Array.from({ length: 31 }, (_, n) => [String(n)]),
  startInput: ["15"],
  prelude: "#include <stdio.h>\n",
  main: ([n]) => `int main(void) {\n  fizzbuzz(${n});\n  return 0;\n}`,
  solve: `void fizzbuzz(int n) {
  for (int i = 1; i <= n; i++) {
    if (i % 15 == 0) {
      printf("FizzBuzz\\n");
    } else if (i % 3 == 0) {
      printf("Fizz\\n");
    } else if (i % 5 == 0) {
      printf("Buzz\\n");
    } else {
      printf("%d\\n", i);
    }
  }
}`,
  textareaMinLines: 14,
  instructions:
    "A classic: write a function named $c{fizzbuzz} that prints the numbers from $v{1} to $n{n}, one per line, except that multiples of 3 are replaced by $c{Fizz}, multiples of 5 by $c{Buzz}, and multiples of both by $c{FizzBuzz}.\nFor example, $c{fizzbuzz(6)} prints $c{1}, $c{2}, $c{Fizz}, $c{4}, $c{Buzz}, $c{Fizz}, each followed by a newline.",
  hints: (ctx) => {
    if (ctx.behavesLike(`void fizzbuzz(int n) {
  for (int i = 1; i <= n; i++) {
    if (i % 3 == 0) {
      printf("Fizz\\n");
    } else if (i % 5 == 0) {
      printf("Buzz\\n");
    } else if (i % 15 == 0) {
      printf("FizzBuzz\\n");
    } else {
      printf("%d\\n", i);
    }
  }
}`)) {
      return "Multiples of 15 are also multiples of 3, so they never reach the $c{FizzBuzz} check. Check for multiples of 15 first.";
    }
    if (ctx.currentResult.kind === "wrong-stdout") {
      return "Compare your output with the expected output line by line. Is each line followed by a newline?";
    }
    return null;
  },
  next: "111-integer-types.html",
});
