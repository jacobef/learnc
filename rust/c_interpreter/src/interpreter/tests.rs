mod aggregates;
mod arithmetic;
mod declarations;
mod execution;
mod library;
mod memory;
mod nonlocal_control;
mod preprocessing;
mod standard_examples;
mod stdio;

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    ProgramOutput, RunExpressionEvalRequest, RunOptions, UbDetectionMode, diag::Diagnostic,
    run_file as crate_run_file, run_files as crate_run_files,
    run_files_with_options as crate_run_files_with_options, run_source as crate_run_source,
    run_source_with_options as crate_run_source_with_options,
};

fn run_source(path: &str, source: &str) -> Result<ProgramOutput, Diagnostic> {
    let mut normalized = source.to_owned();
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    crate_run_source(path, &normalized)
}

fn run_source_with_options(
    path: &str,
    source: &str,
    options: &RunOptions,
) -> Result<ProgramOutput, Diagnostic> {
    let mut normalized = source.to_owned();
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    crate_run_source_with_options(path, &normalized, options)
}

fn assert_stdout(source: &str, expected: &str) {
    assert_eq!(run_source("test.c", source).unwrap().stdout, expected);
}

fn assert_exit_status(source: &str, expected: i32) {
    assert_eq!(run_source("test.c", source).unwrap().exit_status, expected);
}

fn rendered_diagnostic(source: &str) -> String {
    run_source("test.c", source).unwrap_err().render()
}

fn assert_diagnostic_contains(source: &str, expected: &str) {
    let rendered = rendered_diagnostic(source);
    assert!(rendered.contains(expected), "{rendered}");
}

fn simple_ub_options() -> RunOptions {
    RunOptions {
        ub_detection_mode: UbDetectionMode::Simple,
        ..RunOptions::default()
    }
}

fn normalize_test_file(path: &Path) {
    let mut text = fs::read_to_string(path).unwrap();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
        fs::write(path, text).unwrap();
    }
}

fn run_file(path: PathBuf) -> Result<ProgramOutput, Diagnostic> {
    normalize_test_file(&path);
    crate_run_file(path)
}

fn run_files<I>(paths: I) -> Result<ProgramOutput, Diagnostic>
where
    I: IntoIterator<Item = PathBuf>,
{
    let collected = paths.into_iter().collect::<Vec<_>>();
    for path in &collected {
        normalize_test_file(path);
    }
    crate_run_files(collected)
}

fn run_files_with_options<I>(paths: I, options: &RunOptions) -> Result<ProgramOutput, Diagnostic>
where
    I: IntoIterator<Item = PathBuf>,
{
    let collected = paths.into_iter().collect::<Vec<_>>();
    for path in &collected {
        normalize_test_file(path);
    }
    crate_run_files_with_options(collected, options)
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let root = temp_test_dir(label);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, path: &str, source: &str) -> std::io::Result<()> {
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }

    fn run<const N: usize>(&self, sources: [&str; N]) -> Result<ProgramOutput, Diagnostic> {
        run_files(sources.map(|source| self.root.join(source)))
    }

    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_user_code_library_diag(rendered: &str, expected: &str) {
    assert!(rendered.contains("test.c"), "{rendered}");
    assert!(rendered.contains(expected), "{rendered}");
    assert!(!rendered.contains("__codex"), "{rendered}");
}

fn state_address(result: &ProgramOutput, name: &str) -> u64 {
    result
        .state
        .iter()
        .find(|item| item.name == name)
        .and_then(|item| item.address)
        .unwrap_or_else(|| panic!("missing address for {name}"))
}

fn sqlite_varint_source() -> &'static str {
    // Extracted from SQLite src/util.c. SQLITE_NOINLINE is only an optimizer
    // annotation in SQLite, so it is intentionally omitted here.
    r#"
            #include <assert.h>
            #include <stdint.h>

            typedef unsigned char u8;
            typedef uint32_t u32;
            typedef uint64_t u64;

            static int putVarint64(unsigned char *p, u64 v){
              int i, j, n;
              u8 buf[10];
              if( v & (((u64)0xff000000)<<32) ){
                p[8] = (u8)v;
                v >>= 8;
                for(i=7; i>=0; i--){
                  p[i] = (u8)((v & 0x7f) | 0x80);
                  v >>= 7;
                }
                return 9;
              }
              n = 0;
              do{
                buf[n++] = (u8)((v & 0x7f) | 0x80);
                v >>= 7;
              }while( v!=0 );
              buf[0] &= 0x7f;
              assert( n<=9 );
              for(i=0, j=n-1; j>=0; j--, i++){
                p[i] = buf[j];
              }
              return n;
            }

            int sqlite3PutVarint(unsigned char *p, u64 v){
              if( v<=0x7f ){
                p[0] = v&0x7f;
                return 1;
              }
              if( v<=0x3fff ){
                p[0] = ((v>>7)&0x7f)|0x80;
                p[1] = v&0x7f;
                return 2;
              }
              return putVarint64(p,v);
            }

            #define SLOT_2_0     0x001fc07f
            #define SLOT_4_2_0   0xf01fc07f

            u8 sqlite3GetVarint(const unsigned char *p, u64 *v){
              u32 a,b,s;

              if( ((signed char*)p)[0]>=0 ){
                *v = *p;
                return 1;
              }
              if( ((signed char*)p)[1]>=0 ){
                *v = ((u32)(p[0]&0x7f)<<7) | p[1];
                return 2;
              }
              assert( SLOT_2_0 == ((0x7f<<14) | (0x7f)) );
              assert( SLOT_4_2_0 == ((0xfU<<28) | (0x7f<<14) | (0x7f)) );

              a = ((u32)p[0])<<14;
              b = p[1];
              p += 2;
              a |= *p;
              if (!(a&0x80)){
                a &= SLOT_2_0;
                b &= 0x7f;
                b = b<<7;
                a |= b;
                *v = a;
                return 3;
              }

              a &= SLOT_2_0;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                b &= SLOT_2_0;
                a = a<<7;
                a |= b;
                *v = a;
                return 4;
              }

              b &= SLOT_2_0;
              s = a;
              p++;
              a = a<<14;
              a |= *p;
              if (!(a&0x80)){
                b = b<<7;
                a |= b;
                s = s>>18;
                *v = ((u64)s)<<32 | a;
                return 5;
              }

              s = s<<7;
              s |= b;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                a &= SLOT_2_0;
                a = a<<7;
                a |= b;
                s = s>>18;
                *v = ((u64)s)<<32 | a;
                return 6;
              }

              p++;
              a = a<<14;
              a |= *p;
              if (!(a&0x80)){
                a &= SLOT_4_2_0;
                b &= SLOT_2_0;
                b = b<<7;
                a |= b;
                s = s>>11;
                *v = ((u64)s)<<32 | a;
                return 7;
              }

              a &= SLOT_2_0;
              p++;
              b = b<<14;
              b |= *p;
              if (!(b&0x80)){
                b &= SLOT_4_2_0;
                a = a<<7;
                a |= b;
                s = s>>4;
                *v = ((u64)s)<<32 | a;
                return 8;
              }

              p++;
              a = a<<15;
              a |= *p;
              b &= SLOT_2_0;
              b = b<<8;
              a |= b;
              s = s<<4;
              b = p[-4];
              b &= 0x7f;
              b = b>>3;
              s |= b;
              *v = ((u64)s)<<32 | a;
              return 9;
            }

            static int check(u64 input){
              unsigned char encoded[9] = {0};
              u64 decoded = 0;
              int written = sqlite3PutVarint(encoded, input);
              int read = sqlite3GetVarint(encoded, &decoded);
              return written==read && decoded==input;
            }

            int main(void){
              const u64 boundaries[] = {
                0, 1, 0x7f, 0x80, 0x3fff, 0x4000,
                0x1fffff, 0x200000, 0xfffffff, 0x10000000,
                0x7ffffffffULL, 0x800000000ULL,
                0x3ffffffffffULL, 0x40000000000ULL,
                0x1ffffffffffffULL, 0x2000000000000ULL,
                0xffffffffffffffULL, 0x100000000000000ULL,
                0x7fffffffffffffffULL, 0x8000000000000000ULL,
                0xffffffffffffffffULL
              };
              unsigned long i;
              for(i=0; i<sizeof(boundaries)/sizeof(boundaries[0]); i++){
                if( !check(boundaries[i]) ) return 1;
              }
              u64 value = 0x243f6a8885a308d3ULL;
              for(i=0; i<512; i++){
                value = value * 6364136223846793005ULL + 1442695040888963407ULL;
                if( !check(value) ) return 2;
              }
              return 0;
            }
        "#
}

fn temp_test_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("c_interpreter-{label}-{unique}"))
}
