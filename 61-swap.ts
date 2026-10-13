import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [
    { name: "x", type: "int" },
    { name: "y", type: "int" },
  ],
  outputs: [
    { name: "x", type: "int" },
    { name: "y", type: "int" },
  ],
  testInputs: [["1", "2"], ["2", "1"], ["5", "5"], ["-4", "9"], ["0", "100"]],
  startInput: ["1", "2"],
  main: ([x, y]) => `int main(void) {\n  int x = ${x};\n  int y = ${y};\n  swap(&x, &y);\n  return 0;\n}`,
  solve: "void swap(int* a, int* b) {\n  int saved = *a;\n  *a = *b;\n  *b = saved;\n}",
  textareaMinLines: 7,
  instructions:
    "Write a function named $c{swap} that swaps the values of the two variables it's given pointers to. For example, if $n{x} is $v{1} and $n{y} is $v{2}, then after $c{swap(&x, &y)}, $n{x} should be $v{2} and $n{y} should be $v{1}.",
  hints: (ctx) => {
    if (ctx.behavesLike("void swap(int* a, int* b) {\n  *a = *b;\n  *b = *a;\n}")) {
      return "After $c{*a = *b;}, $n{*a}'s original value is gone. Save it in another variable first.";
    }
    if (ctx.behavesLike("void swap(int* a, int* b) {\n  int* saved = a;\n  a = b;\n  b = saved;\n}")) {
      return "That swaps the pointers $n{a} and $n{b}, which are just $c{swap}'s own copies. Swap the variables they point to instead: $n{*a} and $n{*b}.";
    }
    return null;
  },
  next: "62-function-declarations.html",
});
