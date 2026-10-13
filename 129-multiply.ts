import { createMachineChallengeTemplate } from "./shared-machine-template.js";

const cases: Array<[number, number]> = [[3, 4], [0, 9], [9, 0], [1, 1], [7, 6], [15, 15], [12, 10], [2, 100]];

createMachineChallengeTemplate({
  instructions:
    "This machine has no instruction for multiplying (many real early processors didn't either), so multiplying takes a loop. When the program starts, $c{A} and $c{B} hold two numbers. Put $c{A * B} in $c{C}, and show it on the lights too. Every product here fits in a byte.\nThe registers' starting values don't have to survive: you're allowed to change $c{A} and $c{B}.",
  startCode: "; A and B hold the numbers to multiply.\n\n",
  cases: cases.map(([a, b]) => ({ registers: { A: a, B: b } })),
  solve: "        SET C, 0\n        JZ B, done\nloop:   ADD C, A\n        SUB B, 1\n        JNZ B, loop\ndone:   STORE [255], C\n        HALT\n",
  check: { lights: true, registers: ["C"] },
  hints: ({ error, final, start }) => {
    if (error) return error;
    if (final && start.registers.B === 0 && final.registers.C !== 0) return "When $c{B} is $v{0}, the product is $v{0}. Check for that before the loop starts.";
    return "Multiplying $c{A} by $c{B} is adding $c{A} to $c{C}, $c{B} times.";
  },
  next: "130-machine-memory.html",
});
