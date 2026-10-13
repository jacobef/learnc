import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A union is written like a struct, but its members all share the same memory: it holds only one of them at a time. Its size is the size of its largest member. Unions are for saving memory when a value can be one of several types, but only one at a time.\nClick $runLineButton to continue.",
    steps: [
        { code: "union number {\n  int i;\n  double d;\n};\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  union number n;\n",
        },
        {
            code: "  n.i = 7;\n",
            instructions: "Now $n{n} holds an $t{int}. The program state shows which member a union currently holds.",
        },
        {
            code: "  n.d = 2.5;\n",
            instructions: "Storing a $t{double} replaces the $t{int}: they're in the same bytes.",
        },
        {
            code: "  int size = sizeof n;\n",
            instructions: "The union only needs enough room for its largest member.",
        },
        {
            code: "  union number m = n;\n",
            instructions: "Like a struct, a union can be copied. Reading a member other than the one last stored is allowed in C, but it reinterprets the bytes, which gives a meaningless value unless you know exactly how the types are stored.",
        },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "134-free.html",
});
