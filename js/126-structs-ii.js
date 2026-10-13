import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Structs follow the rules that arrays break. A struct can be assigned, copied, passed to a function, and returned from one, just like an $t{int}. Each time, every member is copied.\nClick $runLineButton to continue.",
    steps: [
        { code: "struct pair {\n  int values[2];\n  double weight;\n};\n\n" },
        { code: "struct pair doubled(struct pair p) {\n" },
        {
            code: "  p.values[0] *= 2;\n",
            instructions: "$n{p} is a copy of the argument, so changing it doesn't change $c{main}'s $n{a}.",
        },
        { code: "  p.values[1] *= 2;\n" },
        { code: "  return p;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        { code: "  struct pair a = {{1, 2}, 0.5};\n" },
        {
            code: "  struct pair b = a;\n",
            instructions: "Initializing a struct with another struct copies every member, even the array inside it. An array can't be copied on its own, but an array inside a struct is copied along with the struct.",
        },
        {
            code: "  b.values[0] = 9;\n",
            instructions: "$n{b} is a separate copy, so this doesn't change $n{a}.",
        },
        {
            code: "  struct pair c = doubled(b);\n",
            editable: [false, true],
            instructions: ["", "What should the program state look like after $c{doubled} returns?"],
            hints: (ctx) => ctx.basicHint,
        },
        {
            code: "  a = c;\n",
            instructions: "Assigning a struct copies every member too.",
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "127-struct-pointers.html",
});
