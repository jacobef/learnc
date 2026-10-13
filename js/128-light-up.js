import { createMachineChallengeTemplate } from "./shared-machine-template.js";
createMachineChallengeTemplate({
    instructions: "Now you write the program. When it starts, $c{A} holds a number from $v{0} to $v{8}. Turn on that many lights, starting from light 0, with the rest off. For example, if $c{A} is $v{3}, lights 0, 1, and 2 should be on: $c{0b00000111}.\nYou'll need a loop. One way: start with a register at $v{0}, and each time around the loop, shift it left one place and set its lowest bit, until you've done that $c{A} times. Remember to store the result at address $v{255}, and to end with $c{HALT}.",
    startCode: "; A holds the number of lights to turn on.\n\n",
    cases: [0, 1, 2, 3, 4, 5, 6, 7, 8].map((n) => ({ registers: { A: n } })),
    solve: "        SET B, 0\n        JZ A, done\nloop:   SHL B, 1\n        OR B, 1\n        SUB A, 1\n        JNZ A, loop\ndone:   STORE [255], B\n        HALT\n",
    hints: ({ error, final, start }) => {
        if (error)
            return error;
        if (final && start.registers.A === 0 && final.memory[255] !== 0)
            return "When $c{A} starts at $v{0}, no lights should be on. Check $c{A} before the loop starts, with $c{JZ}.";
        return null;
    },
    next: "129-multiply.html",
});
