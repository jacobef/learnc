import { createMachineTemplate } from "./shared-machine-template.js";

const c = `unsigned char twice(unsigned char x) {
  return x + x;
}

int main(void) {
  lights = twice(3);
  lights = twice(lights);
}`;

createMachineTemplate({
  initialInstructions:
    "How does a function call work? Calling a function is a jump, but returning is harder: the function has to jump back to wherever it was called from, which could be anywhere. So the processor saves the place to come back to, called the return address, in an area of memory called the stack.\nThe stack starts at the top of memory, just below the lights, and grows downwards. The register $c{SP}, the stack pointer, holds the address of the next free byte. $c{PUSH A} stores $c{A} at $c{SP} and moves $c{SP} down one byte; $c{POP A} moves it back up and loads that byte. $c{CALL label} pushes the return address (here, the number of the instruction after the call) and jumps; $c{RET} pops it and jumps back there.\nClick $runLineButton to continue.",
  c: { code: c, lines: { 0: 5, 1: 5, 2: 5, 3: 6, 4: 6, 5: 7, 6: 0, 7: 1, 8: 1, 9: 1, 10: 1 } },
  view: { memory: [250, 251, 252, 253, 254], registers: ["A", "B"], stack: true },
  start: { registers: { B: 42 } },
  steps: [
    {
      code: "        SET A, 3         ; the argument goes in A\n",
      instructions: "This compiler passes a function's argument, and its result, in $c{A}. ($c{B} starts out holding $v{42}, which belongs to $c{main}.)",
    },
    {
      code: "        CALL twice       ; twice(3)\n",
      editable: [true, false],
      instructions: [
        "Select where execution continues, and watch what happens to the stack.",
        "",
      ],
      hints: ["$c{CALL} jumps to the label $c{twice}."],
    },
    {
      code: "        STORE [255], A   ; lights = the result, in A\n",
      instructions: [
        "$c{RET} came back here, to the instruction after the $c{CALL}, using the address on the stack. And the stack is back to how it was before the call.",
        "",
      ],
    },
    { code: "        CALL twice       ; twice(lights), still in A\n" },
    { code: "        STORE [255], A\n" },
    { code: "        HALT\n" },
    {
      code: "twice:  PUSH B           ; save B, so it can be used\n",
      editable: [true, false],
      instructions: [
        "Location 254 now holds $v{2}, the return address: the number of the instruction after the $c{CALL}. ($c{SET} is instruction 0.)\n$c{twice} needs a second register to work with, but $c{B} belongs to its caller. So it pushes $c{B}'s value onto the stack, to restore it later. What happens?",
        "",
      ],
      hints: ["$c{PUSH B} stores $c{B} at address $c{SP}, which is $v{253}, and then moves $c{SP} down by one."],
    },
    { code: "        COPY B, A        ; B = x\n" },
    { code: "        ADD A, B         ; A = x + x\n" },
    {
      code: "        POP B            ; restore B\n",
      editable: [false, true],
      hints: ["", "$c{POP B} moves $c{SP} back up by one, and loads $c{B} from that address."],
    },
    {
      code: "        RET              ; return to the caller\n",
      instructions: [
        "",
        "This is how C's local variables work, too. A function's variables live on the stack, just below its return address, and returning moves $c{SP} back up past them. They aren't erased, but the memory is free to be reused, and the very next call will put its own variables in the same place. That's why the boxes showed the same addresses for each call's variables, and why using a pointer to a variable from a function that has returned is a bug.",
      ],
    },
  ],
  next: "136-undefined-behavior-revisited.html",
});
