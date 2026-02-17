Always run `tsc` after making edits to Typescript. Never manually edit Javascript.

All of the shared*.ts files, sandbox.ts, 4-code-editing-i.ts, and 5-code-editing-ii.ts, and any CSS, are yours to edit as you wish; I generally won't be touching or looking at them. This means you are free to make sweeping edits on a whim that don't respect backwards compatibility. (Though obviously, if you do break compatibility, then change the consumers of that API accordingly.) Never keep dead code around for "backward compatibility" purposes.

If you're working on a feature and find some messy code on the way that could be cleaned up, clean it up. If and only if cleaning it up would be a massive ordeal, ask me if I want it cleaned up, after implementing the feature I originally asked for.

If I say to do a "YOLO" cleanup pass or refactor, that means you should be as aggressive as possible about cleanup, and can run a large risk of breaking or changing user-facing functionality, resting assured that I can always revert to the last commit if I don't like what you've done.
