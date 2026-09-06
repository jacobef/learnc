# SQLite in cBoxes

`cboxes-sqlite` runs an in-memory SQLite database inside the native cBoxes C
interpreter. It accepts an SQLite amalgamation, applies the small strict-C
compatibility adapter in memory, and never modifies the supplied `sqlite3.c`.

Build the optimized launcher:

```sh
cargo build --profile analysis \
  --manifest-path rust/c_interpreter/Cargo.toml \
  --bin cboxes-sqlite
```

Generate an amalgamation from a SQLite checkout, then start the shell:

```sh
cd /path/to/sqlite
./configure --disable-shared --disable-readline
make sqlite3.c sqlite3.h

/path/to/cBoxes/rust/c_interpreter/target/analysis/cboxes-sqlite \
  --sqlite /path/to/sqlite/sqlite3.c
```

Enter SQL ending in `;`. `.reset` clears the in-memory database and `.quit`
exits. The interpreter, SQLite connection, and database stay live for the
whole session. Standard input, output, and error are native streams, so piped
scripts are consumed incrementally and earlier statements are not replayed.

For a one-shot script, pipe SQL or pass it as an argument:

```sh
printf 'select 6 * 7 as answer;\n' | \
  rust/c_interpreter/target/analysis/cboxes-sqlite --sqlite /path/to/sqlite3.c
```

The CLI is intentionally memory-only. It does not yet open database files,
load extensions, use WAL, or provide all dot commands from SQLite's own shell.
The default limits are adjustable with `--allocation-limit-mib` and
`--step-limit`; pass `none` to disable either limit.
