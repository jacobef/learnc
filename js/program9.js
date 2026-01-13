"use strict";
/// <reference path="./shared-program-template.ts" />
{
    const createProgramTemplate = window.MB.createProgramTemplate;
    createProgramTemplate({
        initialInstructions: "Click $runLineButton to continue.",
        steps: [
            {
                code: "int a; // mary had a\n",
                editable: false,
                instructions: "Anything after $c{//} on a line is ignored. This is called a line comment.",
            },
            {
                code: "// little lamb\n",
                editable: false,
                instructions: "Comments can appear on their own lines as well.",
            },
            {
                code: "int b; int c;\n",
                editable: false,
                instructions: "Multiple statements can appear on one line.",
            },
            {
                code: "int d; // int e;\n",
                editable: true,
                hints: (ctx) => {
                    const [d, e] = ctx.getBoxesByName(ctx.boxes, "d", "e");
                    if (!d) {
                        return "Add the $n{d} variable.";
                    }
                    if (e) {
                        return "$c{// int e;} is a comment.";
                    }
                    if (d.type !== "int") {
                        return "$n{d}'s type should be $t{int}.";
                    }
                    if (d.value !== "") {
                        return "$n{d}'s value should be empty.";
                    }
                    return null;
                },
            },
            {
                code: "int f\n= 5\n;\n",
                editable: false,
                instructions: "A statement can be split across multiple lines.",
            },
            {
                code: "/* whose fleece\nwas white as snow */\n",
                editable: false,
                instructions: "A comment can appear on multiple lines, or within a line, beginning with $c{/*} and ending with $c{*/}. This is called a block comment.",
            },
            {
                code: "int g /* hi */ = 3;\n",
                editable: false,
                instructions: "A comment can appear on multiple lines, or within a line, beginning with $c{/*} and ending with $c{*/}. This is called a block comment.",
            },
            {
                code: "int\nh // = 9;\n/* = 10;\n= 11;\n*/ = 12;\n// = 13;\n",
                editable: true,
                hints: (ctx) => {
                    const h = ctx.getBoxByName(ctx.boxes, "h");
                    if (!h) {
                        return "Add the $n{h} variable.";
                    }
                    if (h.type !== "int") {
                        return "$n{h}'s type should be $t{int}.";
                    }
                    if (h.value === "9") {
                        return "$c{// = 9;} is a comment.";
                    }
                    else if (h.value === "10" || h.value === "11") {
                        return "$c{/* = 10;\n= 11;\n*/} is a comment.";
                    }
                    else if (h.value === "13") {
                        return "$c{// = 13;} is a comment.";
                    }
                    else if (h.value === "") {
                        return "$n{h}'s value shouldn't be empty; one of these numbers is not inside a comment.";
                    }
                    else if (h.value !== "12") {
                        return `I'm not sure where you're getting $v{${h.value}} from.`;
                    }
                    return null;
                },
            },
        ],
        next: "program10.html",
        workspace: { allowVariableCreation: true },
    });
}
