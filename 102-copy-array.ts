import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

createCodeOutputChallengeTemplate({
  inputs: [{ name: "a", type: "int[5]" }],
  outputs: [{ name: "b", type: "int[5]" }],
  testInputs: ["{1, 2, 3, 4, 5}", "{0, 0, 0, 0, 0}", "{-7, 3, 0, 12, 9}", "{5, 5, 5, 5, 6}"].map((a) => [a]),
  startInput: ["{1, 2, 3, 4, 5}"],
  startCode: "int b[5] = a;\n",
  solve: "int b[5];\nfor (int i = 0; i < 5; i++) {\n  b[i] = a[i];\n}\n",
  textareaMinLines: 6,
  instructions:
    "This code is supposed to create an array $n{b} holding a copy of $n{a}'s elements, but it doesn't compile. Fix it.",
  next: "103-length.html",
});
