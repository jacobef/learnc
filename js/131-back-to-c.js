import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "C was created in the early 1970s at Bell Labs, by Dennis Ritchie, to write the Unix operating system, which until then had been written in assembly, like most operating systems of the time. C was designed so that it would be almost as close to the machine as assembly, but much easier to write, and not tied to one kind of processor. Each of its operations corresponds to a few machine instructions: a variable is some bytes at an address, a pointer is an address, $c{+} is an $c{ADD}, and $c{while} is a conditional jump. That's why C is still used for operating systems, for embedded devices like microwaves and cars, and for anything else that needs to control hardware directly.\nOn a real device, C code would turn the lights on by writing to address $v{255} through a pointer, something like $c{*(unsigned char *)255 = 6;}. The interpreter in this tutorial doesn't let a program make up addresses like that, so here, the lights are wired to a global variable named $n{lights} instead.\nFor now, you can think of each statement as doing exactly what it says, one step at a time, on a machine like the one in the last few lessons.\nClick $runLineButton to continue.",
    steps: [
        { code: "unsigned char lights = 0;\n\n" },
        { code: "int main(void) {\n" },
        { code: "  lights = 6;\n" },
        {
            code: "  lights |= 0b01000000;\n",
            editable: true,
            instructions: "This is the read-change-write from two lessons ago, in one statement: $c{LOAD}, $c{OR}, and $c{STORE}. Click the lights, or change $n{lights}'s value.",
            hints: basic,
        },
        { code: "  lights &= 0b11111101;\n", editable: true, hints: basic },
        {
            code: "  lights ^= 0xFF;\n",
            editable: true,
            instructions: "XOR with all 1 bits flips every bit.",
            hints: basic,
        },
        {
            code: "  for (unsigned char bit = 1; bit != 0; bit <<= 1) {\n",
            instructions: [
                "A light chaser, like the one in assembly. When $n{bit} is shifted past bit 7, it wraps around to $v{0}, since it's an $t{unsigned char}, and the loop ends.",
                "",
            ],
        },
        { code: "    lights = bit;\n", editable: [false, false, true], hints: basic },
        { code: "  }\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "132-bits-in-c.html",
    workspace: { lights: "lights" },
});
