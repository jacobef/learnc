import { createCompileQuizTemplate } from "./shared-compile-quiz-template.js";
createCompileQuizTemplate({
    initialInstructions: "Arrays follow a surprising number of rules that other variables don't. For each program, decide what happens.",
    allowUndefinedAnswer: true,
    steps: [
        {
            code: "int a[3] = {1, 2, 3};\nint b[3];\nb = a;",
            explanation: "An array can't be assigned to. There's no way to replace a whole array at once: to copy one array into another, copy the elements one at a time.",
        },
        {
            code: "int a[3] = {1, 2, 3};\nint b[3] = a;",
            explanation: "An array can't be initialized from another array, either. In fact, only two things can initialize an array: a brace-enclosed list, or (for a $t{char} array) a string literal.",
        },
        {
            code: "int a[3];\na = {1, 2, 3};",
            explanation: "Even ignoring the previous rules, this would fail, because $c{\\{1, 2, 3\\}} isn't an expression. A brace-enclosed list is called an initializer list, and it's special syntax that can only appear in an initializer, right after the $c{=} in a declaration. As far as the compiler is concerned, there's nothing on the right side of the $c{=} here, as its message says.",
        },
        {
            code: "int x = {5};",
            explanation: "Braces are allowed around any variable's initializer, so this means the same thing as $c{int x = 5;}.",
        },
        {
            code: "int *p = {1, 2, 3};",
            explanation: "$n{p} is a pointer, so it gets initialized with a single address. An initializer list doesn't create an array that $n{p} could point to; it's just a list of values for the variable being declared.",
        },
        {
            code: "char *s = \"abc\";",
            explanation: "Compare this with the previous program. A string literal is an expression: it's a nameless array that exists for the whole program, so it can decay into a pointer to its first element. An initializer list doesn't make an array anywhere.",
        },
        {
            code: "int a[2] = {1, 2, 3};",
            explanation: "There are more values than elements.",
        },
        {
            code: "char s[2] = \"abc\";",
            explanation: "The string's characters don't fit in the array.",
        },
        {
            code: "char s[3] = \"abc\";",
            explanation: "This is allowed! When a $t{char} array has exactly as many elements as the text has characters, the $c{'\\0'} is just left out. That means $n{s} holds a, b, and c, but it isn't a string.",
            hint: "$c{\"abc\"} has 3 characters, plus the null character at the end.",
        },
        {
            code: "char s[3] = \"abc\";\nint length = 0;\nwhile (s[length] != '\\0') {\n  length++;\n}",
            explanation: "Since $n{s} has no null character, this loop never finds one inside the array, and keeps going past its end. This is why leaving out the $c{'\\0'} is dangerous.",
        },
        {
            code: "int a[3];\na++;",
            explanation: "$c{a++} would mean $c{a = a + 1}, which assigns to an array.",
        },
        {
            code: "int a[3];\nint *p = a;\np++;",
            explanation: "A pointer to an element, on the other hand, can be changed freely. This is often how code moves through an array.",
        },
        {
            code: "char s[] = \"hi\";\ns = \"yo\";",
            explanation: "Assigning a string literal doesn't work either. $c{char s[] = \"hi\";} is allowed because it's an initialization, and initializing isn't the same as assigning.",
        },
        {
            code: "int a[];",
            explanation: "The compiler needs to know how big the array is. (Without a size, it can only work it out from an initializer.)",
        },
        {
            code: "int a[3] = {1, 2, 3};\nint *p = a;",
            question: false,
            instructions: "Why are arrays like this?",
            explanation: "Mostly for historical reasons. C grew out of an older language called B, which had no types. In B, declaring an array created an ordinary variable that held a pointer to the array's elements, which were stored somewhere else. Plenty of B code relied on using an array name as a pointer.\nWhen C was designed, keeping that hidden pointer variable caused problems, so C got rid of it: an array's elements are stored right in the array, and the array's name isn't stored anywhere. Instead, using the name converts it into a pointer to the first element. That's decay, and it kept the old code working. But it also means an array almost never gets treated as a single value that could be assigned, copied, passed to a function, or returned. Those operations were simply never added.\n(It's sometimes said that this was done to make passing large arrays efficient. Passing a pointer is indeed cheap, but the design mostly came from keeping compatibility with B.)",
        },
    ],
    next: "102-copy-array.html",
});
