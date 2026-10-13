import { createMachineTemplate } from "./shared-machine-template.js";
const lightsHint = (value, bits) => `${value} is $c{${bits}} in binary. Each 1 bit is a light that's on, and the rightmost bit is bit 0.`;
createMachineTemplate({
    initialInstructions: "A byte is 8 bits, and each bit is worth twice as much as the one to its right: from bit 7 down to bit 0, that's $v{128}, $v{64}, $v{32}, $v{16}, $v{8}, $v{4}, $v{2}, and $v{1}. That's why one byte can hold $v{256} different values, from $v{0} to $v{255}.\nSince the bits are what matter here, it's often easier to write numbers in binary directly. In this assembly language, and in C, a number starting with $c{0b} is written in binary.\nClick $runLineButton to continue.",
    steps: [
        {
            code: "SET A, 0b00001111   ; A = 0b00001111\n",
            instructions: "$c{0b00001111} is $c{8 + 4 + 2 + 1}, which is $v{15}. (Leading zeros don't change a number's value, but writing all 8 bits makes it easy to see which lights it'll turn on.)",
        },
        { code: "STORE [255], A      ; the byte at address 255 = A\n" },
        {
            code: "SET B, 0b10100101   ; B = 0b10100101\n",
            editable: true,
            instructions: "What number is this, in decimal? Change $c{B}'s value.",
            hints: "$c{0b10100101} has 1 bits worth $v{128}, $v{32}, $v{4}, and $v{1}.",
        },
        {
            code: "STORE [255], B      ; the byte at address 255 = B\n",
            editable: true,
            hints: "Turn on exactly the lights for the 1 bits in $c{0b10100101}.",
        },
        {
            code: "SET C, 0xF0         ; C = 0xF0\n",
            instructions: "Binary numbers get long, so programmers often use hexadecimal instead: base 16, written with $c{0x} in front. Its digits are $c{0} to $c{9} and then $c{A} to $c{F}, which stand for $v{10} to $v{15}. The useful thing about it is that each hex digit is exactly 4 bits, so a byte is always two hex digits: $c{0xF0} is $c{1111} (F) followed by $c{0000} (0).",
        },
        {
            code: "STORE [255], C      ; the byte at address 255 = C\n",
            editable: true,
            hints: lightsHint("$c{0xF0}", "11110000"),
        },
        { code: "SET D, 0x81         ; D = 0x81\n" },
        {
            code: "STORE [255], D      ; the byte at address 255 = D\n",
            editable: true,
            hints: lightsHint("$c{0x81}", "10000001: 8 is 1000, and 1 is 0001"),
        },
        {
            code: "SET A, 0x2A         ; A = 0x2A\n",
            editable: true,
            instructions: "What's $c{0x2A} in decimal? (The first digit is worth $v{16} each, and the second is worth $v{1}.)",
            hints: "$c{0x2A} is $c{2 * 16 + 10}, since $c{A} is $v{10}.",
        },
        { code: "HALT\n" },
    ],
    next: "126-machine-arithmetic.html",
});
