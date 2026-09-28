# Benchmark inputs

Unmodified upstream files fetched from pinned release tags:

- zlib puff v1.3.1: https://github.com/madler/zlib/tree/v1.3.1/contrib/puff (zlib license; notice in source).
- cJSON v1.7.18: https://github.com/DaveGamble/cJSON/tree/v1.7.18 (MIT; notices in source/header).
- jsmn v1.1.0: https://github.com/zserge/jsmn/tree/v1.1.0 (MIT; notice in header).
- tiny-AES-c v1.0.0: https://github.com/kokke/tiny-AES-c/tree/v1.0.0 (public domain / Unlicense).

`projects.ts` combines each header and implementation into a single translation
unit without expanding system headers. It removes only the local header include
and defines cJSON's public symbol visibility macro for portability.
