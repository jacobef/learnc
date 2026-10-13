import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "A struct can contain a pointer to another struct of the same type. That makes it possible to chain structs together into a linked list: each one, called a node, holds a value and points to the next node, and the last node's pointer is $c{NULL}, a pointer that points to nothing. Unlike an array, a linked list can grow one node at a time, without moving anything.\n($c{NULL} is defined in $c{stdlib.h}, among other headers. It's shown as the address $v{0}.)\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <stdlib.h>\n\n" },
        { code: "struct node {\n  int value;\n  struct node *next;\n};\n\n" },
        { code: "struct node *push(struct node *head, int value) {\n" },
        { code: "  struct node *n = malloc(sizeof(struct node));\n" },
        { code: "  n->value = value;\n" },
        {
            code: "  n->next = head;\n",
            instructions: ["The new node points to the old first node, so it becomes the new first node.", ""],
        },
        { code: "  return n;\n" },
        { code: "}\n\n" },
        { code: "int main(void) {\n" },
        {
            code: "  struct node *list = NULL;\n",
            instructions: "An empty list is just a $c{NULL} pointer.",
        },
        { code: "  list = push(list, 3);\n" },
        { code: "  list = push(list, 2);\n" },
        { code: "  list = push(list, 1);\n" },
        { code: "  int sum = 0;\n" },
        {
            code: "  for (struct node *p = list; p != NULL; p = p->next) {\n",
            instructions: [
                "To visit every node, follow the pointers from the first node until reaching $c{NULL}.",
                "",
                "",
                "",
            ],
        },
        { code: "    sum += p->value;\n", editable: [false, true, false], hints: basic },
        { code: "  }\n" },
        { code: "  return 0;\n" },
        { code: "}\n" },
    ],
    next: "145-list-length.html",
});
