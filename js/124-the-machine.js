import { createMachineTemplate } from "./shared-machine-template.js";
createMachineTemplate({
    initialInstructions: "Time for a detour, to see what's underneath C.\nA computer's processor doesn't understand C. It runs much simpler instructions, each of which does one tiny thing, like copying a number, or adding two numbers. Translating your program into those instructions is what the compiler does. Instructions written out as text, like the ones below, are called assembly language.\nEvery kind of processor has its own instructions, and real ones have hundreds. So this detour uses a made-up computer that has just a few, so you can see all of it at once. It has 256 bytes of memory, with addresses from $v{0} to $v{255}, just like the addresses of C's variables, but much smaller. It also has four registers, named $c{A}, $c{B}, $c{C}, and $c{D}: tiny storage spaces inside the processor itself, each holding one byte. And it has 8 lights, wired to the byte at address $v{255}: each bit of that byte turns one light on or off.\nClick $runLineButton to continue.",
    steps: [
        {
            code: "SET A, 1         ; A = 1\n",
            instructions: "$c{SET} puts a number in a register. Everything after a $c{;} is a comment. The comments here say what each instruction does, written like C.\nNotice that the register being changed comes first, like the left side of $c{=} in C.",
        },
        {
            code: "STORE [255], A   ; the byte at address 255 = A\n",
            instructions: "$c{STORE} copies a register into memory. The square brackets mean \"the byte at this address\", a lot like $c{*} in C.\nAddress $v{255} is wired to the lights, and $v{1} in binary is $c{00000001}, so this turns on light 0 and turns off the rest. (This is called memory-mapped I/O: the computer controls a device just by writing to a particular memory address. Real computers work this way too.)",
        },
        { code: "SET B, 6         ; B = 6\n" },
        {
            code: "STORE [255], B   ; the byte at address 255 = B\n",
            editable: true,
            instructions: "Which lights are on now? Click the lights to turn them on or off, then press $checkButton. (Each light's number, below it, is how much that bit is worth: $v{6} is $c{4 + 2}.)",
            hints: "$v{6} is $c{4 + 2}, so it's $c{00000110} in binary: bits 1 and 2 are 1, and the rest are 0. Storing a byte replaces all 8 bits.",
        },
        { code: "SET C, 255       ; C = 255\n" },
        {
            code: "STORE [255], C   ; the byte at address 255 = C\n",
            editable: true,
            hints: "$v{255} is $c{128 + 64 + 32 + 16 + 8 + 4 + 2 + 1}: every bit is 1.",
        },
        {
            code: "LOAD D, [255]    ; D = the byte at address 255\n",
            editable: true,
            instructions: "$c{LOAD} is the opposite of $c{STORE}: it copies a byte from memory into a register. Change the registers to show what this does.",
            hints: "This copies the byte at address $v{255} into $c{D}.",
        },
        {
            code: "HALT             ; stop\n",
            instructions: "$c{HALT} stops the processor. The lights stay as they were. A real processor doesn't know where a program ends, so without $c{HALT} it would carry on, treating whatever bytes come next in memory as more instructions.\nThat's really all a computer is: memory, registers, and instructions that move numbers between them and do arithmetic on them. Everything else, including every C program, is built from instructions like these.",
        },
    ],
    view: { registers: ["A", "B", "C", "D"] },
    next: "125-binary-and-hex.html",
});
