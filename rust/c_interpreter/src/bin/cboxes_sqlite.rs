use c_interpreter::{NativeExecutionOptions, NativeStreamIo, run_native_source};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

const SQLITE_PREFIX: &str = r#"
#define SQLITE_THREADSAFE 0
#define HAVE_STDINT_H 1
#define SQLITE_OMIT_LOAD_EXTENSION 1
#define SQLITE_OMIT_SHARED_CACHE 1
#define SQLITE_OMIT_WAL 1
#define SQLITE_OMIT_DEPRECATED 1
#define SQLITE_OMIT_DESERIALIZE 1
#define SQLITE_OMIT_AUTOINIT 1
#define SQLITE_UNTESTABLE 1
#define SQLITE_OS_OTHER 1
"#;

const MEMORY_VFS: &str = r#"
typedef void (*CboxesDlSymbol)(void);

static int cboxesVfsOpen(sqlite3_vfs *vfs, sqlite3_filename name,
                        sqlite3_file *file, int flags, int *out_flags) {
  (void)vfs; (void)name; (void)file; (void)flags; (void)out_flags;
  return SQLITE_CANTOPEN;
}
static int cboxesVfsDelete(sqlite3_vfs *vfs, const char *name, int sync_dir) {
  (void)vfs; (void)name; (void)sync_dir;
  return SQLITE_IOERR_DELETE;
}
static int cboxesVfsAccess(sqlite3_vfs *vfs, const char *name, int flags,
                           int *result) {
  (void)vfs; (void)name; (void)flags; *result = 0;
  return SQLITE_OK;
}
static int cboxesVfsFullPathname(sqlite3_vfs *vfs, const char *name, int size,
                                 char *output) {
  int i;
  (void)vfs;
  for (i = 0; i + 1 < size && name[i] != '\0'; ++i) output[i] = name[i];
  output[i] = '\0';
  return SQLITE_OK;
}
static void *cboxesVfsDlOpen(sqlite3_vfs *vfs, const char *name) {
  (void)vfs; (void)name; return 0;
}
static void cboxesVfsDlError(sqlite3_vfs *vfs, int size, char *message) {
  (void)vfs; if (size > 0) message[0] = '\0';
}
static CboxesDlSymbol cboxesVfsDlSym(sqlite3_vfs *vfs, void *handle,
                                     const char *name) {
  (void)vfs; (void)handle; (void)name; return 0;
}
static void cboxesVfsDlClose(sqlite3_vfs *vfs, void *handle) {
  (void)vfs; (void)handle;
}
static int cboxesVfsRandomness(sqlite3_vfs *vfs, int size, char *output) {
  unsigned state = 0x9e3779b9u;
  int i;
  (void)vfs;
  for (i = 0; i < size; ++i) {
    state = state * 1664525u + 1013904223u;
    output[i] = (char)(state >> 24);
  }
  return size;
}
static int cboxesVfsSleep(sqlite3_vfs *vfs, int microseconds) {
  (void)vfs; return microseconds;
}
static int cboxesVfsCurrentTime(sqlite3_vfs *vfs, double *time) {
  (void)vfs; *time = 2460000.5; return SQLITE_OK;
}
static int cboxesVfsGetLastError(sqlite3_vfs *vfs, int size, char *message) {
  (void)vfs; if (size > 0) message[0] = '\0'; return 0;
}
static sqlite3_vfs cboxesVfs = {
  1, (int)sizeof(sqlite3_file), 1024, 0, "cboxes-memory-only", 0,
  cboxesVfsOpen, cboxesVfsDelete, cboxesVfsAccess, cboxesVfsFullPathname,
  cboxesVfsDlOpen, cboxesVfsDlError, cboxesVfsDlSym, cboxesVfsDlClose,
  cboxesVfsRandomness, cboxesVfsSleep, cboxesVfsCurrentTime,
  cboxesVfsGetLastError, 0, 0, 0, 0
};
int sqlite3_os_init(void) { return sqlite3_vfs_register(&cboxesVfs, 1); }
int sqlite3_os_end(void) { return SQLITE_OK; }
"#;

#[derive(Debug)]
struct CliOptions {
    sqlite: PathBuf,
    allocation_limit_bytes: Option<usize>,
    step_limit: Option<usize>,
    sql: Vec<String>,
}

fn usage() -> &'static str {
    "usage: cboxes-sqlite [--sqlite PATH] [--allocation-limit-mib N|none] \
     [--step-limit N|none] [SQL]"
}

fn default_sqlite_path() -> Option<PathBuf> {
    env::var_os("CBOXES_SQLITE_AMALGAMATION")
        .map(PathBuf::from)
        .or_else(|| {
            let path = PathBuf::from("sqlite3.c");
            path.is_file().then_some(path)
        })
        .or_else(|| {
            let path = PathBuf::from("/private/tmp/cboxes-sqlite-substantial/sqlite3.c");
            path.is_file().then_some(path)
        })
}

fn parse_limit(value: &str, multiplier: usize) -> Result<Option<usize>, String> {
    if value == "none" {
        return Ok(None);
    }
    value
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_mul(multiplier))
        .map(Some)
        .ok_or_else(|| format!("invalid limit {value:?}"))
}

fn parse_args() -> Result<CliOptions, String> {
    let mut args = env::args().skip(1);
    let mut sqlite = None;
    let mut allocation_limit_bytes = Some(512 * 1024 * 1024);
    let mut step_limit = Some(100_000_000);
    let mut sql = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sqlite" => {
                sqlite = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--sqlite needs a path".to_owned())?,
                ));
            }
            "--allocation-limit-mib" => {
                allocation_limit_bytes = parse_limit(
                    &args
                        .next()
                        .ok_or_else(|| "--allocation-limit-mib needs a value".to_owned())?,
                    1024 * 1024,
                )?;
            }
            "--step-limit" => {
                step_limit = parse_limit(
                    &args
                        .next()
                        .ok_or_else(|| "--step-limit needs a value".to_owned())?,
                    1,
                )?;
            }
            "-h" | "--help" => return Err(usage().to_owned()),
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg:?}")),
            _ => {
                sql.push(arg);
                sql.extend(args);
                break;
            }
        }
    }
    let sqlite = sqlite
        .or_else(default_sqlite_path)
        .ok_or_else(|| "could not find sqlite3.c; pass --sqlite PATH".to_owned())?;
    Ok(CliOptions {
        sqlite,
        allocation_limit_bytes,
        step_limit,
        sql,
    })
}

fn replace_once(source: &mut String, old: &str, new: &str, label: &str) -> Result<(), String> {
    if new.is_empty() {
        if source.contains(old) {
            *source = source.replacen(old, new, 1);
        }
        return Ok(());
    }
    let old_present = source.contains(old);
    let new_present = source.contains(new);
    if new.contains(old) {
        if new_present {
            return Ok(());
        }
    } else if old_present {
        *source = source.replacen(old, new, 1);
        return Ok(());
    } else if new_present {
        return Ok(());
    }
    if old_present {
        *source = source.replacen(old, new, 1);
        return Ok(());
    }
    Err(format!(
        "SQLite source does not match the supported amalgamation at {label}"
    ))
}

fn adapt_sqlite(mut source: String) -> Result<String, String> {
    let patches = [
        (
            "return p<db->lookaside.pMiddle ? db->lookaside.szTrue : LOOKASIDE_SMALL;",
            "return (const unsigned char*)p<(unsigned char*)db->lookaside.pMiddle\n             ? db->lookaside.szTrue\n             : LOOKASIDE_SMALL;",
            "lookaside pointer comparison",
        ),
        (
            "sqlite3GlobalConfig.m = *va_arg(ap, sqlite3_mem_methods*);",
            "sqlite3GlobalConfig.m = *va_arg(ap, const sqlite3_mem_methods*);",
            "allocator configuration",
        ),
        (
            "sqlite3GlobalConfig.pcache2 = *va_arg(ap, sqlite3_pcache_methods2*);",
            "sqlite3GlobalConfig.pcache2 = *va_arg(ap, const sqlite3_pcache_methods2*);",
            "page-cache configuration",
        ),
        (
            "bufpt = va_arg(ap,char*);",
            "bufpt = (char*)va_arg(ap,void*);",
            "SQLite printf string arguments",
        ),
        (
            "char const *azArg[6];",
            "char *azArg[6];",
            "schema callback arguments",
        ),
        (
            "char *zSchemaTabName;",
            "const char *zSchemaTabName;",
            "schema table name",
        ),
        (
            "azArg[1] = zSchemaTabName = SCHEMA_TABLE(iDb);",
            "zSchemaTabName = SCHEMA_TABLE(iDb);\n  azArg[1] = (char*)zSchemaTabName;",
            "schema table callback pointer",
        ),
        (
            "  int iDb = pData->iDb;\n\n  assert( argc==5 );",
            "  int iDb = pData->iDb;\n  const char *azInit[3];\n\n  assert( argc==5 );",
            "schema callback local array",
        ),
        (
            "    db->init.azInit = (const char**)argv;",
            "    azInit[0] = argv[0];\n    azInit[1] = argv[1];\n    azInit[2] = argv[2];\n    db->init.azInit = azInit;",
            "schema callback array type",
        ),
        (
            "*(void **)pPage->page.pExtra = 0;",
            "((PgHdr*)pPage->page.pExtra)->pPage = 0;",
            "page-header effective type",
        ),
        (
            "memset(&pPgHdr->pDirty, 0, sizeof(PgHdr) - offsetof(PgHdr,pDirty));",
            "memset(pPgHdr, 0, sizeof(PgHdr));",
            "page-header initialization",
        ),
        (
            "memset(&p->aOp, 0, sizeof(Vdbe)-offsetof(Vdbe,aOp));",
            "memset(p, 0, sizeof(Vdbe));",
            "VDBE initialization",
        ),
        (
            "  pWInfo->pParse = pParse;",
            "  memset(pWInfo, 0, nByteWInfo + sizeof(WhereLoop));\n  pWInfo->pParse = pParse;",
            "where-info allocation initialization",
        ),
        (
            "  memset(&pWInfo->nOBSat, 0,\n         offsetof(WhereInfo,sWC) - offsetof(WhereInfo,nOBSat));\n  memset(&pWInfo->a[0], 0, sizeof(WhereLoop)+nTabList*sizeof(WhereLevel));\n",
            "",
            "where-info tail initialization",
        ),
        (
            "  pTerm = &pWC->a[idx = pWC->nTerm++];\n  if( (wtFlags & TERM_VIRTUAL)==0 )",
            "  pTerm = &pWC->a[idx = pWC->nTerm++];\n  memset(pTerm, 0, sizeof(*pTerm));\n  if( (wtFlags & TERM_VIRTUAL)==0 )",
            "where-term initialization",
        ),
        (
            "  memset(&pTerm->eOperator, 0,\n         sizeof(WhereTerm) - offsetof(WhereTerm,eOperator));\n",
            "",
            "where-term tail initialization",
        ),
        (
            "u8 zChunk[8];                   /* Content of this chunk */",
            "u8 zChunk[];                    /* Content of this chunk */",
            "memory-journal flexible array",
        ),
        (
            "#define fileChunkSize(nChunkSize) (sizeof(FileChunk) + ((nChunkSize)-8))",
            "#define fileChunkSize(nChunkSize) (sizeof(FileChunk) + (nChunkSize))",
            "memory-journal allocation size",
        ),
        (
            "  sqlite3 xdb;",
            "  sqlite3 xdb = {0};",
            "temporary connection initialization",
        ),
        (
            "\n  memset(&xdb, 0, sizeof(xdb));\n  temp1 = pSchema->tblHash;",
            "\n  temp1 = pSchema->tblHash;",
            "temporary connection volatile subobject",
        ),
        (
            "    *(u16*)(&zBuf[i-2]) = *(u16*)&sqlite3DigitPairs.a[kk];",
            "    memcpy(&zBuf[i-2], &sqlite3DigitPairs.a[kk], 2);",
            "strictly conforming two-digit copy",
        ),
        (
            "  azArg = (const char *const*)pTab->u.vtab.azArg;",
            "",
            "virtual-table argument qualification",
        ),
        (
            "  pTab->u.vtab.azArg[1] = db->aDb[iDb].zDbSName;\n\n  /* Invoke the virtual table constructor */",
            "  pTab->u.vtab.azArg[1] = db->aDb[iDb].zDbSName;\n  const char *azArgValues[nArg];\n  for(int iArg = 0; iArg < nArg; ++iArg){\n    azArgValues[iArg] = pTab->u.vtab.azArg[iArg];\n  }\n  azArg = azArgValues;\n\n  /* Invoke the virtual table constructor */",
            "virtual-table argument array effective type",
        ),
        (
            "          escarg = va_arg(ap,char*);",
            "          escarg = (char*)va_arg(ap,void*);",
            "SQLite printf escaped-string argument",
        ),
        (
            "          escarg = va_arg(ap,char*);",
            "          escarg = (char*)va_arg(ap,void*);",
            "SQLite printf quoted-string argument",
        ),
        (
            "          }else if( flag_long ){\n            if( flag_long==2 ){\n              longvalue = va_arg(ap,u64);",
            "          }else if( xtype==etPOINTER ){\n            longvalue = (u64)(uintptr_t)va_arg(ap,void*);\n          }else if( flag_long ){\n            if( flag_long==2 ){\n              longvalue = va_arg(ap,u64);",
            "SQLite printf pointer argument",
        ),
        (
            "                   \"sqlite_returning_%p\", pParse);",
            "                   \"sqlite_returning_%p\", (void*)pParse);",
            "SQLite printf pointer call",
        ),
        (
            "  assert( z<=zTerm );\n  while( *z!=0 && z<zTerm ){",
            "  assert( nByte<0 || z<=zTerm );\n  while( *z!=0 && (nByte<0 || z<zTerm) ){",
            "UTF-8 unbounded-length sentinel",
        ),
        (
            "  pRight = &pLeft[1];",
            "  pRight = pSrc->nSrc ? &pLeft[1] : pLeft;",
            "empty source-list pointer arithmetic",
        ),
    ];
    for (old, new, label) in patches {
        replace_once(&mut source, old, new, label)?;
    }
    Ok(source)
}

const RUN_SQL_DRIVER: &str = r#"
static int cboxesRunSql(sqlite3 *db, const char *sql, int show_rows) {
  while (*sql != '\0') {
    sqlite3_stmt *statement = 0;
    const char *tail = 0;
    int rc = sqlite3_prepare_v2(db, sql, -1, &statement, &tail);
    if (rc != SQLITE_OK) return rc;
    if (statement != 0) {
      int columns = sqlite3_column_count(statement);
      int printed_header = 0;
      while ((rc = sqlite3_step(statement)) == SQLITE_ROW) {
        int column;
        if (show_rows && !printed_header) {
          for (column = 0; column < columns; ++column) {
            if (column) printf("|");
            printf("%s", (char*)sqlite3_column_name(statement, column));
          }
          printf("\n");
          printed_header = 1;
        }
        if (show_rows) {
          for (column = 0; column < columns; ++column) {
            if (column) printf("|");
            if (sqlite3_column_type(statement, column) == SQLITE_NULL) {
              printf("NULL");
            } else {
              printf("%s", (char*)sqlite3_column_text(statement, column));
            }
          }
          printf("\n");
        }
      }
      sqlite3_finalize(statement);
      if (rc != SQLITE_DONE) return rc;
    }
    if (tail == 0 || tail == sql) break;
    sql = tail;
  }
  return SQLITE_OK;
}
"#;

fn c_string(text: &str) -> Result<String, String> {
    if text.contains('\0') {
        return Err("SQL cannot contain a NUL byte".to_owned());
    }
    let mut output = String::with_capacity(text.len() + 2);
    output.push('"');
    for byte in text.bytes() {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'"' => output.push_str("\\\""),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(byte)),
            _ => {
                use std::fmt::Write as _;
                write!(output, "\\{:03o}", byte).unwrap();
            }
        }
    }
    output.push('"');
    Ok(output)
}

fn query_driver(sql: &str) -> Result<String, String> {
    let sql = c_string(sql)?;
    Ok(format!(
        r#"
int main(void) {{
  sqlite3 *db = 0;
  int rc = sqlite3_initialize();
  if (rc == SQLITE_OK) rc = sqlite3_open(":memory:", &db);
  if (rc == SQLITE_OK) rc = cboxesRunSql(db, {sql}, 1);
  if (rc != SQLITE_OK) printf("error: %s\n", db ? sqlite3_errmsg(db) : "initialization failed");
  if (db) sqlite3_close(db);
  sqlite3_shutdown();
  return rc == SQLITE_OK ? 0 : 1;
}}
"#
    ))
}

fn run_query(sqlite: &str, sql: &str, options: &CliOptions) -> Result<bool, String> {
    let driver = query_driver(sql)?;
    let mut source = String::with_capacity(
        SQLITE_PREFIX.len() + sqlite.len() + MEMORY_VFS.len() + RUN_SQL_DRIVER.len() + driver.len(),
    );
    source.push_str(SQLITE_PREFIX);
    source.push_str(sqlite);
    source.push_str(MEMORY_VFS);
    source.push_str(RUN_SQL_DRIVER);
    source.push_str(&driver);
    let result = run_native_source(
        "sqlite-cboxes.c",
        source,
        &NativeExecutionOptions {
            allocation_limit_bytes: options.allocation_limit_bytes,
            execution_step_limit: options.step_limit,
            ..NativeExecutionOptions::default()
        },
    )?;
    print!("{}", result.stdout);
    eprint!("{}", result.stderr);
    Ok(result.exit_status == 0)
}

fn batch(sqlite: &str, sql: String, options: &CliOptions) -> Result<i32, String> {
    Ok(if run_query(sqlite, &sql, options)? {
        0
    } else {
        1
    })
}

const SESSION_DRIVER: &str = r#"
static int cboxesAppend(char **sql, int *length, int *capacity, const char *line) {
  int added = (int)strlen(line);
  int needed = *length + added + 1;
  if (needed > *capacity) {
    int next_capacity = *capacity ? *capacity : 256;
    char *next;
    while (next_capacity < needed) next_capacity *= 2;
    next = realloc(*sql, (size_t)next_capacity);
    if (!next) return SQLITE_NOMEM;
    *sql = next;
    *capacity = next_capacity;
  }
  memcpy(*sql + *length, line, (size_t)added + 1);
  *length += added;
  return SQLITE_OK;
}

int main(void) {
  sqlite3 *db = 0;
  char line[1024];
  char *sql = 0;
  int length = 0;
  int capacity = 0;
  int had_error = 0;
  int rc = sqlite3_initialize();
  if (rc == SQLITE_OK) rc = sqlite3_open(":memory:", &db);
  while (rc == SQLITE_OK) {
    if (CBOXES_INTERACTIVE) {
      printf("%s", length == 0 ? "sqlite-cboxes> " : "          ...> ");
      fflush(stdout);
    }
    if (fgets(line, sizeof(line), stdin) == 0) break;
    if (length == 0 && (!strcmp(line, ".quit\n") || !strcmp(line, ".exit\n")
                     || !strcmp(line, ".quit\r\n") || !strcmp(line, ".exit\r\n"))) {
      break;
    }
    if (length == 0 && (!strcmp(line, ".help\n") || !strcmp(line, ".help\r\n"))) {
      printf("Enter SQL ending in ';'. Commands: .reset, .quit, .help\n");
      continue;
    }
    if (length == 0 && (!strcmp(line, ".reset\n") || !strcmp(line, ".reset\r\n"))) {
      sqlite3_close(db);
      db = 0;
      rc = sqlite3_open(":memory:", &db);
      if (rc == SQLITE_OK) printf("in-memory database reset\n");
      continue;
    }
    rc = cboxesAppend(&sql, &length, &capacity, line);
    if (rc == SQLITE_OK && sqlite3_complete(sql)) {
      rc = cboxesRunSql(db, sql, 1);
      if (rc != SQLITE_OK) {
        printf("error: %s\n", sqlite3_errmsg(db));
        had_error = 1;
        rc = SQLITE_OK;
      }
      length = 0;
      sql[0] = '\0';
    }
  }
  if (rc == SQLITE_OK && length != 0) {
    rc = cboxesRunSql(db, sql, 1);
    if (rc != SQLITE_OK) {
      printf("error: %s\n", sqlite3_errmsg(db));
      had_error = 1;
      rc = SQLITE_OK;
    }
  }
  if (rc != SQLITE_OK) printf("error: %s\n", db ? sqlite3_errmsg(db) : "initialization failed");
  free(sql);
  if (db) sqlite3_close(db);
  sqlite3_shutdown();
  return rc == SQLITE_OK && (!had_error || CBOXES_INTERACTIVE) ? 0 : 1;
}
"#;

#[derive(Debug)]
struct TerminalStreamIo;

impl NativeStreamIo for TerminalStreamIo {
    fn read_stdin(&self, buffer: &mut [u8]) -> io::Result<usize> {
        io::stdin().read(buffer)
    }

    fn write_stdout(&self, bytes: &[u8]) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(bytes)?;
        stdout.flush()
    }

    fn write_stderr(&self, bytes: &[u8]) -> io::Result<()> {
        let mut stderr = io::stderr().lock();
        stderr.write_all(bytes)?;
        stderr.flush()
    }
}

fn stream_session(sqlite: &str, options: &CliOptions, interactive: bool) -> Result<i32, String> {
    let driver = SESSION_DRIVER.replace("CBOXES_INTERACTIVE", if interactive { "1" } else { "0" });
    let mut source = String::with_capacity(
        SQLITE_PREFIX.len() + sqlite.len() + MEMORY_VFS.len() + RUN_SQL_DRIVER.len() + driver.len(),
    );
    source.push_str(SQLITE_PREFIX);
    source.push_str(sqlite);
    source.push_str(MEMORY_VFS);
    source.push_str(RUN_SQL_DRIVER);
    source.push_str(&driver);
    let result = run_native_source(
        "sqlite-cboxes.c",
        source,
        &NativeExecutionOptions {
            stream_io: Some(Arc::new(TerminalStreamIo)),
            allocation_limit_bytes: options.allocation_limit_bytes,
            execution_step_limit: options.step_limit,
            ..NativeExecutionOptions::default()
        },
    )?;
    Ok(result.exit_status)
}

fn run() -> Result<i32, String> {
    let options = parse_args()?;
    let sqlite = fs::read_to_string(&options.sqlite)
        .map_err(|error| format!("cannot read {}: {error}", options.sqlite.display()))?;
    let sqlite = adapt_sqlite(sqlite)?;
    if !options.sql.is_empty() {
        return batch(&sqlite, options.sql.join(" "), &options);
    }
    stream_session(&sqlite, &options, io::stdin().is_terminal())
}

fn main() {
    if env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        println!("{}", usage());
        println!("Run without SQL for a REPL. Commands: .reset, .quit, .help");
        return;
    }
    match run() {
        Ok(status) => std::process::exit(status),
        Err(message) => {
            eprintln!("cboxes-sqlite: {message}\n{}", usage());
            std::process::exit(2);
        }
    }
}
