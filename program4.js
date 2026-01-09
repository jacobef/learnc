{
  const { createCodeEditorTemplate, stripLineComments } = window.MB;

  function linesForHint(text) {
    return text
      .split(/\r?\n/)
      .map((line) => stripLineComments(line || ""))
      .map((res) => (res.text || "").trim())
      .filter((t) => t && t !== ";");
  }

  createCodeEditorTemplate({
    startCode: "int rain;",
    textareaWidth: "100%",
    textareaMinLines: 1,
    allowNewLines: false,
    targetState: [
      { name: "cloud", type: "int", value: "", address: "<i>(any)</i>" },
    ],
    next: "program5.html",
    instructions: () =>
      'Until now, you have been editing the program state to match the code. Now you will be editing the code to match the program state. Edit the line so that "Your code\'s final state" matches the "Target final state", then press $b{Check}.',
    hints: (ctx) => {
      const lines = linesForHint(ctx.text || "");
      if (!lines.length) {
        return "Edit the line to create an empty variable named $n{cloud}, of type $t{int}. Look at the earlier programs if you forget how this is done.";
      }
      const almostDecl = lines.find((l) =>
        /^int\s+[A-Za-z_][A-Za-z0-9_]*\s*$/.test(l),
      );
      const almostAssign = lines.find((l) =>
        /^[A-Za-z_][A-Za-z0-9_]*\s*=\s*-?\d+\s*$/.test(l),
      );
      if (almostDecl || almostAssign) {
        return "You need a semicolon at the end of the line.";
      }
      if (!lines.some((l) => /int\s+cloud\s*;/.test(l))) {
        const wrongName = lines.find((l) =>
          /^int\s+[A-Za-z_][A-Za-z0-9_]*\s*;/.test(l),
        );
        if (wrongName) {
          return "The variable's name should be $n{cloud}.";
        }
        return "Declare $n{cloud} as an $t{int}.";
      }
      if (lines.some((l) => /cloud\s*=/.test(l))) {
        return "Leave $n{cloud} empty—no assignments to $n{cloud}.";
      }
      return "Edit the line to create an empty variable named $n{cloud}, of type $t{int}. Look at the earlier programs if you forget how this is done.";
    },
  });
}
