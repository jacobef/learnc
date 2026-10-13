import { createProgramTemplate } from "./shared-program-template.js";
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "Every variable so far has been declared inside a function, so it's created when its declaration runs and removed at the end of its block. That's called automatic storage. A variable declared outside of any function is a global variable: it exists for the whole time the program runs, it's initialized before $c{main} starts, and every function after its declaration can use it.\nClick $runLineButton to continue.",
    steps: [
        { code: "int total = 100;\n\n" },
        { code: "int count_call(void) {\n" },
        {
            code: "  static int calls = 0;\n",
            instructions: "$c{static} on a variable inside a function gives it the same lifetime as a global: it's created and initialized once, before the program starts, and keeps its value between calls. But its name can still only be used inside this function. So when this line runs, nothing happens: the initialization already happened.",
        },
        { code: "  calls++;\n", editable: [false, true] },
        {
            code: "  total -= 1;\n",
        },
        { code: "  return calls;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  count_call();\n",
            instructions: [
                "",
                "$c{count_call} changed the global $n{total}, and its $c{static} variable $n{calls} keeps its value now that the call has returned.",
            ],
        },
        {
            code: "  int n = count_call();\n",
        },
        {
            code: "  return 0;\n",
            instructions: "Globals make it easy to share data, but hard to keep track of what changes it, since any function might. It's usually better to pass values as parameters.",
        },
        { code: "}\n" },
    ],
    next: "139-const.html",
});
