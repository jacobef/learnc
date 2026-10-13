import { createMachineTemplate } from "./shared-machine-template.js";
createMachineTemplate({
    initialInstructions: "In square brackets, an instruction can use a register's value as the address, instead of a number written in the program: $c{LOAD D, [B]} loads the byte at the address in $c{B}. A register holding an address is exactly what a C pointer is.\nThis program adds up 4 bytes stored one after another, at addresses $v{16} to $v{19}: an array, like $c{unsigned char a[4]}. (In memory, an array is nothing more than that: values in a row.)\nClick $runLineButton to continue.",
    start: { memory: { 16: 3, 17: 5, 18: 7, 19: 9 } },
    view: { memory: [16, 17, 18, 19], memoryNames: { 16: "a[0]", 17: "a[1]", 18: "a[2]", 19: "a[3]" } },
    steps: [
        { code: "        SET B, 16       ; B = the address of a[0], like p = &a[0]\n" },
        { code: "        SET A, 0        ; A = 0, the sum\n" },
        { code: "        SET C, 4        ; C = 4, the number of bytes left\n" },
        {
            code: "loop:   LOAD D, [B]     ; D = *p\n",
            editable: [false, true, false, false],
            instructions: [
                "$c{B} holds $v{16}, so this loads the byte at address $v{16}.",
                "What does this load now?",
            ],
            hints: ["", "$c{B} is $v{17} now, so this loads the byte at address $v{17}."],
        },
        { code: "        ADD A, D        ; sum += *p\n" },
        {
            code: "        ADD B, 1        ; p++\n",
            instructions: [
                "Adding 1 to the address moves on to the next byte. That's what $c{p++} does in C, too. (With an $t{int *}, C adds 4 bytes instead, since each $t{int} is 4 bytes, but it's the same idea.)",
                "",
            ],
        },
        { code: "        SUB C, 1\n" },
        { code: "        JNZ C, loop\n" },
        { code: "        STORE [255], A  ; show the sum\n", editable: true, hints: "The sum is $c{3 + 5 + 7 + 9}." },
        { code: "        HALT\n" },
    ],
    next: "131-back-to-c.html",
});
