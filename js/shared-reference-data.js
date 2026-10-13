// Quick-reference content. Each entry names the lesson that introduces it, and
// only appears once the learner has reached that lesson.
const REFERENCE_SECTIONS = [
    {
        title: "Operators",
        notes: [
            {
                lesson: "14-integer-arithmetic.html",
                code: "",
                text: "Higher rows are evaluated first. Within a row, evaluation goes left to right: $c{a - b + c} means $c{(a - b) + c}. Spacing never matters.",
            },
        ],
        groups: [
            {
                label: "Parentheses",
                entries: [
                    { lesson: "14-integer-arithmetic.html", code: "(x)", text: "Evaluated before anything outside them." },
                ],
            },
            {
                label: "After the operand",
                entries: [
                    { lesson: "78-indexing.html", code: "a[i]", text: "Element $c{i} of the array $c{a}. Indices start at 0." },
                    { lesson: "85-decay.html", code: "", text: "$c{x[i]} means $c{*(x + i)}, so brackets work on any pointer, too." },
                    { lesson: "140-structs-i.html", code: "s.m", text: "Member $c{m} of the struct $c{s}." },
                    { lesson: "142-struct-pointers.html", code: "p->m", text: "Member $c{m} of the struct $c{p} points to: $c{(*p).m}." },
                    { lesson: "54-functions-i.html", code: "f(x)", text: "Calls the function $c{f}, with $c{x} as its argument." },
                ],
            },
            {
                label: "One side",
                entries: [
                    { lesson: "14-integer-arithmetic.html", code: "-x", text: "Negation." },
                    { lesson: "9-pointers.html", code: "&x", text: "$n{x}'s address." },
                    { lesson: "10-dereferencing-i.html", code: "*p", text: "The variable whose address is $n{p}'s value." },
                    { lesson: "114-casts.html", code: "(T)x", text: "A cast: converts $c{x} to the type $c{T}, like $c{(double)n}." },
                    { lesson: "122-bitwise.html", code: "~x", text: "Flips every bit of $c{x}." },
                    { lesson: "75-sizes.html", code: "sizeof x", text: "The number of bytes $c{x}'s type takes up, without evaluating $c{x}. A type needs parentheses: $c{sizeof(int)}." },
                    { lesson: "38-logic.html", code: "!x", text: "\"Not\": $v{1} if $c{x} is zero, $v{0} otherwise." },
                ],
            },
            {
                label: "Multiplication",
                entries: [
                    { lesson: "14-integer-arithmetic.html", code: "x * y", text: "Multiplication." },
                    { lesson: "14-integer-arithmetic.html", code: "x / y", text: "Division. If both sides are $t{int}s, the decimal part is dropped: $c{7 / 2} is $v{3}, and $c{-7 / 2} is $v{-3}." },
                    { lesson: "36-remainder.html", code: "x % y", text: "Remainder, for $t{int}s only: $c{17 % 5} is $v{2}. If $c{x} is negative, the result is negative or zero: $c{-7 % 3} is $v{-1}." },
                ],
            },
            {
                label: "Addition",
                entries: [
                    { lesson: "14-integer-arithmetic.html", code: "x + y", text: "Addition." },
                    { lesson: "14-integer-arithmetic.html", code: "x - y", text: "Subtraction." },
                    { lesson: "84-pointer-arithmetic.html", code: "p + n", text: "If $c{p} points to an array element, $c{p + n} points $c{n} elements later ($c{p - n} goes back). The result must stay inside the array, or point just past its end." },
                ],
            },
            {
                label: "Shifts",
                entries: [
                    { lesson: "122-bitwise.html", code: "x << n", text: "Shifts $c{x}'s bits left $c{n} places: multiplies by $c{2^n}. $c{x >> n} shifts right." },
                ],
            },
            {
                label: "Comparison",
                entries: [
                    { lesson: "20-comparisons.html", code: "x < y", text: "$v{1} if $c{x} is less than $c{y}, $v{0} otherwise. Similarly $c{<=}, $c{>}, and $c{>=}." },
                ],
            },
            {
                label: "Equality",
                entries: [
                    { lesson: "14-integer-arithmetic.html", code: "x == y", text: "$v{1} if $c{x} and $c{y} are equal, $v{0} otherwise." },
                    { lesson: "20-comparisons.html", code: "x != y", text: "$v{1} if $c{x} and $c{y} are not equal, $v{0} otherwise." },
                ],
            },
            {
                label: "Bitwise and, then exclusive or, then or",
                entries: [
                    { lesson: "122-bitwise.html", code: "x & y", text: "Each bit is 1 if it's 1 in both. $c{x ^ y}: in exactly one. $c{x | y}: in either. Lower precedence than $c{==}!" },
                ],
            },
            {
                label: "And",
                entries: [
                    { lesson: "38-logic.html", code: "x && y", text: "$v{1} if both $c{x} and $c{y} are non-zero, $v{0} otherwise." },
                    { lesson: "43-short-circuiting.html", code: "", text: "If $c{x} is zero, $c{y} is never evaluated." },
                ],
            },
            {
                label: "Or",
                entries: [
                    { lesson: "38-logic.html", code: "x || y", text: "$v{1} if at least one of $c{x} and $c{y} is non-zero, $v{0} otherwise." },
                    { lesson: "43-short-circuiting.html", code: "", text: "If $c{x} is non-zero, $c{y} is never evaluated." },
                ],
            },
            {
                label: "Conditional",
                entries: [
                    { lesson: "121-conditional-operator.html", code: "c ? x : y", text: "$c{x} if $c{c} is non-zero, $c{y} otherwise. Only the chosen side is evaluated." },
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
                    { lesson: "9-pointers.html", code: "int* p = &x;", text: "A pointer: its value is an address." },
                    { lesson: "17-expressions-doubles.html", code: "double d = 2.5;", text: "A $t{double} holds numbers with a decimal point. Any number written with a decimal point, like $c{5.0}, is a $t{double}." },
                    { lesson: "17-expressions-doubles.html", code: "2 / 4.0", text: "An operation on an $t{int} and a $t{double} is done as if both were $t{double}s." },
                    { lesson: "18-doubles.html", code: "int i = 4.9;", text: "Storing a $t{double} in an $t{int} drops the decimal part. Storing an $t{int} in a $t{double} keeps its value." },
                    { lesson: "111-integer-types.html", code: "10L", text: "A $t{long}. $c{10LL} is a $t{long long}, $c{10u} an $t{unsigned int}, and $c{1.5f} a $t{float}." },
                    { lesson: "113-conversions.html", code: "'a' + 1", text: "Values smaller than an $t{int} are promoted to $t{int} first. Mixed types are converted to the larger one; signed to unsigned if they're the same size." },
                    { lesson: "115-bool.html", code: "bool b = 5;", text: "From $c{stdbool.h}: holds only $v{0} or $v{1}, also called $c{false} and $c{true}. Any non-zero value converts to $v{1}." },
                    { lesson: "117-ieee.html", code: "isnan(x)", text: "From $c{math.h}: whether $c{x} is NaN, which isn't equal to anything, even itself. $c{INFINITY} and $c{NAN} are also there." },
                    { lesson: "138-scope-and-storage.html", code: "static int n = 0;", text: "Inside a function: initialized once, before the program starts, and keeps its value between calls. Outside any function, a variable is global." },
                    { lesson: "139-const.html", code: "const int *p", text: "$c{*p} can't be changed through $n{p}. With $c{int *const p}, $n{p} itself can't be changed." },
                    { lesson: "140-structs-i.html", code: "struct point { int x; int y; };", text: "Defines a struct type. Structs can be initialized with lists, like $c{\\{.y = 2\\}}, and can be assigned, passed, and returned (all copies)." },
                    { lesson: "146-typedef.html", code: "typedef int score;", text: "Makes $t{score} another name for $t{int}." },
                    { lesson: "147-enums.html", code: "enum dir { N, E, S };", text: "Named integer constants, numbered from $v{0} unless given values." },
                    { lesson: "148-unions.html", code: "union u { int i; double d; };", text: "Like a struct, but its members share the same memory, so it holds one at a time." },
                    { lesson: "91-declarations.html", code: "int a[3];", text: "A declaration says how its name is used: here, $c{a[i]} is an $t{int}, so $n{a} is an array of $t{int}s. Read the declarator from the name outwards: $n{a} is an array of 3, of $t{int}s." },
                    { lesson: "92-declarations-ii.html", code: "int *q[3];", text: "A declaration mirrors how its variable is used: here, $c{*q[i]} is an $t{int}, so $n{q} is an array of 3 pointers to $t{int}s. $c{int (*r)[3];} says $c{(*r)[i]} is an $t{int}, so $n{r} points to an array of 3 $t{int}s." },
                    { lesson: "92-declarations-ii.html", code: "int (*r)[3];", text: "Declarators are grouped like expressions: $c{[]} comes before $c{*}, so $n{q} above is an array of pointers. Parentheses change that: $n{r} is a pointer to an array of 3 $t{int}s." },
                    { lesson: "91-declarations.html", code: "int *[3]", text: "To write a type on its own, remove the name from a declaration." },
                    { lesson: "91-declarations.html", code: "int* m, n;", text: "Each variable has its own declarator: $n{m} is an $t{int*}, but $n{n} is an $t{int}." },
                    { lesson: "65-characters.html", code: "char c = 'A';", text: "A $t{char} is a small integer holding a character's code. $c{'A'} is just another way to write $v{65}." },
                    { lesson: "97-strings.html", code: "char s[] = \"hi\";", text: "A string is an array of $t{char}s ending in $c{'\\0'}. In an initializer, $c{\"hi\"} means $c{\{'h', 'i', '\\0'\}}." },
                    { lesson: "100-string-literals.html", code: "char *p = \"hi\";", text: "Elsewhere, a string literal is a nameless $t{char} array that exists for the whole program. It must never be changed." },
                ],
            },
            {
                label: "Ranges",
                entries: [
                    { lesson: "111-integer-types.html", code: "short", text: "Here 2 bytes: $v{-32768} to $v{32767}. $t{long} and $t{long long}: 8 bytes, up to $v{9223372036854775807}." },
                    { lesson: "112-unsigned.html", code: "unsigned int", text: "$v{0} to $v{4294967295}. Unsigned arithmetic wraps around instead of overflowing." },
                    { lesson: "40-undefined-behavior.html", code: "int", text: "From $v{-2147483648} to $v{2147483647}. Going outside this range is overflow, which is undefined behavior." },
                    { lesson: "65-characters.html", code: "char", text: "Typically from $v{-128} to $v{127}." },
                    { lesson: "116-floats.html", code: "float", text: "4 bytes, about 7 significant digits. $t{double}: 8 bytes, about 16 digits, up to about $c{1.8 × 10^308}." },
                ],
            },
        ],
    },
    {
        title: "Statements",
        groups: [
            {
                entries: [
                    { lesson: "13-whitespace-comments.html", code: "// comment", text: "Ignored until the end of the line." },
                    { lesson: "13-whitespace-comments.html", code: "/* comment */", text: "Ignored, and can span multiple lines." },
                    { lesson: "19-blocks.html", code: "{\n  ...\n}", text: "A block. Variables created inside it are removed at its $c{\\}}." },
                    { lesson: "21-if.html", code: "if (c) {\n  ...\n}", text: "Runs the block if $c{c} is non-zero, and skips it otherwise." },
                    { lesson: "22-else.html", code: "} else {\n  ...\n}", text: "Runs if the $c{if} block above didn't." },
                    { lesson: "27-else-if.html", code: "} else if (c) {\n  ...\n}", text: "Checked only if every $c{if} and $c{else if} above was skipped." },
                    { lesson: "32-while.html", code: "while (c) {\n  ...\n}", text: "Runs the block again and again, as long as $c{c} is non-zero." },
                    { lesson: "44-compound-assignment.html", code: "x += y;", text: "Same as $c{x = x + (y);}. Similarly $c{-=}, $c{*=}, $c{/=}, and $c{%=}." },
                    { lesson: "44-compound-assignment.html", code: "x++;", text: "Same as $c{x += 1;}. $c{x--;} is the same as $c{x -= 1;}." },
                    { lesson: "47-for.html", code: "for (setup; c; update) {\n  ...\n}", text: "Same as $c{setup;} followed by $c{while (c) \\{ ... update; \\}}. A variable created in $c{setup} is removed when the loop finishes." },
                    { lesson: "118-loop-control.html", code: "do {\n  ...\n} while (c);", text: "Like $c{while}, but checks $c{c} after each run, so the block runs at least once." },
                    { lesson: "118-loop-control.html", code: "break;", text: "Ends the innermost loop (or $c{switch}). $c{continue;} skips to the loop's next run." },
                    { lesson: "120-switch.html", code: "switch (x) {\n  case 1:\n    ...\n    break;\n  default:\n    ...\n}", text: "Jumps to the $c{case} matching $c{x}, or to $c{default}. Without $c{break}, execution falls through into the next case." },
                    { lesson: "54-functions-i.html", code: "int f(int x) {\n  ...\n  return x;\n}", text: "A function definition. Calling it creates its parameters, set to the arguments' values; $c{return} ends it and gives the call its value. Every program starts at $c{main}." },
                    { lesson: "55-functions-ii.html", code: "void f(int x)", text: "A $t{void} function doesn't return a value. Parameters are copies, so changing one doesn't affect the caller." },
                    { lesson: "60-pointer-parameters.html", code: "void f(int *p)", text: "Pass a variable's address to let a function change it." },
                    { lesson: "95-array-parameters.html", code: "int f(int a[])", text: "In a parameter list, $c{int a[]} (even $c{int a[3]}) means $c{int *a}. Pass the length separately." },
                    { lesson: "58-recursion.html", code: "return n * f(n - 1);", text: "A function can call itself (recursion). Each call has its own copies of the function's variables. It needs a base case that doesn't call itself." },
                    { lesson: "156-command-line.html", code: "int main(int argc, char *argv[])", text: "Receives the command-line arguments: $c{argv[0]} is the program's name." },
                    { lesson: "57-exit-status.html", code: "return 1;", text: "In $c{main}, ends the program. The value is the exit status: $v{0} means success, anything else means something went wrong." },
                    { lesson: "62-function-declarations.html", code: "int f(int x);", text: "A function declaration (prototype): lets the function be called before, or without, its definition." },
                    { lesson: "70-include.html", code: "#include \"file.h\"", text: "Replaced, before compiling, by the entire contents of $c{file.h}." },
                    { lesson: "71-standard-headers.html", code: "#include <stdio.h>", text: "Declares the standard input and output functions, like $c{putchar} and $c{puts}." },
                    { lesson: "74-define.html", code: "#define SQUARE(x) ((x) * (x))", text: "A macro: the preprocessor replaces each use with the text after it, before compiling. Put parentheses around arguments and the whole body." },
                ],
            },
        ],
    },
    {
        title: "Arrays",
        notes: [
            {
                lesson: "76-arrays-i.html",
                code: "",
                text: "An array is a group of values of the same type, one after another in memory. Its type is written like $t{int[3]}: an array of 3 $t{int}s.",
            },
        ],
        groups: [
            {
                label: "Array variables",
                entries: [
                    { lesson: "76-arrays-i.html", code: "int a[3];", text: "An array variable of 3 $t{int}s: $c{a[0]}, $c{a[1]}, and $c{a[2]}. Its type is $t{int[3]}." },
                    { lesson: "77-arrays-ii.html", code: "int a[4] = {1, 2};", text: "Elements left out of the list are set to $v{0}. With $c{int a[] = \\{...\\}}, the size comes from the list." },
                    { lesson: "84-pointer-arithmetic.html", code: "p + 1", text: "If $n{p} points to an array element, points to the next element. Must stay inside the array, or just past its end." },
                    { lesson: "85-decay.html", code: "a", text: "An array used in an expression decays into a pointer to its first element, $c{&a[0]}. Not with $c{&a}, which points to the whole array, or $c{sizeof a}." },
                    { lesson: "86-array-sizes.html", code: "sizeof a / sizeof a[0]", text: "The number of elements in the array variable $n{a}. Doesn't work on a pointer: $c{sizeof p} is just a pointer's size." },
                    { lesson: "94-2d-arrays.html", code: "int m[2][3];", text: "An array of 2 arrays of 3 $t{int}s. $c{m[i][j]} is element $c{j} of row $c{i}." },
                ],
            },
            {
                label: "Arrays from malloc",
                entries: [
                    { lesson: "88-malloc.html", code: "int* p = malloc(3 * sizeof(int));", text: "Gets a nameless block with room for 3 $t{int}s, which lasts until the program ends. Needs $c{#include <stdlib.h>}." },
                    { lesson: "88-malloc.html", code: "p = q;", text: "If nothing points to a block anymore, it still exists, but can't be used: a memory leak." },
                ],
            },
            {
                label: "Array rules",
                entries: [
                    { lesson: "101-array-rules.html", code: "b = a;", text: "Arrays can't be assigned to. Copy them element by element." },
                    { lesson: "101-array-rules.html", code: "int b[3] = a;", text: "An array can only be initialized with a brace-enclosed list or (for a $t{char} array) a string literal." },
                    { lesson: "101-array-rules.html", code: "{1, 2, 3}", text: "An initializer list isn't an expression: it can only appear in an initializer. It can't be assigned, passed, or pointed to." },
                    { lesson: "101-array-rules.html", code: "char s[3] = \"abc\";", text: "Allowed, but there's no room for the $c{'\\0'}, so $n{s} isn't a string." },
                    { lesson: "96-functions-and-arrays.html", code: "int f(void)[3];", text: "A function can't return an array." },
                ],
            },
        ],
    },
    {
        title: "Characters",
        groups: [
            {
                label: "ASCII codes",
                entries: [
                    { lesson: "65-characters.html", code: "'\\0'", text: "$v{0}: the null character, which ends a string." },
                    { lesson: "65-characters.html", code: "'\\n'", text: "$v{10}: newline." },
                    { lesson: "65-characters.html", code: "' '", text: "$v{32}: space." },
                    { lesson: "65-characters.html", code: "'0' to '9'", text: "In order: 0 48, 1 49, 2 50, 3 51, 4 52, 5 53, 6 54, 7 55, 8 56, 9 57. $c{c - '0'} turns a digit character into its value." },
                    { lesson: "65-characters.html", code: "'A' to 'Z'", text: "In order: A 65, B 66, C 67, D 68, E 69, F 70, G 71, H 72, I 73, J 74, K 75, L 76, M 77, N 78, O 79, P 80, Q 81, R 82, S 83, T 84, U 85, V 86, W 87, X 88, Y 89, Z 90." },
                    { lesson: "65-characters.html", code: "'a' to 'z'", text: "In order: a 97, b 98, c 99, d 100, e 101, f 102, g 103, h 104, i 105, j 106, k 107, l 108, m 109, n 110, o 111, p 112, q 113, r 114, s 115, t 116, u 117, v 118, w 119, x 120, y 121, z 122. Each is 32 more than its uppercase letter." },
                ],
            },
        ],
    },
    {
        title: "Library functions",
        groups: [
            {
                entries: [
                    { lesson: "67-putchar.html", code: "int putchar(int c);", text: "Prints the character with code $c{c} to stdout." },
                    { lesson: "88-malloc.html", code: "void *malloc(size_t size);", text: "In $c{stdlib.h}. Returns a pointer to a new block of $c{size} bytes. ($t{size_t} is an unsigned integer type for sizes.)" },
                    { lesson: "105-puts.html", code: "int puts(const char *s);", text: "Prints the string $c{s} points to, up to its $c{'\\0'}, then a newline." },
                    { lesson: "108-printf.html", code: "printf(\"%d %c %s %f\\n\", ...)", text: "In $c{stdio.h}. Prints its format string, replacing each conversion with the next argument: $c{%d} an $t{int}, $c{%c} a character, $c{%s} a string, $c{%f} a $t{double}, $c{%%} a $c{%}." },
                    { lesson: "109-printf-formatting.html", code: "%5.2f", text: "Width 5 (padded with spaces on the left; $c{%-5d} pads on the right, $c{%05d} with zeros), and 2 digits after the decimal point. $c{%x} prints hexadecimal." },
                    { lesson: "149-free.html", code: "free(p);", text: "In $c{stdlib.h}. Ends the lifetime of a block from $c{malloc}. Every block should be freed exactly once." },
                    { lesson: "151-realloc.html", code: "p = realloc(p, n);", text: "Returns a block of $c{n} bytes with the same contents, possibly moved. Returns $c{NULL} if it fails." },
                    { lesson: "152-string-functions.html", code: "strlen(s)", text: "In $c{string.h}, with $c{strcpy(dest, src)}, $c{strcat(dest, src)}, and $c{strcmp(a, b)} (zero if equal). None of them check the destination's size." },
                    { lesson: "154-input.html", code: "scanf(\"%d\", &n)", text: "In $c{stdio.h}. Reads input in the format given, and returns how many values it stored. $c{getchar()} reads one character, and $c{fgets(s, size, stdin)} reads a line." },
                    { lesson: "155-files.html", code: "FILE *f = fopen(\"a.txt\", \"r\");", text: "Opens a file ($c{\"w\"} to write, $c{\"a\"} to append), or returns $c{NULL}. Use with $c{fprintf}, $c{fscanf}, $c{fgets}, and close with $c{fclose(f)}." },
                ],
            },
        ],
    },
    {
        title: "The toy machine",
        notes: [
            {
                lesson: "124-the-machine.html",
                code: "",
                text: "256 bytes of memory, registers $c{A} to $c{D} (one byte each), and 8 lights wired to the byte at address $v{255}. The destination comes first, like the left side of $c{=}.",
            },
        ],
        groups: [
            {
                entries: [
                    { lesson: "124-the-machine.html", code: "SET A, 5", text: "$c{A = 5}. $c{COPY A, B} is $c{A = B}." },
                    { lesson: "124-the-machine.html", code: "LOAD A, [200]", text: "$c{A} = the byte at address $v{200}. $c{STORE [200], A} is the reverse. $c{[B]} uses the address in $c{B}, like a pointer." },
                    { lesson: "126-machine-arithmetic.html", code: "ADD A, B", text: "$c{A = A + B}. Also $c{SUB}, $c{AND}, $c{OR}, $c{XOR}, $c{SHL}, $c{SHR}, and $c{NOT A}. The second operand can be a number. Results wrap around, like $t{unsigned char}." },
                    { lesson: "125-binary-and-hex.html", code: "0b1010, 0x2A", text: "Binary and hexadecimal numbers. Each hex digit is 4 bits. These work in C too." },
                    { lesson: "127-jumps.html", code: "JNZ A, label", text: "Jump to $c{label:} if $c{A} isn't zero. $c{JZ} jumps if it is, and $c{JMP} always jumps. $c{HALT} stops." },
                    { lesson: "135-the-stack.html", code: "CALL label", text: "Push the return address and jump. $c{RET} pops it and jumps back. $c{PUSH A} and $c{POP A} use the stack too; $c{SP} is the address of its next free byte." },
                ],
            },
        ],
    },
    {
        title: "Undefined behavior",
        notes: [
            {
                lesson: "40-undefined-behavior.html",
                code: "",
                text: "C makes no promises about what happens if a program does any of these. On a real computer, the program might crash, keep going with garbage values, or do something else entirely.",
            },
        ],
        groups: [
            {
                entries: [
                    { lesson: "40-undefined-behavior.html", code: "x / 0", text: "Dividing by zero. $c{x % 0} is undefined too." },
                    { lesson: "40-undefined-behavior.html", code: "int u;\nint v = u + 1;", text: "Using a variable's value before it has one." },
                    { lesson: "41-errors-or-ub.html", code: "int* p;\n*p = 1;", text: "Following a pointer that doesn't point to anything." },
                    { lesson: "84-pointer-arithmetic.html", code: "p - 1", text: "Pointer arithmetic that leaves an array (here, if $n{p} points to the first element). Just past the end is allowed, but can't be followed." },
                    { lesson: "40-undefined-behavior.html", code: "2147483647 + 1", text: "Overflowing an $t{int}: any step of a calculation whose result is outside the range of an $t{int}, even if later steps would bring it back into range." },
                    { lesson: "117-ieee.html", code: "1.0 / 0.0", text: "Floating-point division by zero, or a result too big for its type. (IEEE 754 gives infinity, but C doesn't require it.)" },
                    { lesson: "150-memory-errors.html", code: "free(p);\n*p = 1;", text: "Using memory after freeing it, or freeing it twice." },
                    { lesson: "78-indexing.html", code: "a[3]", text: "Using an index outside an array (here, if $n{a} has 3 elements)." },
                    { lesson: "100-string-literals.html", code: "char *p = \"hi\";\np[0] = 'y';", text: "Changing a string literal's array." },
                    { lesson: "96-functions-and-arrays.html", code: "int *f(void) {\n  int a[3];\n  return a;\n}", text: "Using a pointer to a variable that no longer exists, such as a function's local array after the function returns." },
                ],
            },
        ],
    },
];
export { REFERENCE_SECTIONS };
