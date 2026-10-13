import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [
    { name: "ax", type: "double" },
    { name: "ay", type: "double" },
    { name: "bx", type: "double" },
    { name: "by", type: "double" },
  ],
  outputs: [
    { name: "mx", type: "double" },
    { name: "my", type: "double" },
  ],
  testInputs: [
    ["0.0", "0.0", "2.0", "4.0"],
    ["1.0", "1.0", "1.0", "1.0"],
    ["-3.0", "5.0", "3.0", "-5.0"],
    ["0.5", "2.5", "1.5", "0.5"],
    ["10.0", "0.0", "0.0", "10.0"],
  ],
  startInput: ["0.0", "0.0", "2.0", "4.0"],
  prelude: "struct point {\n  double x;\n  double y;\n};\n",
  main: ([ax, ay, bx, by]) =>
    `int main(void) {\n  struct point a = {${ax}, ${ay}};\n  struct point b = {${bx}, ${by}};\n  struct point m = midpoint(a, b);\n  double mx = m.x;\n  double my = m.y;\n  return 0;\n}`,
  solve: "struct point midpoint(struct point a, struct point b) {\n  struct point m = {(a.x + b.x) / 2, (a.y + b.y) / 2};\n  return m;\n}",
  textareaMinLines: 5,
  instructions:
    "Write a function named $c{midpoint} that takes two $t{struct point}s and returns the point halfway between them. (Its $n{x} is the average of their $n{x}s, and likewise for $n{y}.)",
  hints: (ctx) =>
    ctx.currentResult.kind === "wrong-output-value"
      ? "The average of two numbers is their sum divided by $v{2}."
      : null,
  next: "144-linked-lists.html",
});
