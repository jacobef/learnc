import { createProgramTemplate } from "./shared-program-template.js";
const basic = (ctx) => ctx.basicHint;
createProgramTemplate({
    implicitMain: false,
    initialInstructions: "The header $c{string.h} declares functions for working with strings, so you don't have to write loops like the ones in the strings lessons every time.\nClick $runLineButton to continue.",
    steps: [
        { code: "#include <string.h>\n\n" },
        { code: "int main(void) {\n" },
        { code: "  char name[16] = \"Ada\";\n" },
        {
            code: "  int length = strlen(name);\n",
            editable: true,
            instructions: "$c{strlen} returns a string's length: the number of characters before the $c{'\\0'}.",
            hints: basic,
        },
        { code: "  char copy[16];\n" },
        {
            code: "  strcpy(copy, name);\n",
            instructions: "$c{strcpy(destination, source)} copies a string, including its $c{'\\0'}, into the array the destination points into. This is how to get around the rule that arrays can't be assigned.",
        },
        {
            code: "  strcat(copy, \"!\");\n",
            instructions: "$c{strcat} appends a string to the end of another one.",
        },
        {
            code: "  int same = strcmp(name, \"Ada\") == 0;\n",
            instructions: "$c{strcmp} compares two strings character by character. It returns $v{0} if they're equal, a negative number if the first comes first in alphabetical order (going by character codes), and a positive number otherwise. Comparing with $c{==} would compare the pointers, not the characters.",
        },
        {
            code: "  int before = strcmp(name, copy) < 0;\n",
            editable: true,
            instructions: "Does $c{name} come before $c{copy}? A string that ends first comes first, like in a dictionary.",
            hints: (ctx) => ctx.basicHintTopicIs("value", "before")
                ? "$c{\"Ada\"} is the same as the start of $c{\"Ada!\"}, but ends first."
                : ctx.basicHint,
        },
        {
            code: "  return 0;\n",
            instructions: "None of these functions know how big the destination array is. If the result doesn't fit, they write past its end, which is undefined behavior. Making sure there's room is up to you.",
        },
        { code: "}\n" },
    ],
    next: "138-duplicate.html",
    workspace: { hideStringLiterals: true },
});
