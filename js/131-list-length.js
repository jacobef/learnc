import { createCodeOutputChallengeTemplate } from "./shared-code-output-template.js";
const list = (values) => values.reduce((rest, value) => `push(${rest}, ${value})`, "NULL");
createCodeOutputChallengeTemplate({
    inputs: [{ name: "count", type: "int" }],
    outputs: [{ name: "n", type: "int" }],
    testInputs: [0, 1, 2, 3, 5, 8].map((count) => [String(count)]),
    startInput: ["3"],
    prelude: "#include <stdlib.h>\n\nstruct node {\n  int value;\n  struct node *next;\n};\n\nstruct node *push(struct node *head, int value) {\n  struct node *n = malloc(sizeof(struct node));\n  n->value = value;\n  n->next = head;\n  return n;\n}\n",
    main: ([count]) => `int main(void) {\n  struct node *list = ${list(Array.from({ length: Number(count) }, (_, i) => (i * 7) % 10))};\n  int n = length(list);\n  return 0;\n}`,
    solve: "int length(struct node *head) {\n  int n = 0;\n  while (head != NULL) {\n    n++;\n    head = head->next;\n  }\n  return n;\n}",
    textareaMinLines: 8,
    instructions: "Write a function named $c{length} that takes a pointer to the first node of a linked list (or $c{NULL}, for an empty list), and returns the number of nodes in it.",
    hints: (ctx) => ctx.currentResult.kind === "step-limit"
        ? "Your loop doesn't end. Each time around, move to the next node, until reaching $c{NULL}."
        : null,
    next: "132-typedef.html",
});
