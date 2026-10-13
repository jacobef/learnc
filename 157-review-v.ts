import { createProgramTemplate } from "./shared-program-template.js";

const basic = (ctx: { basicHint: string | null }) => ctx.basicHint;

createProgramTemplate({
  implicitMain: false,
  initialInstructions: "No instructions for this one, but the $referenceButton might come in handy. Good luck!",
  steps: [
    { code: "#include <stdlib.h>\n\n" },
    { code: "typedef struct {\n  char tag;\n  unsigned int flags;\n  double *readings;\n} sensor;\n\n" },
    { code: "void record(sensor *p, int i, double value) {\n" },
    { code: "  p->readings[i] = value > 100.0 ? 100.0 : value;\n", editable: [true, false, true], hints: basic },
    { code: "  p->flags |= 1u << i;\n", editable: [false, true, true], hints: basic },
    { code: "}\n\n" },
    { code: "int main(void) {\n" },
    { code: "  sensor s = {'A', 0, malloc(3 * sizeof(double))};\n", editable: true, hints: basic },
    { code: "  record(&s, 0, 42.5);\n" },
    { code: "  record(&s, 2, 250.0);\n" },
    { code: "  sensor t = s;\n", editable: true, hints: basic },
    { code: "  t.tag++;\n", editable: true, hints: basic },
    { code: "  record(&t, 1, -1.5);\n" },
    { code: "  int unset = ~t.flags & 7u;\n", editable: true, hints: basic },
    { code: "  free(s.readings);\n", editable: true, hints: basic },
    { code: "  return 0;\n" },
    { code: "}\n" },
  ],
  next: "sandbox.html?finished=1",
  workspace: { allowVariableCreation: true },
});
