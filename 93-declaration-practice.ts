import { createCodeEditorTemplate } from "./shared-code-editor.js";

const targets: Array<[string, string, string]> = [
  ["pp", "int**", "$c{**pp} is an $t{int}, so $n{pp} is a pointer to a pointer to an $t{int}."],
  ["d", "double[4]", "$c{d[i]} is a $t{double}, so $n{d} is an array of 4 $t{double}s."],
  ["names", "char*[2]", "$c{*names[i]} is a $t{char}. $c{[]} has higher precedence than $c{*}, so $n{names} is an array of 2 pointers to $t{char}s."],
  ["pa", "int (*)[5]", "$c{(*pa)[i]} is an $t{int}, so $c{*pa} is an array of 5 $t{int}s, and $n{pa} is a pointer to it."],
];

createCodeEditorTemplate({
  solve: "int **pp;\ndouble d[4];\nchar *names[2];\nint (*pa)[5];\n",
  textareaMinLines: 5,
  instructions:
    "Write declarations so that your code's final state matches the target final state. Don't initialize the variables.\nIf you're stuck, write out how each variable would be used, like \"$c{(*pa)[i]} is an $t{int}\". For types involving arrays, you can also click the ? next to the type in the target state to see it spelled out.",
  hints: (ctx) => {
    const state = ctx.applyUserProgram();
    if (!state) return ctx.diagnostic?.message || "Your code doesn't compile yet.";
    for (const [name, type, reading] of targets) {
      const box = state.find((candidate) => candidate.name === name);
      if (!box) return `Declare a variable named $n{${name}}, of type $t{${type}}.`;
      if (box.type.replace(/\s+/g, "") !== type.replace(/\s+/g, "")) {
        return `${reading} Its type should be $t{${type}}, not $t{${box.type}}.`;
      }
    }
    if (state.some((box) => box.value !== "" && !box.arrayShape?.length)) {
      return "Leave the variables uninitialized.";
    }
    const extra = state.find((box) => !box.arrayRoot && !targets.some(([name]) => name === box.name));
    if (extra) return `Remove $n{${extra.name}}; only the variables in the target are needed.`;
    return null;
  },
  next: "94-2d-arrays.html",
});
