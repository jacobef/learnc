import { createMachineTemplate } from "./shared-machine-template.js";
createMachineTemplate({
    initialInstructions: "The processor keeps track of which instruction comes next in a hidden register called the program counter. Normally it just moves on to the next instruction, but a jump instruction changes it. $c{JMP label} always jumps to the label, like $c{goto}. $c{JZ A, label} jumps only if $c{A} is zero, and $c{JNZ A, label} jumps only if it isn't.\nThat's all a processor needs to make an $c{if} statement or a loop. In fact, this program is exactly the kind of loop you saw made with $c{goto}, and it's what a $c{while} loop really turns into.\nClick $runLineButton to continue.",
    steps: [
        { code: "        SET A, 4        ; A = 4\n" },
        { code: "        SET B, 1        ; B = 1\n" },
        {
            code: "again:  STORE [255], B  ; the byte at address 255 = B\n",
            editable: [false, true, false, false],
            hints: ["", "$c{B} is $v{2} now, which is $c{0b00000010}."],
        },
        { code: "        SHL B, 1        ; B = B << 1\n" },
        { code: "        SUB A, 1        ; A = A - 1\n", editable: [false, false, true, false], hints: ["", "", "$c{A} counts down by one each time around the loop."] },
        {
            code: "        JNZ A, again    ; if (A != 0) goto again;\n",
            editable: [true, false, false, true],
            instructions: [
                "Is $c{A} zero? If it isn't, execution jumps back to the line labeled $c{again}. Select where execution continues.",
                "",
                "",
                "And now?",
            ],
            hints: [
                "$c{A} is $v{3}, which isn't zero, so this jumps.",
                "",
                "",
                "$c{A} is $v{0} now, so this doesn't jump: execution goes on to the next line.",
            ],
        },
        { code: "        SET B, 0xFF     ; B = 0xFF\n" },
        { code: "        STORE [255], B  ; the byte at address 255 = B\n" },
        {
            code: "        HALT\n",
            instructions: "Each time around the loop, the lit light moved one place to the left, which is how a simple light chaser works. The last two instructions turn all the lights on once the loop ends.",
        },
    ],
    next: "128-light-up.html",
});
