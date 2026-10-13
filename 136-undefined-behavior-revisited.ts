import { createMachineTemplate } from "./shared-machine-template.js";

const c = `unsigned char a[2];
unsigned char b = 7;

int main(void) {
  a[0] = 1;
  a[1] = 2;
  a[2] = 9;
  lights = b;
}`;

createMachineTemplate({
  initialInstructions:
    "Back in $i{40. Undefined Behavior}, undefined behavior in this tutorial just stopped the program, with a note that a real computer might do something stranger. Now you can see why.\nThe program state you've been watching all along isn't a real computer. It's what the C standard calls the abstract machine: an imaginary computer, made of objects that have types and values, like the boxes. The standard doesn't describe any real processor. It describes what each C program does on the abstract machine, and a compiler's only job is to produce instructions that give the same results on a real one.\nFor a program with undefined behavior, the standard says nothing at all about what it does, so whatever the compiler produces is fine. Usually, it just translates each line the way it always would, without checking for the problem, and then whatever those instructions happen to do on the real machine is what happens. Here's an example, compiled for the toy machine. The compiler put $n{a} at addresses $v{16} and $v{17}, and $n{b} right after it, at $v{18}.\nClick $runLineButton to continue.",
  c: { code: c, lines: { 0: 4, 1: 4, 2: 5, 3: 5, 4: 6, 5: 6, 6: 7, 7: 7, 8: 8 } },
  start: { memory: { 18: 7 } },
  view: { memory: [16, 17, 18], memoryNames: { 16: "a[0]", 17: "a[1]", 18: "b" }, registers: ["A"] },
  steps: [
    { code: "        SET A, 1\n" },
    { code: "        STORE [16], A     ; a[0] = 1\n" },
    { code: "        SET A, 2\n" },
    { code: "        STORE [17], A     ; a[1] = 2\n" },
    {
      code: "        SET A, 9\n",
      instructions:
        "To find $c{a[2]}, the compiler adds 2 to $n{a}'s address: $c{16 + 2}, which is $v{18}. That address isn't part of $n{a}. But the instruction doesn't know or care where $n{a} ends; it just writes to address $v{18}.",
    },
    {
      code: "        STORE [18], A     ; a[2] = 9\n",
      editable: true,
      instructions: "What happens?",
      hints: "Address $v{18} is where $n{b} is.",
    },
    {
      code: "        LOAD A, [18]\n",
      instructions:
        "So $n{b} changed, even though the program never assigned to it. On a real computer, this is what out-of-bounds bugs usually look like: a write past the end of one array quietly changes something else, and the program goes wrong somewhere else entirely, maybe much later. If the array had been a local variable, the bytes after it might have been on the stack, maybe including a return address, and then the function would return somewhere else entirely. That's how many security exploits work.",
    },
    { code: "        STORE [255], A    ; lights = b\n", editable: true, hints: "$n{b} is $v{9} now." },
    {
      code: "        HALT\n",
      instructions:
        "This is why the abstract machine doesn't just say \"$c{a[2]} writes the next byte\": what's next to an array, if anything, depends on the compiler, the processor, and the rest of the program. In the boxes, this program would stop at line 7, which is the most helpful thing a tutorial can do. A real program usually won't be so kind.",
    },
  ],
  next: "137-optimizers.html",
});
