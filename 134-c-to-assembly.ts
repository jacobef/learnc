import { createMachineTemplate } from "./shared-machine-template.js";

const c = `unsigned char total = 0;
unsigned char n = 4;

while (n != 0) {
  total += n;
  n -= 1;
}
lights = total;`;

createMachineTemplate({
  initialInstructions:
    "Here's a small C program, along with the assembly a simple compiler might translate it into, for this machine. As each instruction runs, the C line it came from is highlighted.\nThe compiler decided where each variable lives in memory: $n{total} at address $v{16}, and $n{n} at address $v{17}. That's what the addresses in the program state have been all along.\nClick $runLineButton to continue.",
  c: {
    code: c,
    lines: { 0: 0, 1: 0, 2: 1, 3: 1, 4: 3, 5: 3, 6: 4, 7: 4, 8: 4, 9: 5, 10: 5, 11: 6, 12: 7, 13: 7, 14: 7 },
  },
  view: { memory: [16, 17], memoryNames: { 16: "total", 17: "n" }, registers: ["A", "B"] },
  steps: [
    { code: "        SET A, 0\n" },
    { code: "        STORE [16], A     ; total = 0\n" },
    { code: "        SET A, 4\n" },
    { code: "        STORE [17], A     ; n = 4\n" },
    {
      code: "top:    LOAD A, [17]\n",
      instructions: [
        "A $c{while} loop starts by checking its condition: load $n{n}, and jump past the loop if it's zero.",
        "",
      ],
    },
    { code: "        JZ A, done        ; while (n != 0)\n", editable: [false, false, false, false, true] },
    { code: "        LOAD B, [16]\n" },
    {
      code: "        ADD B, A\n",
      instructions: [
        "One line of C, $c{total += n;}, became three instructions: load $n{total}, add, and store it back. ($c{A} still holds $n{n} from before.)",
        "",
      ],
    },
    { code: "        STORE [16], B     ; total += n\n", editable: [false, true, false, false] },
    { code: "        SUB A, 1\n" },
    { code: "        STORE [17], A     ; n -= 1\n" },
    {
      code: "        JMP top           ; back to the condition\n",
      instructions: ["The end of the loop's block is a jump back to the condition.", ""],
    },
    { code: "done:   LOAD A, [16]\n" },
    { code: "        STORE [255], A    ; lights = total\n" },
    {
      code: "        HALT\n",
      instructions:
        "A real compiler would do better than this. It would notice that $n{total} and $n{n} could stay in registers the whole time, and skip most of the loads and stores. A good one might even work out the whole answer, $v{10}, while compiling, and replace the program with $c{SET A, 10} and one $c{STORE}. That's allowed, because the program would still do exactly what the C code says. Keep that in mind; it'll matter soon.",
    },
  ],
  next: "135-the-stack.html",
});
