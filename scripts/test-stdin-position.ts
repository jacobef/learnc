import assert from "node:assert/strict";
import { stdinReadPosition } from "../js/shared-stdin-position.js";

for (const [position, offset, detail] of [
  [0, 0, "line 1, column 1"],
  [1, 1, "line 1, column 2"],
  [2, 1, "byte 2 of this character"],
  [3, 2, "line 1, column 3"],
  [6, 2, "byte 4 of this character"],
  [7, 4, "line 1, column 4"],
  [8, 5, "line 2, column 1"],
  [9, 6, "End of available input"],
] as const) {
  const result = stdinReadPosition("aé🙂\nb", position);
  assert.equal(result.offset, offset, `byte ${position}`);
  assert.ok(result.label.includes(detail), result.label);
}
for (const marker of ["\u0004", "␄"]) {
  const result = stdinReadPosition(`a${marker}ignored`, 1);
  assert.equal(result.offset, 1);
  assert.ok(result.label.includes("EOF"));
  assert.equal(stdinReadPosition(`a${marker}ignored`, 100).offset, 1);
}
assert.equal(stdinReadPosition("abc", -1).offset, 0);
assert.equal(stdinReadPosition("abc", 100).offset, 3);
assert.equal(stdinReadPosition("", 0).offset, 0);
console.log("PASS stdin cursor mapping, UTF-8 boundaries, and EOF markers");
