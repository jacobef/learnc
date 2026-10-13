import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";

const values = [0, 1, 2, 3, 6, 128, 255, 0b10110000, 0b00011101, 0b01010101, 0b11110000, 37];

createCodeOutputChallengeTemplate({
  inputs: [{ name: "n", type: "unsigned char" }],
  outputs: [{ name: "lights", type: "unsigned char" }],
  testInputs: values.map((value) => [String(value)]),
  startInput: ["6"],
  solve: "unsigned char lights = 0;\nfor (int i = 0; i < 8; i++) {\n  if ((n >> i) & 1) {\n    lights |= 1 << (7 - i);\n  }\n}",
  textareaMinLines: 8,
  instructions:
    "Write code that creates an $t{unsigned char} variable named $n{lights}, holding $n{n}'s bits in reverse order, as if the row of lights were seen in a mirror: bit 0 of $n{n} becomes bit 7 of $n{lights}, bit 1 becomes bit 6, and so on. For example, if $n{n} is $c{0b00000110} ($v{6}), $n{lights} should be $c{0b01100000} ($v{96}).",
  hints: (ctx) =>
    ctx.currentResult.kind === "wrong-output-value"
      ? "Go through the bits one at a time, with $c{i} from $v{0} to $v{7}: if bit $c{i} of $n{n} is set, set bit $c{7 - i} of $n{lights}."
      : null,
  next: "134-c-to-assembly.html",
});
