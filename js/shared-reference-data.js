// Quick-reference content. Each entry names the lesson that introduces it, and
// only appears once the learner has reached that lesson.
const REFERENCE_SECTIONS = [
    {
        title: "Operators",
        notes: [
            { lesson: "11-integer-arithmetic.html", code: "", text: "Higher rows are evaluated first. Within a row, evaluation goes left to right: $c{a - b + c} means $c{(a - b) + c}. Spacing never matters." },
        ],
        groups: [
            {
                label: "Parentheses",
                entries: [
                    { lesson: "11-integer-arithmetic.html", code: "(x)", text: "Evaluated before anything outside them." },
                ],
            },
            {
                label: "One side",
                entries: [
                    { lesson: "11-integer-arithmetic.html", code: "-x", text: "Negation." },
                    { lesson: "7-pointers.html", code: "&x", text: "$n{x}'s address." },
                    { lesson: "8-dereferencing-i.html", code: "*p", text: "The variable whose address is $n{p}'s value." },
                ],
            },
            {
                label: "Multiplication",
                entries: [
                    { lesson: "11-integer-arithmetic.html", code: "x * y", text: "Multiplication." },
                    { lesson: "11-integer-arithmetic.html", code: "x / y", text: "Division. If both sides are $t{int}s, the decimal part is dropped: $c{7 / 2} is $v{3}, and $c{-7 / 2} is $v{-3}." },
                ],
            },
            {
                label: "Addition",
                entries: [
                    { lesson: "11-integer-arithmetic.html", code: "x + y", text: "Addition." },
                    { lesson: "11-integer-arithmetic.html", code: "x - y", text: "Subtraction." },
                ],
            },
            {
                label: "Comparison",
                entries: [
                    { lesson: "17-comparisons.html", code: "x < y", text: "$v{1} if $c{x} is less than $c{y}, $v{0} otherwise. Similarly $c{<=}, $c{>}, and $c{>=}." },
                ],
            },
            {
                label: "Equality",
                entries: [
                    { lesson: "11-integer-arithmetic.html", code: "x == y", text: "$v{1} if $c{x} and $c{y} are equal, $v{0} otherwise." },
                    { lesson: "17-comparisons.html", code: "x != y", text: "$v{1} if $c{x} and $c{y} are not equal, $v{0} otherwise." },
                ],
            },
        ],
    },
    {
        title: "Variables and types",
        groups: [
            {
                entries: [
                    { lesson: "2-declaration.html", code: "int x;", text: "Declaration: creates a variable with no value." },
                    { lesson: "3-initialization.html", code: "int x = 5;", text: "Initialization: creates a variable and gives it a value." },
                    { lesson: "1-assignment-i.html", code: "x = 5;", text: "Assignment: sets $n{x}'s value." },
                    { lesson: "7-pointers.html", code: "int* p = &x;", text: "A pointer: its value is an address." },
                    { lesson: "14-expressions-doubles.html", code: "double d = 2.5;", text: "A $t{double} holds numbers with a decimal point. Any number written with a decimal point, like $c{5.0}, is a $t{double}." },
                    { lesson: "14-expressions-doubles.html", code: "2 / 4.0", text: "An operation on an $t{int} and a $t{double} is done as if both were $t{double}s." },
                    { lesson: "15-doubles.html", code: "int i = 4.9;", text: "Storing a $t{double} in an $t{int} drops the decimal part. Storing an $t{int} in a $t{double} keeps its value." },
                ],
            },
        ],
    },
    {
        title: "Statements",
        groups: [
            {
                entries: [
                    { lesson: "10-whitespace-comments.html", code: "// comment", text: "Ignored until the end of the line." },
                    { lesson: "10-whitespace-comments.html", code: "/* comment */", text: "Ignored, and can span multiple lines." },
                    { lesson: "16-blocks.html", code: "{\n  ...\n}", text: "A block. Variables created inside it are removed at its $c{\\}}." },
                    { lesson: "18-if.html", code: "if (c) {\n  ...\n}", text: "Runs the block if $c{c} is non-zero, and skips it otherwise." },
                    { lesson: "19-else.html", code: "} else {\n  ...\n}", text: "Runs if the $c{if} block above didn't." },
                    { lesson: "23-else-if.html", code: "} else if (c) {\n  ...\n}", text: "Checked only if every $c{if} and $c{else if} above was skipped." },
                    { lesson: "28-while.html", code: "while (c) {\n  ...\n}", text: "Runs the block again and again, as long as $c{c} is non-zero." },
                ],
            },
        ],
    },
];
export { REFERENCE_SECTIONS };
