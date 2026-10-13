import { createMachineTemplate } from "./shared-machine-template.js";
createMachineTemplate({
    initialInstructions: "Most instructions do arithmetic on a register. Like $c{STORE}, they put the register being changed first: $c{ADD A, B} means $c{A = A + B}. The second operand can be a register or a number.\nClick $runLineButton to continue.",
    start: { memory: { 255: 0b00010001 } },
    steps: [
        { code: "SET A, 12        ; A = 12\n" },
        { code: "SET B, 10        ; B = 10\n" },
        { code: "COPY C, A        ; C = A\n", instructions: "$c{COPY} copies one register into another." },
        {
            code: "AND C, B         ; C = C & B\n",
            editable: true,
            instructions: "These are the same bitwise operations as in C. $v{12} is $c{0b1100}, and $v{10} is $c{0b1010}.",
            hints: "$c{0b1100 & 0b1010} keeps only the bits that are in both: $c{0b1000}.",
        },
        { code: "COPY D, A        ; D = A\n" },
        { code: "OR D, B          ; D = D | B\n", editable: true, hints: "$c{0b1100 | 0b1010} has the bits that are in either: $c{0b1110}." },
        { code: "SET B, 250       ; B = 250\n" },
        {
            code: "ADD B, 10        ; B = B + 10\n",
            editable: true,
            instructions: "A register holds one byte, so it can't hold $v{260}. Like an $t{unsigned char} in C, arithmetic wraps around: anything that doesn't fit in 8 bits is just lost.",
            hints: "$c{250 + 10} is $v{260}, which is $v{4} more than $v{256}.",
        },
        { code: "SUB B, 5         ; B = B - 5\n", editable: true, hints: "Going below $v{0} wraps around to $v{255}." },
        { code: "SHL A, 2         ; A = A << 2\n", editable: true, hints: "Shifting left 2 places multiplies by $v{4}." },
        {
            code: "LOAD C, [255]    ; C = the byte at address 255\n",
            instructions: "Here's a common task: turning one light on without changing any of the others. There's no instruction that changes one bit of memory, so a program has to read the whole byte, change the bit in a register, and write the byte back.",
        },
        {
            code: "OR C, 0b01000000 ; C = C | 0b01000000\n",
            instructions: "OR with a byte that only has bit 6 set turns on bit 6, and leaves the others as they were.",
        },
        { code: "STORE [255], C   ; the byte at address 255 = C\n", editable: true, hints: "Light 6 is on now, along with the lights that were already on." },
        {
            code: "AND C, 0b11111110 ; C = C & 0b11111110\n",
            instructions: "And AND with a byte that has every bit set except bit 0 turns off bit 0, and leaves the others alone.",
        },
        { code: "STORE [255], C   ; the byte at address 255 = C\n", editable: true, hints: "Light 0 is off now. The others haven't changed." },
        { code: "HALT\n" },
    ],
    next: "127-jumps.html",
});
