use libc::{self, c_char, c_double, c_float, c_int, c_long, c_longlong, c_ulonglong, c_void};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind, set_hook, take_hook};
use std::rc::Rc;
use std::sync::Once;

use crate::ast::{
    BinaryOp, Block, BlockItem, Declaration, Designator, Expr, ExternalDeclaration, ForInit,
    FunctionDecl, FunctionDef, GenericAssociation, Initializer, Linkage, Parameter, PostfixOp,
    Statement, StorageClass, SwitchLabel, TranslationUnit, UnaryOp,
};
use crate::diag::{Diagnostic, DiagnosticRuntimeContext};
use crate::fast_hash::{FastHashMap as HashMap, FastHashSet as HashSet};
use crate::number::{NumberLiteral, NumberValue, parse_number_literal};
use crate::source::{FileId, SourceManager, Span};
use crate::token::StringLiteralValue;
use crate::types::{CType, HOST_LONG_DOUBLE_ALIGN, RecordMember, RecordType, TypeQualifiers};
use crate::{RunOptions, UbDetectionMode, composite_type};

const INT_MIN: i128 = i32::MIN as i128;
const INT_MAX: i128 = i32::MAX as i128;
const HOST_MATH_ERRHANDLING: c_int = 2;
const HOST_FE_INVALID: c_int = 0x0001;
const HOST_FE_DIVBYZERO: c_int = 0x0002;
const HOST_FE_OVERFLOW: c_int = 0x0004;
const HOST_FE_UNDERFLOW: c_int = 0x0008;
const HOST_FE_ALL_EXCEPT: c_int = 0x001f;
const HOST_FE_TONEAREST: c_int = 0x00000000;
#[cfg(target_os = "wasi")]
const HOST_FE_UPWARD: c_int = 0x00400000;
#[cfg(target_os = "wasi")]
const HOST_FE_DOWNWARD: c_int = 0x00800000;
#[cfg(target_os = "wasi")]
const HOST_FE_TOWARDZERO: c_int = 0x00C00000;
const HOST_FP_NAN: c_int = 1;
const HOST_FP_INFINITE: c_int = 2;
const HOST_FP_ZERO: c_int = 3;
const HOST_FP_NORMAL: c_int = 4;
const HOST_FP_SUBNORMAL: c_int = 5;
const HOST_EOF: c_int = -1;
const HOST_L_TMPNAM: usize = 32;
const HOST_BUFSIZ: usize = 1024;
const HOST_POINTER_SIZE: usize = 8;
const COMPACT_OBJECT_REPRESENTATION_THRESHOLD: usize = 1024 * 1024;
// The execution entry point provides a fixed-size evaluator stack in every build profile/target,
// so this interpreted-C limit is independent of the host compiler's optimization choices.
const MAX_FUNCTION_CALL_DEPTH: usize = 80;

fn byte_limit_description(bytes: usize) -> String {
    const MIB: usize = 1024 * 1024;
    const KIB: usize = 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB && bytes.is_multiple_of(KIB) {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

// These are properties of the interpreted machine, not aliases for the build
// target's C ABI.
type HostLongDouble = f64;
type InterpretedLong = i64;

// Route modeled-long and modeled-long-double operations through host functions
// with matching signatures. In particular, Wasm has 32-bit long and binary128
// long double, while the interpreter deliberately uses LP64 and binary64.
use self::{
    acos as acosl, acosh as acoshl, asin as asinl, asinh as asinhl, atan as atanl, atan2 as atan2l,
    atanh as atanhl, atoll as atol, cbrt as cbrtl, ceil as ceill, copysign as copysignl,
    cos as cosl, cosh as coshl, erf as erfl, erfc as erfcl, exp as expl, exp2 as exp2l,
    expm1 as expm1l, fabs as fabsl, fdim as fdiml, floor as floorl, fma as fmal, fmax as fmaxl,
    fmin as fminl, fmod as fmodl, frexp as frexpl, hypot as hypotl, ilogb as ilogbl,
    ldexp as ldexpl, lgamma as lgammal, llround as lround, llround as lroundl, llround as llroundl,
    llroundf as lroundf, log as logl, log1p as log1pl, log2 as log2l, log10 as log10l,
    logb as logbl, modf as modfl, nan as nanl, nextafter as nextafterl, nextafter as nexttoward,
    nextafter as nexttowardl, pow as powl, remainder as remainderl, remquo as remquol,
    round as roundl, scalbn as scalbnl, sin as sinl, sinh as sinhl, sqrt as sqrtl,
    strtod as strtold, strtoll as strtol, strtoull as strtoul, tan as tanl, tanh as tanhl,
    tgamma as tgammal, trunc as truncl, wcstod as wcstold, wcstoll as wcstol, wcstoull as wcstoul,
};

#[cfg(not(target_os = "wasi"))]
use self::{
    llrint as lrint, llrint as lrintl, llrint as llrintl, llrintf as lrintf,
    nearbyint as nearbyintl, rint as rintl,
};

unsafe extern "C" fn nexttowardf(x: c_float, y: HostLongDouble) -> c_float {
    if x.is_nan() || y.is_nan() {
        return x + y as c_float;
    }
    if f64::from(x) == y {
        return y as c_float;
    }
    // Rounding y to float first could erase the requested direction.
    let direction = if f64::from(x) < y {
        c_float::INFINITY
    } else {
        c_float::NEG_INFINITY
    };
    unsafe { nextafterf(x, direction) }
}

fn scalbln_exponent(exponent: InterpretedLong) -> c_int {
    exponent.clamp(c_int::MIN.into(), c_int::MAX.into()) as c_int
}

unsafe extern "C" fn modeled_scalbln(x: c_double, exponent: InterpretedLong) -> c_double {
    unsafe { scalbn(x, scalbln_exponent(exponent)) }
}

unsafe extern "C" fn modeled_scalblnf(x: c_float, exponent: InterpretedLong) -> c_float {
    unsafe { scalbnf(x, scalbln_exponent(exponent)) }
}

#[cfg(target_os = "wasi")]
fn host_system(command: *const c_char) -> c_int {
    if command.is_null() { 0 } else { -1 }
}

#[cfg(not(target_os = "wasi"))]
fn host_system(command: *const c_char) -> c_int {
    unsafe { libc::system(command) }
}

type FileScopedMap<T> = HashMap<FileId, HashMap<String, T>>;

#[repr(C, align(8))]
struct HostMbState {
    opaque: [u8; 128],
}

unsafe extern "C" {
    fn atof(nptr: *const c_char) -> c_double;
    fn atoi(nptr: *const c_char) -> c_int;
    fn atoll(nptr: *const c_char) -> c_longlong;
    fn mblen(s: *const c_char, n: libc::size_t) -> c_int;
    fn mbtowc(pwc: *mut libc::wchar_t, s: *const c_char, n: libc::size_t) -> c_int;
    fn wctomb(s: *mut c_char, wchar: libc::wchar_t) -> c_int;
    fn mbstowcs(pwcs: *mut libc::wchar_t, s: *const c_char, n: libc::size_t) -> libc::size_t;
    fn wcstombs(s: *mut c_char, pwcs: *const libc::wchar_t, n: libc::size_t) -> libc::size_t;
    fn btowc(c: c_int) -> c_int;
    fn wctob(c: c_int) -> c_int;
    fn mbsinit(ps: *const HostMbState) -> c_int;
    fn mbrlen(s: *const c_char, n: libc::size_t, ps: *mut HostMbState) -> libc::size_t;
    fn mbrtowc(
        pwc: *mut libc::wchar_t,
        s: *const c_char,
        n: libc::size_t,
        ps: *mut HostMbState,
    ) -> libc::size_t;
    fn wcrtomb(s: *mut c_char, wc: libc::wchar_t, ps: *mut HostMbState) -> libc::size_t;
    fn mbsrtowcs(
        dst: *mut libc::wchar_t,
        src: *mut *const c_char,
        len: libc::size_t,
        ps: *mut HostMbState,
    ) -> libc::size_t;
    fn wcsrtombs(
        dst: *mut c_char,
        src: *mut *const libc::wchar_t,
        len: libc::size_t,
        ps: *mut HostMbState,
    ) -> libc::size_t;
    fn wcstof(nptr: *const libc::wchar_t, endptr: *mut *mut libc::wchar_t) -> c_float;
    fn wcstod(nptr: *const libc::wchar_t, endptr: *mut *mut libc::wchar_t) -> f64;
    fn wcstoll(
        nptr: *const libc::wchar_t,
        endptr: *mut *mut libc::wchar_t,
        base: c_int,
    ) -> c_longlong;
    fn wcstoull(
        nptr: *const libc::wchar_t,
        endptr: *mut *mut libc::wchar_t,
        base: c_int,
    ) -> libc::c_ulonglong;
    fn wcscoll(s1: *const libc::wchar_t, s2: *const libc::wchar_t) -> c_int;
    fn wcsxfrm(s1: *mut libc::wchar_t, s2: *const libc::wchar_t, n: libc::size_t) -> libc::size_t;
    fn wcsftime(
        s: *mut libc::wchar_t,
        maxsize: libc::size_t,
        format: *const libc::wchar_t,
        timeptr: *const HostTm,
    ) -> libc::size_t;
    fn iswalnum(wc: c_int) -> c_int;
    fn iswalpha(wc: c_int) -> c_int;
    fn iswblank(wc: c_int) -> c_int;
    fn iswcntrl(wc: c_int) -> c_int;
    fn iswdigit(wc: c_int) -> c_int;
    fn iswgraph(wc: c_int) -> c_int;
    fn iswlower(wc: c_int) -> c_int;
    fn iswprint(wc: c_int) -> c_int;
    fn iswpunct(wc: c_int) -> c_int;
    fn iswspace(wc: c_int) -> c_int;
    fn iswupper(wc: c_int) -> c_int;
    fn iswxdigit(wc: c_int) -> c_int;
    fn iswctype(wc: c_int, desc: libc::c_uint) -> c_int;
    fn towlower(wc: c_int) -> c_int;
    fn towupper(wc: c_int) -> c_int;
    fn towctrans(wc: c_int, desc: c_int) -> c_int;
    fn wctrans(property: *const c_char) -> c_int;
    fn wctype(property: *const c_char) -> libc::c_uint;
    fn strtof(nptr: *const c_char, endptr: *mut *mut c_char) -> c_float;
    fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> f64;
    fn strtoll(nptr: *const c_char, endptr: *mut *mut c_char, base: c_int) -> c_longlong;
    fn strtoull(nptr: *const c_char, endptr: *mut *mut c_char, base: c_int) -> libc::c_ulonglong;
    fn div(numer: c_int, denom: c_int) -> HostDivResultInt;
    fn lldiv(numer: c_longlong, denom: c_longlong) -> HostDivResultLongLong;
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    fn localeconv() -> *mut HostLconv;
    fn clock() -> libc::clock_t;
    fn difftime(time1: libc::time_t, time0: libc::time_t) -> c_double;
    #[cfg(not(target_os = "wasi"))]
    fn mktime(timeptr: *mut HostTm) -> libc::time_t;
    fn asctime(timeptr: *const HostTm) -> *mut c_char;
    fn ctime(timer: *const libc::time_t) -> *mut c_char;
    fn gmtime(timer: *const libc::time_t) -> *mut HostTm;
    fn localtime(timer: *const libc::time_t) -> *mut HostTm;
    fn strftime(
        s: *mut c_char,
        maxsize: libc::size_t,
        format: *const c_char,
        timeptr: *const HostTm,
    ) -> libc::size_t;
    #[cfg(not(target_os = "wasi"))]
    fn feclearexcept(excepts: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fegetexceptflag(flagp: *mut HostFExcept, excepts: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn feraiseexcept(excepts: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fesetexceptflag(flagp: *const HostFExcept, excepts: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fetestexcept(excepts: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fegetround() -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fesetround(round: c_int) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fegetenv(envp: *mut HostFEnv) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn feholdexcept(envp: *mut HostFEnv) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn fesetenv(envp: *const HostFEnv) -> c_int;
    #[cfg(not(target_os = "wasi"))]
    fn feupdateenv(envp: *const HostFEnv) -> c_int;
    static mut signgam: c_int;

    fn acos(x: c_double) -> c_double;
    fn acosf(x: c_float) -> c_float;
    fn asin(x: c_double) -> c_double;
    fn asinf(x: c_float) -> c_float;
    fn atan(x: c_double) -> c_double;
    fn atanf(x: c_float) -> c_float;
    fn atan2(y: c_double, x: c_double) -> c_double;
    fn atan2f(y: c_float, x: c_float) -> c_float;
    fn cos(x: c_double) -> c_double;
    fn cosf(x: c_float) -> c_float;
    fn sin(x: c_double) -> c_double;
    fn sinf(x: c_float) -> c_float;
    fn tan(x: c_double) -> c_double;
    fn tanf(x: c_float) -> c_float;

    fn acosh(x: c_double) -> c_double;
    fn acoshf(x: c_float) -> c_float;
    fn asinh(x: c_double) -> c_double;
    fn asinhf(x: c_float) -> c_float;
    fn atanh(x: c_double) -> c_double;
    fn atanhf(x: c_float) -> c_float;
    fn cosh(x: c_double) -> c_double;
    fn coshf(x: c_float) -> c_float;
    fn sinh(x: c_double) -> c_double;
    fn sinhf(x: c_float) -> c_float;
    fn tanh(x: c_double) -> c_double;
    fn tanhf(x: c_float) -> c_float;

    fn exp(x: c_double) -> c_double;
    fn expf(x: c_float) -> c_float;
    fn frexp(x: c_double, exp: *mut c_int) -> c_double;
    fn frexpf(x: c_float, exp: *mut c_int) -> c_float;
    fn ldexp(x: c_double, exp: c_int) -> c_double;
    fn ldexpf(x: c_float, exp: c_int) -> c_float;
    fn log(x: c_double) -> c_double;
    fn logf(x: c_float) -> c_float;
    fn log10(x: c_double) -> c_double;
    fn log10f(x: c_float) -> c_float;
    fn modf(x: c_double, iptr: *mut c_double) -> c_double;
    fn modff(x: c_float, iptr: *mut c_float) -> c_float;
    fn exp2(x: c_double) -> c_double;
    fn exp2f(x: c_float) -> c_float;
    fn expm1(x: c_double) -> c_double;
    fn expm1f(x: c_float) -> c_float;
    fn ilogb(x: c_double) -> c_int;
    fn ilogbf(x: c_float) -> c_int;
    fn log1p(x: c_double) -> c_double;
    fn log1pf(x: c_float) -> c_float;
    fn log2(x: c_double) -> c_double;
    fn log2f(x: c_float) -> c_float;
    fn logb(x: c_double) -> c_double;
    fn logbf(x: c_float) -> c_float;
    fn scalbn(x: c_double, n: c_int) -> c_double;
    fn scalbnf(x: c_float, n: c_int) -> c_float;

    fn pow(x: c_double, y: c_double) -> c_double;
    fn powf(x: c_float, y: c_float) -> c_float;
    fn sqrt(x: c_double) -> c_double;
    fn sqrtf(x: c_float) -> c_float;
    fn cbrt(x: c_double) -> c_double;
    fn cbrtf(x: c_float) -> c_float;
    fn hypot(x: c_double, y: c_double) -> c_double;
    fn hypotf(x: c_float, y: c_float) -> c_float;

    fn erf(x: c_double) -> c_double;
    fn erff(x: c_float) -> c_float;
    fn erfc(x: c_double) -> c_double;
    fn erfcf(x: c_float) -> c_float;
    fn tgamma(x: c_double) -> c_double;
    fn tgammaf(x: c_float) -> c_float;
    fn lgamma(x: c_double) -> c_double;
    fn lgammaf(x: c_float) -> c_float;
    fn fabs(x: c_double) -> c_double;
    fn fabsf(x: c_float) -> c_float;

    fn ceil(x: c_double) -> c_double;
    fn ceilf(x: c_float) -> c_float;
    fn floor(x: c_double) -> c_double;
    fn floorf(x: c_float) -> c_float;
    fn fmod(x: c_double, y: c_double) -> c_double;
    fn fmodf(x: c_float, y: c_float) -> c_float;
    fn trunc(x: c_double) -> c_double;
    fn truncf(x: c_float) -> c_float;
    fn round(x: c_double) -> c_double;
    fn roundf(x: c_float) -> c_float;
    fn llround(x: c_double) -> c_longlong;
    fn llroundf(x: c_float) -> c_longlong;
    #[cfg(not(target_os = "wasi"))]
    fn rint(x: c_double) -> c_double;
    #[cfg(not(target_os = "wasi"))]
    fn rintf(x: c_float) -> c_float;
    #[cfg(not(target_os = "wasi"))]
    fn llrint(x: c_double) -> c_longlong;
    #[cfg(not(target_os = "wasi"))]
    fn llrintf(x: c_float) -> c_longlong;
    #[cfg(not(target_os = "wasi"))]
    fn nearbyint(x: c_double) -> c_double;
    #[cfg(not(target_os = "wasi"))]
    fn nearbyintf(x: c_float) -> c_float;
    fn remainder(x: c_double, y: c_double) -> c_double;
    fn remainderf(x: c_float, y: c_float) -> c_float;
    fn remquo(x: c_double, y: c_double, quo: *mut c_int) -> c_double;
    fn remquof(x: c_float, y: c_float, quo: *mut c_int) -> c_float;
    fn copysign(x: c_double, y: c_double) -> c_double;
    fn copysignf(x: c_float, y: c_float) -> c_float;
    fn nan(tagp: *const c_char) -> c_double;
    fn nanf(tagp: *const c_char) -> c_float;
    fn nextafter(x: c_double, y: c_double) -> c_double;
    fn nextafterf(x: c_float, y: c_float) -> c_float;
    fn fdim(x: c_double, y: c_double) -> c_double;
    fn fdimf(x: c_float, y: c_float) -> c_float;
    fn fmax(x: c_double, y: c_double) -> c_double;
    fn fmaxf(x: c_float, y: c_float) -> c_float;
    fn fmin(x: c_double, y: c_double) -> c_double;
    fn fminf(x: c_float, y: c_float) -> c_float;
    fn fma(x: c_double, y: c_double, z: c_double) -> c_double;
    fn fmaf(x: c_float, y: c_float, z: c_float) -> c_float;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn __error() -> *mut c_int;
    fn ___mb_cur_max() -> c_int;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    fn __errno_location() -> *mut c_int;
    fn __ctype_get_mb_cur_max() -> libc::size_t;
}

fn host_errno_ptr() -> *mut c_int {
    #[cfg(target_os = "macos")]
    unsafe {
        __error()
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        __errno_location()
    }
}

fn host_mb_cur_max() -> usize {
    #[cfg(target_os = "macos")]
    unsafe {
        ___mb_cur_max() as usize
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        __ctype_get_mb_cur_max() as usize
    }
}

pub struct ProgramOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_status: c_int,
    pub state: Vec<ProgramStateBox>,
    pub trace: Vec<ProgramTraceEvent>,
    pub main_close: ProgramSourceLocation,
    pub blocked: Option<ProgramBlocked>,
    pub execution_limit: Option<ProgramExecutionLimit>,
    pub expression: Option<ProgramExpressionResult>,
}

const CBOXES_STEP_LIMIT_TRACE_PREFIX_CAP: usize = 256;
const CBOXES_MAX_ARRAY_ELEMENTS: usize = 4096;
const CBOXES_FLOAT_DISPLAY_CACHE_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct ProgramSourceLocation {
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ProgramBlocked {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub function: String,
    pub state: Vec<ProgramStateBox>,
}

#[derive(Debug, Clone)]
pub struct ProgramExecutionLimit {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub trace_position: usize,
}

#[derive(Debug, Clone)]
pub struct ProgramStateBox {
    pub name: String,
    pub ty: String,
    pub value: String,
    pub display_value: String,
    pub exact_value: String,
    pub address: Option<u64>,
    pub array_root: Option<String>,
    pub array_shape: Vec<usize>,
    pub array_indices: Vec<usize>,
    pub aggregate_root: Option<String>,
    pub aggregate_path: Vec<String>,
    pub aggregate_kind: Option<String>,
    pub aliases: Vec<String>,
    pub type_info: ProgramTypeInfo,
}

#[derive(Debug, Clone)]
pub struct ProgramTypeInfo {
    pub kind: String,
    pub help: Option<String>,
    pub help_type_names: Vec<String>,
    pub help_tree: Option<ProgramTypeHelpNode>,
    pub pointer_depth: usize,
    pub array_shape: Vec<usize>,
    pub pointee_array_shape: Vec<usize>,
    pub size: Option<usize>,
    pub align: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ProgramTypeHelpNode {
    pub kind: String,
    pub label: String,
    pub type_name: Option<String>,
    pub children: Vec<ProgramTypeHelpChild>,
}

#[derive(Debug, Clone)]
pub struct ProgramTypeHelpChild {
    pub relation: String,
    pub node: Box<ProgramTypeHelpNode>,
}

#[derive(Debug, Clone)]
pub struct ProgramTraceEvent {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub kind: String,
    pub state: Vec<ProgramStateBox>,
    pub skipped_range: Option<ProgramSourceRange>,
}

#[derive(Debug, Clone)]
pub struct ProgramSourceRange {
    pub file: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone)]
pub struct ProgramExpressionEvalRequest {
    pub event_index: usize,
    pub expr: Expr,
    pub value_literal_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProgramValueLiteral {
    pub kind: String,
    pub has_suffix: bool,
}

#[derive(Debug, Clone)]
pub struct ProgramExpressionResult {
    pub kind: String,
    pub ty: String,
    pub value: String,
    pub address: Option<u64>,
    pub name: String,
    pub value_literal: Option<ProgramValueLiteral>,
    pub display_value: String,
    pub exact_value: String,
    pub type_info: ProgramTypeInfo,
}

pub struct Interpreter<'a> {
    sources: &'a SourceManager,
    program: TranslationUnit,
    run_options: RunOptions,
    functions: HashMap<String, Rc<FunctionDef>>,
    internal_functions: FileScopedMap<Rc<FunctionDef>>,
    declared_functions: HashMap<String, FunctionDecl>,
    internal_declared_functions: FileScopedMap<FunctionDecl>,
    function_symbols: HashMap<String, Rc<FunctionDef>>,
    function_may_setjmp: HashMap<String, bool>,
    declared_globals: HashMap<String, Declaration>,
    internal_declared_globals: FileScopedMap<Declaration>,
    function_visible_global_declarations: HashMap<String, HashMap<String, Declaration>>,
    global_bindings: HashMap<String, ObjectId>,
    internal_global_bindings: FileScopedMap<ObjectId>,
    local_static_bindings: HashMap<Span, ObjectId>,
    automatic_object_bindings: HashMap<(usize, Span), ObjectId>,
    compound_literal_bindings: HashMap<(usize, Span), ObjectId>,
    full_expression_temporaries: Vec<ObjectId>,
    next_object: usize,
    reusable_automatic_object_ids: Vec<ObjectId>,
    retired_objects: HashMap<ObjectId, ObjectState>,
    live_non_dynamic_bytes: usize,
    expr_state: ExprState,
    assignment_targets: Vec<AssignmentTarget>,
    initializer_sequencing: Option<InitializerSequencing>,
    restrict_trackers: Vec<RestrictTracker>,
    restrict_tracking_required: bool,
    formatted_io_accesses: Option<Vec<FormattedIoAccess>>,
    pending_stream_buffer_lifetime_ub: Option<Diagnostic>,
    next_encoded_pointer: u64,
    object_type_registry: HashMap<ObjectId, CType>,
    object_base_addresses: HashMap<ObjectId, u64>,
    dynamic_allocations: HashSet<ObjectId>,
    dynamic_bytes_allocated: usize,
    virtual_filesystem: VirtualFileSystem,
    host_streams: HashMap<ObjectId, HostStream>,
    configured_stream_buffers: usize,
    host_stdio_bindings: HashMap<String, ObjectId>,
    host_capture_streams: HashMap<CaptureStream, ObjectId>,
    errno_binding: Option<ObjectId>,
    signgam_binding: Option<ObjectId>,
    tmpnam_binding: Option<ObjectId>,
    strerror_binding: Option<ObjectId>,
    time_text_binding: Option<ObjectId>,
    getenv_binding: Option<ObjectId>,
    locale_generation: u64,
    rand_state: u32,
    signal_handlers: HashMap<i32, SignalHandlerState>,
    fe_dfl_env_binding: Option<ObjectId>,
    startup_fenv: [u8; 16],
    #[cfg(target_os = "wasi")]
    modeled_fenv: RefCell<ModeledFloatingEnvironment>,
    fexcept_provenance: HashMap<(ObjectId, usize), (u64, i128)>,
    fenv_provenance: HashMap<(ObjectId, usize), u64>,
    mbstate_provenance: HashMap<(ObjectId, usize), (u64, u64)>,
    internal_mbstate_generations: HashMap<String, u64>,
    wcstok_state_provenance: HashMap<(ObjectId, usize), (u64, PointerValue)>,
    fpos_provenance: HashMap<(ObjectId, usize), (u64, StreamBacking)>,
    ftell_provenance: HashSet<(StreamBacking, i128)>,
    pending_qsort_order: Option<Vec<usize>>,
    pending_bsearch_result: Option<Option<usize>>,
    pending_heap_call: Option<PendingHeapCall>,
    wctrans_descriptors: HashMap<i128, u64>,
    wctype_descriptors: HashMap<i128, u64>,
    atexit_handlers: Vec<String>,
    running_atexit: bool,
    quick_exit_handlers: Vec<String>,
    running_quick_exit: bool,
    func_name_bindings: HashMap<usize, ObjectId>,
    mbrtoc16_pending: HashMap<PointerValue, u16>,
    mbrtoc16_null_pending: Option<u16>,
    c16rtomb_pending: HashMap<PointerValue, u16>,
    c16rtomb_null_pending: Option<u16>,
    encoded_object_pointers: HashMap<PointerValue, u64>,
    encoded_function_pointers: HashMap<String, u64>,
    decoded_pointers: HashMap<u64, Vec<EncodedPointer>>,
    opaque_integer_pointer_addresses: RefCell<HashSet<u64>>,
    current_variadic_args: Vec<Vec<TypedValue>>,
    va_lists: HashMap<u64, VaListCursor>,
    next_va_list_handle: u64,
    current_functions: Vec<Rc<FunctionDef>>,
    current_frame_ids: Vec<usize>,
    active_block_scopes: Vec<ActiveBlockScope>,
    next_frame_id: usize,
    setjmp_envs: HashMap<u64, SetjmpEnvironment>,
    setjmp_provenance: HashMap<(ObjectId, usize), (u64, Vec<ByteCell>)>,
    live_setjmp_frames: HashSet<usize>,
    next_setjmp_handle: u64,
    active_setjmp_contexts: Vec<ActiveSetjmpContext>,
    expr_setjmp_cache: HashMap<usize, bool>,
    expr_side_effect_cache: HashMap<usize, bool>,
    switch_dispatch_cache: HashMap<(usize, CType), SwitchDispatch>,
    switch_entry_index_cache: HashMap<(usize, Span), Option<usize>>,
    type_contains_union_cache: HashMap<CType, bool>,
    variable_cache: HashMap<usize, VariableCacheEntry>,
    member_access_cache: RefCell<MemberAccessCache>,
    float_display_cache: RefCell<HashMap<u64, (String, String)>>,
    pending_longjmp_return: Option<PendingLongjmpReturn>,
    host_library_runtime_depth: usize,
    active_switch_dispatch_depth: usize,
    strtok_state: Option<PointerValue>,
    strtok_started: bool,
    cboxes_main_state: Vec<ProgramStateBox>,
    cboxes_trace: Vec<ProgramTraceEvent>,
    cboxes_expression_request: Option<ProgramExpressionEvalRequest>,
    cboxes_expression_result: Option<ProgramExpressionResult>,
    execution_steps_remaining: Option<usize>,
    execution_steps_completed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ObjectId(usize);

#[derive(Debug, Clone)]
struct Frame {
    id: usize,
    bindings: HashMap<String, ObjectId>,
    object_decls: HashMap<String, Declaration>,
    function_decls: HashMap<String, FunctionDecl>,
}

#[derive(Debug, Clone)]
struct VariableCacheEntry {
    frame_id: Option<usize>,
    value: ValueCategory,
}

#[derive(Debug, Clone)]
struct SwitchDispatch {
    cases: Vec<(i128, Span)>,
    default: Option<Span>,
}

#[derive(Clone, Copy)]
struct ConstraintContext<'a> {
    return_type: &'a CType,
    return_type_span: Span,
    loop_depth: usize,
    switch_depth: usize,
}

#[derive(Default)]
struct JumpScopeValidation {
    labels: HashMap<String, (HashSet<Span>, Span)>,
    gotos: Vec<(String, HashSet<Span>, Span)>,
    switches: Vec<SwitchScopeValidation>,
}

struct SwitchScopeValidation {
    source_scope: HashSet<Span>,
    label_scopes: Vec<(HashSet<Span>, Span)>,
}

struct CallFrameCleanup<'a> {
    interpreter: *mut Interpreter<'a>,
    objects: *mut ObjectFrames,
    frame_id: usize,
}

impl<'a> CallFrameCleanup<'a> {
    fn new(interpreter: &mut Interpreter<'a>, objects: &mut ObjectFrames, frame_id: usize) -> Self {
        Self {
            interpreter,
            objects,
            frame_id,
        }
    }
}

impl Drop for CallFrameCleanup<'_> {
    fn drop(&mut self) {
        unsafe {
            (*self.interpreter).cleanup_call_frame(self.frame_id, &mut *self.objects);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageDuration {
    Automatic,
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy)]
enum TrackedQualifier {
    Const,
    Volatile,
}

#[derive(Debug, Clone)]
struct ObjectState {
    ty: CType,
    storage_duration: StorageDuration,
    alive: bool,
    readonly: bool,
    const_object: bool,
    has_const_subobject: bool,
    has_volatile_subobject: bool,
    register_object: bool,
    address_taken: bool,
    initialized: bool,
    indeterminate_reason: Option<&'static str>,
    byte_size: usize,
    value: StoredValue,
    declaration_span: Span,
    modification_count: u64,
    raw_indeterminate_bytes: Option<(u64, usize)>,
    variably_modified: bool,
    effective_types: Vec<EffectiveTypeRegion>,
    pointer_slots: BTreeMap<(usize, usize), StoredPointerValue>,
}

#[derive(Debug, Clone)]
struct EffectiveTypeRegion {
    start: usize,
    size: usize,
    ty: CType,
    coalesced_element_type: Option<CType>,
}

#[derive(Debug, Clone, Copy)]
enum PendingHeapCall {
    Malloc(usize),
    Calloc(Option<usize>),
    Realloc {
        object: Option<ObjectId>,
        size: usize,
    },
    Free(Option<ObjectId>),
}

#[derive(Debug, Clone)]
struct VaListCursor {
    args: Vec<TypedValue>,
    index: usize,
    owner_frame_id: usize,
}

#[derive(Debug)]
struct VirtualFileSystem {
    directories: HashSet<String>,
    paths: HashMap<String, u64>,
    files: HashMap<u64, Vec<u8>>,
    next_file_id: u64,
    next_temp_id: u64,
}

impl VirtualFileSystem {
    fn new() -> Self {
        let mut directories = HashSet::default();
        directories.insert("/".to_owned());
        directories.insert("/tmp".to_owned());
        Self {
            directories,
            paths: HashMap::default(),
            files: HashMap::default(),
            next_file_id: 1,
            next_temp_id: 1,
        }
    }

    fn normalize_path(path: &[u8]) -> Result<String, c_int> {
        let text = std::str::from_utf8(path).map_err(|_| libc::EINVAL)?;
        if text.is_empty() {
            return Err(libc::ENOENT);
        }
        let mut components = Vec::new();
        for component in text.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(libc::EINVAL);
                    }
                }
                value => components.push(value),
            }
        }
        Ok(if components.is_empty() {
            "/".to_owned()
        } else {
            format!("/{}", components.join("/"))
        })
    }

    fn parent_path(path: &str) -> &str {
        match path.rfind('/') {
            Some(0) | None => "/",
            Some(index) => &path[..index],
        }
    }

    fn create_parent_directories(&mut self, path: &str) -> Result<(), c_int> {
        let parent = Self::parent_path(path);
        if parent == "/" {
            return Ok(());
        }
        let mut current = String::new();
        let mut missing = Vec::new();
        for component in parent.trim_start_matches('/').split('/') {
            current.push('/');
            current.push_str(component);
            if self.paths.contains_key(&current) {
                return Err(libc::ENOTDIR);
            }
            if !self.directories.contains(&current) {
                missing.push(current.clone());
            }
        }
        for directory in missing {
            self.directories.insert(directory);
        }
        Ok(())
    }

    fn create_file(&mut self, path: Option<&str>) -> u64 {
        let id = self.next_file_id;
        self.next_file_id = self.next_file_id.saturating_add(1);
        self.files.insert(id, Vec::new());
        if let Some(path) = path {
            self.paths.insert(path.to_owned(), id);
        }
        id
    }

    fn open_file(&mut self, path: &str, mode: ParsedFopenMode) -> Result<(u64, usize), c_int> {
        if self.directories.contains(path) {
            return Err(libc::EISDIR);
        }
        let existing = self.paths.get(path).copied();
        if mode.exclusive && existing.is_some() {
            return Err(libc::EEXIST);
        }
        let file_id = match mode.opening {
            FopenOpening::Read => existing.ok_or(libc::ENOENT)?,
            FopenOpening::Write => {
                self.create_parent_directories(path)?;
                existing.unwrap_or_else(|| self.create_file(Some(path)))
            }
            FopenOpening::Append => {
                self.create_parent_directories(path)?;
                existing.unwrap_or_else(|| self.create_file(Some(path)))
            }
        };
        if mode.opening == FopenOpening::Write {
            self.files.entry(file_id).or_default().clear();
        }
        let position = if mode.opening == FopenOpening::Append {
            self.files.get(&file_id).map_or(0, Vec::len)
        } else {
            0
        };
        Ok((file_id, position))
    }

    fn remove(&mut self, path: &str) -> Result<(), c_int> {
        if self.paths.remove(path).is_some() {
            return Ok(());
        }
        if path == "/" || !self.directories.contains(path) {
            return Err(libc::ENOENT);
        }
        let child_prefix = format!("{path}/");
        if self
            .directories
            .iter()
            .any(|entry| entry.starts_with(&child_prefix))
            || self
                .paths
                .keys()
                .any(|entry| entry.starts_with(&child_prefix))
        {
            return Err(libc::ENOTEMPTY);
        }
        self.directories.remove(path);
        Ok(())
    }

    fn rename(&mut self, old_path: &str, new_path: &str) -> Result<(), c_int> {
        if old_path == "/" || new_path == "/" {
            return Err(libc::EINVAL);
        }
        if let Some(file_id) = self.paths.remove(old_path) {
            if new_path.starts_with(&format!("{old_path}/")) {
                self.paths.insert(old_path.to_owned(), file_id);
                return Err(libc::ENOTDIR);
            }
            if self.directories.contains(new_path) {
                self.paths.insert(old_path.to_owned(), file_id);
                return Err(libc::EISDIR);
            }
            if let Err(errno) = self.create_parent_directories(new_path) {
                self.paths.insert(old_path.to_owned(), file_id);
                return Err(errno);
            }
            self.paths.remove(new_path);
            self.paths.insert(new_path.to_owned(), file_id);
            return Ok(());
        }
        if !self.directories.contains(old_path) {
            return Err(libc::ENOENT);
        }
        if new_path.starts_with(&format!("{old_path}/")) {
            return Err(libc::EINVAL);
        }
        if self.paths.contains_key(new_path) {
            return Err(libc::ENOTDIR);
        }
        if self.directories.contains(new_path) {
            return Err(libc::EEXIST);
        }
        self.create_parent_directories(new_path)?;
        let old_prefix = format!("{old_path}/");
        let moved_directories = self
            .directories
            .iter()
            .filter(|entry| *entry == old_path || entry.starts_with(&old_prefix))
            .cloned()
            .collect::<Vec<_>>();
        let moved_files = self
            .paths
            .iter()
            .filter(|(entry, _)| entry.starts_with(&old_prefix))
            .map(|(entry, id)| (entry.clone(), *id))
            .collect::<Vec<_>>();
        for entry in &moved_directories {
            self.directories.remove(entry);
        }
        for (entry, _) in &moved_files {
            self.paths.remove(entry);
        }
        for entry in moved_directories {
            self.directories
                .insert(format!("{new_path}{}", &entry[old_path.len()..]));
        }
        for (entry, id) in moved_files {
            self.paths
                .insert(format!("{new_path}{}", &entry[old_path.len()..]), id);
        }
        Ok(())
    }

    fn temporary_name(&mut self) -> String {
        loop {
            let path = format!("/tmp/cboxes-{}.tmp", self.next_temp_id);
            self.next_temp_id = self.next_temp_id.saturating_add(1);
            if !self.paths.contains_key(&path) {
                return path;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StreamBacking {
    Capture(CaptureStream),
    File { file_id: u64 },
}

#[derive(Debug)]
struct HostStream {
    backing: StreamBacking,
    open: bool,
    captured: Vec<u8>,
    position: usize,
    append: bool,
    pushback: Vec<u8>,
    orientation: i8,
    eof: bool,
    error: bool,
    mode: HostStreamMode,
    last_operation: Option<StreamLastOperation>,
    last_input_hit_eof: bool,
    input_closed: bool,
    operation_performed: bool,
    buffer: Option<StreamBufferConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostStreamModeKind {
    Input,
    Output,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostStreamMode {
    kind: HostStreamModeKind,
    binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamLastOperation {
    Input,
    Output,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct StreamBufferConfig {
    pointer: Option<PointerValue>,
    mode: c_int,
    size: usize,
    supplied_at: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedFopenMode {
    kind: HostStreamModeKind,
    binary: bool,
    opening: FopenOpening,
    exclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FopenOpening {
    Read,
    Write,
    Append,
}

struct DepthGuard {
    depth: *mut usize,
}

impl DepthGuard {
    fn enter(depth: &mut usize) -> Self {
        *depth = depth.saturating_add(1);
        Self { depth }
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        unsafe {
            *self.depth = (*self.depth).saturating_sub(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CaptureStream {
    Stdout,
    Stderr,
    Stdin,
}

#[derive(Debug, Clone)]
enum ScanfDirective {
    Whitespace,
    Literal(libc::wchar_t),
    Percent,
    Conversion(ScanfConversion),
}

#[derive(Default)]
struct ScanProgress {
    assignments: i32,
    converted: bool,
}

impl ScanProgress {
    fn input_failure_result(&self) -> i32 {
        if self.converted { self.assignments } else { -1 }
    }

    fn conversion_result(&mut self, status: ScanConversionStatus) -> Option<i32> {
        match status {
            ScanConversionStatus::Assigned => {
                self.assignments += 1;
                self.converted = true;
                None
            }
            ScanConversionStatus::NoAssignment => {
                self.converted = true;
                None
            }
            ScanConversionStatus::MatchingFailure => Some(self.assignments),
            ScanConversionStatus::InputFailure => Some(self.input_failure_result()),
        }
    }
}

#[derive(Debug, Clone)]
struct ScanfConversion {
    suppress: bool,
    width: Option<usize>,
    length: ScanfLength,
    spec: char,
    scanset: Option<Scanset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanfLength {
    None,
    Hh,
    H,
    L,
    Ll,
    J,
    Z,
    T,
    BigL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrintfLength {
    None,
    Hh,
    H,
    L,
    Ll,
    J,
    Z,
    T,
    BigL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrintfCount {
    Literal(i32),
    FromArg,
}

#[derive(Debug, Clone)]
struct PrintfConversion {
    flags: String,
    width: Option<PrintfCount>,
    precision: Option<PrintfCount>,
    length: PrintfLength,
    spec: char,
}

#[derive(Debug, Clone, Copy)]
struct PrintfArgRef<'a> {
    value: &'a TypedValue,
    span: Span,
}

#[derive(Debug, Clone)]
struct Scanset {
    invert: bool,
    items: Vec<ScansetItem>,
}

#[derive(Debug, Clone)]
enum ScansetItem {
    Single(libc::wchar_t),
    Range(libc::wchar_t, libc::wchar_t),
}

#[derive(Debug, Clone)]
enum WideScanfSource {
    Stream {
        stream_object: ObjectId,
        function_name: &'static str,
        standard: &'static str,
        pushback: Vec<libc::wchar_t>,
        consumed: usize,
    },
    Buffer {
        units: Vec<libc::wchar_t>,
        index: usize,
        consumed: usize,
    },
}

#[derive(Debug, Clone)]
enum ByteScanfSource {
    Stream {
        stream_object: ObjectId,
        function_name: &'static str,
        standard: &'static str,
        pushback: Vec<u8>,
        consumed: usize,
    },
    Buffer {
        bytes: Vec<u8>,
        index: usize,
        consumed: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum ScanConversionStatus {
    Assigned,
    NoAssignment,
    MatchingFailure,
    InputFailure,
}

type StoredMemberName = Rc<str>;

#[derive(Debug, Clone)]
enum StoredValue {
    Scalar(TypedValue),
    Array(Vec<StoredValue>),
    ObjectRepresentation(Vec<ByteCell>),
    Record(Vec<(StoredMemberName, StoredValue)>),
    Union {
        active_member: Option<StoredMemberName>,
        members: Vec<(StoredMemberName, StoredValue)>,
        bytes: Vec<ByteCell>,
    },
    Indeterminate,
}

#[derive(Debug)]
enum StoredValueView<'a> {
    Borrowed(&'a StoredValue),
    Owned(StoredValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteCell {
    Known(u8),
    Indeterminate,
}

#[derive(Debug, Clone)]
struct TypedValue {
    ty: CType,
    data: ValueData,
    restrict_source: Option<Rc<RestrictSource>>,
    indeterminate: bool,
    missing_return: bool,
}

#[derive(Debug, Clone)]
enum ValueData {
    Void,
    Int(CIntValue),
    // All real floating types currently share an f64-backed runtime value.
    Float(f64),
    Complex(ComplexValue),
    Pointer(Rc<PointerValue>),
    Function(Rc<str>),
    Aggregate(Box<StoredValue>),
    ObjectRepresentation(Box<Vec<ByteCell>>),
}

#[derive(Debug, Clone)]
enum StoredPointerValue {
    Object(PointerValue),
    Function(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ComplexValue {
    real: f64,
    imag: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CIntValue {
    low: u64,
    high: i64,
}

impl CIntValue {
    fn new(value: i128) -> Self {
        Self {
            low: value as u64,
            high: (value >> 64) as i64,
        }
    }

    fn get(self) -> i128 {
        ((self.high as i128) << 64) | self.low as i128
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PointerValue {
    object: Option<ObjectId>,
    base_offset: isize,
    offset: isize,
    member_path: Rc<Vec<String>>,
    designated_root_ty: Option<Rc<CType>>,
    byte_offset_override: Option<usize>,
    arithmetic_domain_start: Option<usize>,
    object_representation_domain: Option<ObjectRepresentationDomain>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ByteDomain {
    start: usize,
    one_past_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ObjectRepresentationDomain {
    /// Enforced while the pointer has character-pointer type.
    Active(ByteDomain),
    /// Retained through void* so a later conversion to character pointer can
    /// recover the bounds without constraining byte-counted void* APIs.
    Latent(ByteDomain),
}

impl ObjectRepresentationDomain {
    fn byte_domain(self) -> ByteDomain {
        match self {
            Self::Active(domain) | Self::Latent(domain) => domain,
        }
    }

    fn active(self) -> Option<ByteDomain> {
        match self {
            Self::Active(domain) => Some(domain),
            Self::Latent(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EncodedPointer {
    Object {
        pointer: PointerValue,
        pointee_ty: Option<CType>,
    },
    Function(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ComparablePointer {
    Object(u64),
    Function(String),
    Null,
}

#[derive(Debug, Clone)]
enum ValueCategory {
    RValue(TypedValue),
    LValue(LValue),
}

#[derive(Debug, Clone)]
struct LValue {
    object: ObjectId,
    ty: CType,
    base_offset: isize,
    offset: isize,
    member_path: Rc<Vec<String>>,
    designated_root_ty: Option<Rc<CType>>,
    byte_offset_override: Option<usize>,
    arithmetic_domain_start: Option<usize>,
    object_representation_domain: Option<ObjectRepresentationDomain>,
    bit_field_width: Option<u8>,
    restrict_source: Option<Rc<RestrictSource>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RestrictSource {
    object: ObjectId,
    base_offset: isize,
    offset: isize,
    member_path: Rc<Vec<String>>,
    pointed_to_const: bool,
}

#[derive(Debug, Default, Clone)]
struct RestrictTracker {
    accesses: HashMap<ObjectId, Vec<RestrictAccess>>,
    unrestricted_accesses: HashMap<ObjectId, HashMap<(usize, usize), RestrictAccess>>,
    source_objects: HashSet<ObjectId>,
}

#[derive(Debug, Clone)]
struct RestrictAccess {
    source: Option<Rc<RestrictSource>>,
    start: usize,
    size: usize,
    saw_write: bool,
    span: Span,
}

#[derive(Clone)]
struct ResolvedMemberAccess {
    path: Rc<Vec<String>>,
    ty: CType,
    bit_field_width: Option<u8>,
}

type MemberAccessCache = HashMap<(usize, TypeQualifiers), HashMap<String, ResolvedMemberAccess>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormattedIoAccessRole {
    Format,
    StringArgument,
    PercentN,
}

#[derive(Debug, Clone)]
struct FormattedIoAccess {
    object: ObjectId,
    start: usize,
    size: usize,
    role: FormattedIoAccessRole,
    span: Span,
}

#[derive(Debug)]
enum Flow {
    Continue,
    Goto(String, Span),
    LoopContinue,
    LoopBreak,
    Return(TypedValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetjmpContextKind {
    ExpressionStatement,
    IfCondition,
    WhileCondition,
    DoWhileCondition,
    ForCondition,
    SwitchExpression,
}

#[derive(Debug, Clone)]
struct ActiveSetjmpContext {
    frame_id: usize,
    root_expr: Expr,
    stmt_span: Span,
    kind: SetjmpContextKind,
}

#[derive(Debug, Clone)]
struct ActiveBlockScope {
    frame_id: usize,
    block_span: Span,
    saved_bindings: Option<HashMap<String, ObjectId>>,
    saved_object_decls: Option<HashMap<String, Declaration>>,
    saved_function_decls: Option<HashMap<String, FunctionDecl>>,
    existing_objects: Option<Vec<ObjectId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetjmpSite {
    stmt_span: Span,
    kind: SetjmpContextKind,
}

#[derive(Debug, Clone)]
struct SetjmpEnvironment {
    frame_id: usize,
    site: SetjmpSite,
    bindings: HashMap<String, ObjectId>,
    object_decls: HashMap<String, Declaration>,
    function_decls: HashMap<String, FunctionDecl>,
    object_snapshots: HashMap<ObjectId, ObjectState>,
    object_versions: HashMap<ObjectId, u64>,
    variably_modified_object_ids: Vec<ObjectId>,
    block_scopes: Vec<ActiveBlockScope>,
}

#[derive(Debug, Clone, Copy)]
struct PendingLongjmpReturn {
    frame_id: usize,
    value: i128,
}

#[derive(Debug, Clone, Copy)]
struct TerminationSignal {
    status: c_int,
    run_atexit: bool,
    run_quick_exit: bool,
}

#[derive(Debug, Clone, Copy)]
struct LongjmpSignal {
    handle: u64,
    target_frame_id: usize,
    value: i128,
}

fn install_control_flow_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = take_hook();
        set_hook(Box::new(move |info| {
            if info.payload().is::<LongjmpSignal>() || info.payload().is::<TerminationSignal>() {
                return;
            }
            previous(info);
        }));
    });
}

#[derive(Debug, Clone)]
enum InitSelector {
    Member(String),
    Index(usize),
}

#[derive(Debug, Clone)]
struct ExprState {
    accesses: HashMap<AccessRegion, ObjectAccess>,
    track_unsequenced_accesses: bool,
}

impl Default for ExprState {
    fn default() -> Self {
        Self {
            accesses: HashMap::default(),
            track_unsequenced_accesses: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AccessRegion {
    object: ObjectId,
    bit_start: usize,
    bit_size: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ObjectAccess {
    self_read: Option<Span>,
    other_read: Option<Span>,
    write: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentTargetKind {
    Simple,
    Compound,
    Increment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AssignmentTarget {
    region: AccessRegion,
    kind: AssignmentTargetKind,
}

#[derive(Debug, Clone)]
struct SequencingSnapshot {
    accesses: HashMap<AccessRegion, ObjectAccess>,
    assignment_targets: Vec<AssignmentTarget>,
}

#[derive(Debug, Default)]
struct InitializerSequencing {
    footprint: HashMap<AccessRegion, ObjectAccess>,
}

#[derive(Clone)]
struct ObjectFrames {
    frames: Vec<Vec<ObjectId>>,
    states: Vec<Option<ObjectState>>,
}

impl ObjectFrames {
    fn new() -> Self {
        Self {
            frames: vec![Vec::new()],
            states: Vec::new(),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.frames.len()
    }

    fn push_frame(&mut self) {
        self.frames.push(Vec::new());
    }

    fn insert(&mut self, frame_index: usize, id: ObjectId, state: ObjectState) {
        if self.states.len() <= id.0 {
            self.states.resize_with(id.0 + 1, || None);
        }
        debug_assert!(self.states[id.0].is_none());
        self.states[id.0] = Some(state);
        self.frames[frame_index].push(id);
    }

    fn insert_last(&mut self, id: ObjectId, state: ObjectState) {
        let frame_index = self.frames.len() - 1;
        self.insert(frame_index, id, state);
    }

    #[inline]
    fn get(&self, id: ObjectId) -> Option<&ObjectState> {
        self.states.get(id.0)?.as_ref()
    }

    #[inline]
    fn get_mut(&mut self, id: ObjectId) -> Option<&mut ObjectState> {
        self.states.get_mut(id.0)?.as_mut()
    }

    fn remove(&mut self, id: ObjectId) -> Option<ObjectState> {
        let state = self.states.get_mut(id.0)?.take()?;
        for frame in &mut self.frames {
            if let Some(index) = frame.iter().position(|candidate| *candidate == id) {
                frame.remove(index);
                break;
            }
        }
        Some(state)
    }

    fn remove_from_last(&mut self, id: ObjectId) -> Option<ObjectState> {
        let frame = self.frames.last_mut()?;
        let index = frame.iter().position(|candidate| *candidate == id)?;
        frame.remove(index);
        self.states.get_mut(id.0)?.take()
    }

    fn last_ids(&self) -> Option<&[ObjectId]> {
        self.frames.last().map(Vec::as_slice)
    }

    fn contains_in_last(&self, id: ObjectId) -> bool {
        self.frames.last().is_some_and(|frame| frame.contains(&id))
    }

    fn pop_frame(&mut self) -> Option<Vec<(ObjectId, ObjectState)>> {
        let ids = self.frames.pop()?;
        Some(
            ids.into_iter()
                .filter_map(|id| self.states[id.0].take().map(|state| (id, state)))
                .collect(),
        )
    }
}
type HostUnaryF64Fn = unsafe extern "C" fn(c_double) -> c_double;
type HostUnaryF32Fn = unsafe extern "C" fn(c_float) -> c_float;
type HostUnaryLongDoubleFn = unsafe extern "C" fn(HostLongDouble) -> HostLongDouble;
type HostBinaryF64Fn = unsafe extern "C" fn(c_double, c_double) -> c_double;
type HostBinaryF32Fn = unsafe extern "C" fn(c_float, c_float) -> c_float;
type HostBinaryLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, HostLongDouble) -> HostLongDouble;
type HostTernaryF64Fn = unsafe extern "C" fn(c_double, c_double, c_double) -> c_double;
type HostTernaryF32Fn = unsafe extern "C" fn(c_float, c_float, c_float) -> c_float;
type HostTernaryLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, HostLongDouble, HostLongDouble) -> HostLongDouble;
type HostFrexpF64Fn = unsafe extern "C" fn(c_double, *mut c_int) -> c_double;
type HostFrexpF32Fn = unsafe extern "C" fn(c_float, *mut c_int) -> c_float;
type HostFrexpLongDoubleFn = unsafe extern "C" fn(HostLongDouble, *mut c_int) -> HostLongDouble;
type HostModfF64Fn = unsafe extern "C" fn(c_double, *mut c_double) -> c_double;
type HostModfF32Fn = unsafe extern "C" fn(c_float, *mut c_float) -> c_float;
type HostModfLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, *mut HostLongDouble) -> HostLongDouble;
type HostRemquoF64Fn = unsafe extern "C" fn(c_double, c_double, *mut c_int) -> c_double;
type HostRemquoF32Fn = unsafe extern "C" fn(c_float, c_float, *mut c_int) -> c_float;
type HostRemquoLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, HostLongDouble, *mut c_int) -> HostLongDouble;
type HostUnaryIntF64Fn = unsafe extern "C" fn(c_double) -> c_int;
type HostUnaryIntF32Fn = unsafe extern "C" fn(c_float) -> c_int;
type HostUnaryIntLongDoubleFn = unsafe extern "C" fn(HostLongDouble) -> c_int;
type HostUnaryLongF64Fn = unsafe extern "C" fn(c_double) -> InterpretedLong;
type HostUnaryLongF32Fn = unsafe extern "C" fn(c_float) -> InterpretedLong;
type HostUnaryLongLongDoubleFn = unsafe extern "C" fn(HostLongDouble) -> InterpretedLong;
type HostUnaryLongLongF64Fn = unsafe extern "C" fn(c_double) -> c_longlong;
type HostUnaryLongLongF32Fn = unsafe extern "C" fn(c_float) -> c_longlong;
type HostUnaryLongLongLongDoubleFn = unsafe extern "C" fn(HostLongDouble) -> c_longlong;
type HostRealIntF64Fn = unsafe extern "C" fn(c_double, c_int) -> c_double;
type HostRealIntF32Fn = unsafe extern "C" fn(c_float, c_int) -> c_float;
type HostRealIntLongDoubleFn = unsafe extern "C" fn(HostLongDouble, c_int) -> HostLongDouble;
type HostRealLongF64Fn = unsafe extern "C" fn(c_double, InterpretedLong) -> c_double;
type HostRealLongF32Fn = unsafe extern "C" fn(c_float, InterpretedLong) -> c_float;
type HostRealLongLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, InterpretedLong) -> HostLongDouble;
type HostRealLongDoubleF64Fn = unsafe extern "C" fn(c_double, HostLongDouble) -> c_double;
type HostRealLongDoubleF32Fn = unsafe extern "C" fn(c_float, HostLongDouble) -> c_float;
type HostRealLongDoubleLongDoubleFn =
    unsafe extern "C" fn(HostLongDouble, HostLongDouble) -> HostLongDouble;
type HostStringF64Fn = unsafe extern "C" fn(*const c_char) -> c_double;
type HostStringF32Fn = unsafe extern "C" fn(*const c_char) -> c_float;
type HostStringLongDoubleFn = unsafe extern "C" fn(*const c_char) -> HostLongDouble;

#[repr(C)]
struct HostDivResultInt {
    quot: c_int,
    rem: c_int,
}

#[repr(C)]
struct HostDivResultLongLong {
    quot: c_longlong,
    rem: c_longlong,
}

type HostLconv = libc::lconv;

#[repr(C)]
#[derive(Clone, Copy)]
struct HostTm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

type HostFExcept = u16;

#[repr(C)]
#[derive(Clone, Copy)]
struct HostFEnv {
    opaque: [u8; 16],
}

#[cfg(target_os = "wasi")]
#[derive(Debug, Clone, Copy)]
struct ModeledFloatingEnvironment {
    exceptions: c_int,
    rounding: c_int,
}

#[cfg(target_os = "wasi")]
impl Default for ModeledFloatingEnvironment {
    fn default() -> Self {
        Self {
            exceptions: 0,
            rounding: HOST_FE_TONEAREST,
        }
    }
}

#[derive(Debug, Clone)]
enum SignalHandlerState {
    Default,
    Ignore,
    Function(String),
}

#[derive(Debug, Clone, Copy)]
enum ComplexFunctionKind {
    Constructor,
    UnaryComplex,
    BinaryComplex,
    UnaryReal,
}

mod calendar;
mod execution;
mod formatted_io;
mod initialization;
mod library_dispatch;
mod math;
mod runtime_library;
mod stdio;
mod validation;
mod value_semantics;

impl TypedValue {
    fn from_data(ty: CType, data: ValueData) -> Self {
        Self {
            ty,
            data,
            restrict_source: None,
            indeterminate: false,
            missing_return: false,
        }
    }

    fn pointer(ty: CType, pointer: PointerValue) -> Self {
        Self::from_data(ty, ValueData::Pointer(Rc::new(pointer)))
    }

    fn function(ty: CType, name: impl Into<Rc<str>>) -> Self {
        Self::from_data(ty, ValueData::Function(name.into()))
    }

    fn aggregate(ty: CType, value: StoredValue) -> Self {
        Self::from_data(ty, ValueData::Aggregate(Box::new(value)))
    }

    fn object_representation(ty: CType, bytes: Vec<ByteCell>) -> Self {
        Self::from_data(ty, ValueData::ObjectRepresentation(Box::new(bytes)))
    }

    fn void() -> Self {
        Self::from_data(CType::Void, ValueData::Void)
    }

    fn int(value: i128) -> Self {
        Self::from_data(CType::Int, ValueData::Int(CIntValue::new(value)))
    }

    fn integer(ty: CType, value: i128) -> Self {
        let normalized = ty
            .normalize_integer_value(value)
            .expect("integer() requires an integer type");
        Self::from_data(ty, ValueData::Int(CIntValue::new(normalized)))
    }

    fn floating(ty: CType, value: f64) -> Self {
        debug_assert!(ty.is_floating());
        Self::from_data(ty, ValueData::Float(value))
    }

    fn complex(ty: CType, real: f64, imag: f64) -> Self {
        debug_assert!(ty.is_complex());
        Self::from_data(ty, ValueData::Complex(ComplexValue { real, imag }))
    }

    fn indeterminate_for(ty: CType) -> Self {
        let data = match ty.unqualified() {
            CType::Void => ValueData::Void,
            CType::Pointer(_) => ValueData::Pointer(Rc::new(Interpreter::null_pointer())),
            CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                ValueData::Aggregate(Box::new(StoredValue::Indeterminate))
            }
            CType::Complex(_) => ValueData::Complex(ComplexValue {
                real: 0.0,
                imag: 0.0,
            }),
            CType::Float | CType::Double | CType::LongDouble => ValueData::Float(0.0),
            _ => ValueData::Int(CIntValue::new(0)),
        };
        Self {
            ty,
            data,
            restrict_source: None,
            indeterminate: true,
            missing_return: false,
        }
    }

    fn missing_return(ty: CType) -> Self {
        let data = match ty.unqualified() {
            CType::Void => ValueData::Void,
            CType::Pointer(_) => ValueData::Pointer(Rc::new(Interpreter::null_pointer())),
            CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                ValueData::Aggregate(Box::new(StoredValue::Indeterminate))
            }
            CType::Function(..) => ValueData::Void,
            CType::Complex(_) => ValueData::Complex(ComplexValue {
                real: 0.0,
                imag: 0.0,
            }),
            CType::Float | CType::Double | CType::LongDouble => ValueData::Float(0.0),
            _ => ValueData::Int(CIntValue::new(0)),
        };
        Self {
            ty,
            data,
            restrict_source: None,
            indeterminate: false,
            missing_return: true,
        }
    }

    fn to_int(&self) -> Result<i128, Diagnostic> {
        match &self.data {
            ValueData::Int(value) => Ok(value.get()),
            _ => Err(Diagnostic::error(
                format!("expected integer value, got {}", self.ty),
                crate::source::Span::new(crate::source::FileId(0), 0, 0),
            )),
        }
    }

    fn to_float(&self) -> Result<f64, Diagnostic> {
        match &self.data {
            ValueData::Float(value) => Ok(*value),
            _ => Err(Diagnostic::error(
                format!("expected floating value, got {}", self.ty),
                crate::source::Span::new(crate::source::FileId(0), 0, 0),
            )),
        }
    }

    fn to_complex(&self) -> Result<ComplexValue, Diagnostic> {
        match &self.data {
            ValueData::Complex(value) => Ok(*value),
            _ => Err(Diagnostic::error(
                format!("expected complex value, got {}", self.ty),
                crate::source::Span::new(crate::source::FileId(0), 0, 0),
            )),
        }
    }

    fn as_pointer(&self, span: Span) -> Result<PointerValue, Diagnostic> {
        if self.indeterminate && self.ty.is_pointer() {
            return Err(Diagnostic::ub(
                "use of an indeterminate pointer value",
                span,
                Some("6.2.4"),
            ));
        }
        match &self.data {
            ValueData::Pointer(pointer) => Ok(pointer.as_ref().clone()),
            _ => Err(Diagnostic::error(
                format!("expected pointer value, got {}", self.ty),
                span,
            )),
        }
    }

    fn with_span(self, _span: Span) -> Self {
        self
    }
}

impl PointerValue {
    fn null() -> Self {
        Self {
            object: None,
            base_offset: 0,
            offset: 0,
            member_path: Rc::new(Vec::new()),
            designated_root_ty: None,
            byte_offset_override: None,
            arithmetic_domain_start: None,
            object_representation_domain: None,
        }
    }

    fn at_object(object: ObjectId) -> Self {
        Self {
            object: Some(object),
            ..Self::null()
        }
    }

    fn is_null(&self) -> bool {
        self.object.is_none()
            && self.base_offset == 0
            && self.offset == 0
            && self.member_path.is_empty()
    }
}

impl ComplexValue {
    fn new(real: f64, imag: f64) -> Self {
        Self { real, imag }
    }

    fn mul(self, other: Self) -> Self {
        Self::new(
            self.real.mul_add(other.real, -(self.imag * other.imag)),
            self.real.mul_add(other.imag, self.imag * other.real),
        )
    }

    fn div(self, other: Self) -> Self {
        if other.real.abs() >= other.imag.abs() {
            let ratio = other.imag / other.real;
            let denominator = 1.0 + ratio * ratio;
            Self::new(
                self.real / other.real / denominator
                    + (self.imag / other.real) * (ratio / denominator),
                self.imag / other.real / denominator
                    - (self.real / other.real) * (ratio / denominator),
            )
        } else {
            let ratio = other.real / other.imag;
            let denominator = 1.0 + ratio * ratio;
            Self::new(
                (self.real / other.imag) * (ratio / denominator)
                    + self.imag / other.imag / denominator,
                (self.imag / other.imag) * (ratio / denominator)
                    - self.real / other.imag / denominator,
            )
        }
    }
}

fn all_bit_patterns_valid(ty: &CType) -> bool {
    matches!(
        ty.unqualified(),
        CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Int
            | CType::UnsignedInt
            | CType::Long
            | CType::UnsignedLong
            | CType::LongLong
            | CType::UnsignedLongLong
            | CType::Enum(_, _)
            | CType::Float
            | CType::Double
            | CType::LongDouble
            | CType::Complex(_)
    )
}

impl RestrictSource {
    fn from_lvalue(lvalue: &LValue, pointed_to_const: bool) -> Self {
        Self {
            object: lvalue.object,
            base_offset: lvalue.base_offset,
            offset: lvalue.offset,
            member_path: lvalue.member_path.clone(),
            pointed_to_const,
        }
    }
}

fn function_symbol(function: &FunctionDef) -> String {
    if function.storage_class == Some(StorageClass::Static) {
        format!("__static_fn:{}:{}", function.span.file.0, function.name)
    } else {
        function.name.clone()
    }
}

fn function_contains_restrict_declaration(function: &FunctionDef) -> bool {
    function
        .params
        .iter()
        .any(|parameter| parameter.ty.top_level_qualifiers().is_restrict)
        || block_contains_restrict_declaration(&function.body)
}

fn block_contains_restrict_declaration(block: &Block) -> bool {
    block.items.iter().any(|item| match item {
        BlockItem::Declaration(declaration) => declaration.ty.top_level_qualifiers().is_restrict,
        BlockItem::FunctionDeclaration(_) => false,
        BlockItem::Statement(statement) => statement_contains_restrict_declaration(statement),
    })
}

fn statement_contains_restrict_declaration(statement: &Statement) -> bool {
    match statement {
        Statement::Block(block) => block_contains_restrict_declaration(block),
        Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Expression(..)
        | Statement::Goto { .. }
        | Statement::Return(..) => false,
        Statement::DoWhile { body, .. } | Statement::While { body, .. } => {
            statement_contains_restrict_declaration(body)
        }
        Statement::For { init, body, .. } => {
            init.as_ref().is_some_and(|init| match init {
                ForInit::Declarations(declarations) => declarations
                    .iter()
                    .any(|declaration| declaration.ty.top_level_qualifiers().is_restrict),
                ForInit::Expression(_) => false,
            }) || statement_contains_restrict_declaration(body)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            statement_contains_restrict_declaration(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(statement_contains_restrict_declaration)
        }
        Statement::Labeled { statement, .. } | Statement::UserLabeled { statement, .. } => {
            statement_contains_restrict_declaration(statement)
        }
        Statement::Switch { body, .. } => block_contains_restrict_declaration(body),
    }
}

fn function_body_contains_setjmp(block: &Block) -> bool {
    block.items.iter().any(block_item_contains_setjmp)
}

fn block_item_contains_setjmp(item: &BlockItem) -> bool {
    match item {
        BlockItem::Declaration(decl) => declaration_contains_setjmp(decl),
        BlockItem::FunctionDeclaration(_) => false,
        BlockItem::Statement(stmt) => statement_contains_setjmp(stmt),
    }
}

fn declaration_contains_setjmp(decl: &Declaration) -> bool {
    decl.vla_bounds.iter().flatten().any(expr_contains_setjmp)
        || decl.init.as_ref().is_some_and(initializer_contains_setjmp)
}

fn initializer_contains_setjmp(initializer: &Initializer) -> bool {
    match initializer {
        Initializer::Expr(expr) => expr_contains_setjmp(expr),
        Initializer::List { items, .. } => items
            .iter()
            .any(|item| initializer_contains_setjmp(&item.initializer)),
    }
}

fn statement_contains_setjmp(stmt: &Statement) -> bool {
    match stmt {
        Statement::Block(block) => function_body_contains_setjmp(block),
        Statement::Break(_) | Statement::Continue(_) => false,
        Statement::DoWhile {
            body, condition, ..
        } => statement_contains_setjmp(body) || expr_contains_setjmp(condition),
        Statement::Expression(expr, _) => expr.as_ref().is_some_and(expr_contains_setjmp),
        Statement::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            init.as_ref().is_some_and(for_init_contains_setjmp)
                || condition.as_ref().is_some_and(expr_contains_setjmp)
                || step.as_ref().is_some_and(expr_contains_setjmp)
                || statement_contains_setjmp(body)
        }
        Statement::Goto { .. } => false,
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_setjmp(condition)
                || statement_contains_setjmp(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(statement_contains_setjmp)
        }
        Statement::Labeled {
            label, statement, ..
        } => switch_label_contains_setjmp(label) || statement_contains_setjmp(statement),
        Statement::Return(expr, _) => expr.as_ref().is_some_and(expr_contains_setjmp),
        Statement::Switch { expr, body, .. } => {
            expr_contains_setjmp(expr) || function_body_contains_setjmp(body)
        }
        Statement::UserLabeled { statement, .. } => statement_contains_setjmp(statement),
        Statement::While {
            condition, body, ..
        } => expr_contains_setjmp(condition) || statement_contains_setjmp(body),
    }
}

fn for_init_contains_setjmp(init: &ForInit) -> bool {
    match init {
        ForInit::Declarations(decls) => decls.iter().any(declaration_contains_setjmp),
        ForInit::Expression(expr) => expr_contains_setjmp(expr),
    }
}

fn switch_label_contains_setjmp(label: &SwitchLabel) -> bool {
    match label {
        SwitchLabel::Case { expr, .. } => expr_contains_setjmp(expr),
        SwitchLabel::Default { .. } => false,
    }
}

fn expr_contains_setjmp(expr: &Expr) -> bool {
    match expr {
        Expr::Number(..)
        | Expr::CharLiteral(..)
        | Expr::WideCharLiteral(..)
        | Expr::Utf16CharLiteral(..)
        | Expr::Utf32CharLiteral(..)
        | Expr::StringLiteral(..)
        | Expr::WideStringLiteral(..)
        | Expr::Utf16StringLiteral(..)
        | Expr::Utf32StringLiteral(..)
        | Expr::Variable(..)
        | Expr::OffsetOf { .. } => false,
        Expr::Unary { expr, .. }
        | Expr::Postfix { expr, .. }
        | Expr::SizeofExpr { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::VaArg { ap: expr, .. }
        | Expr::Member { base: expr, .. } => expr_contains_setjmp(expr),
        Expr::Binary { lhs, rhs, .. }
        | Expr::Subscript {
            base: lhs,
            index: rhs,
            ..
        }
        | Expr::Assign { lhs, rhs, .. }
        | Expr::CompoundAssign { lhs, rhs, .. } => {
            expr_contains_setjmp(lhs) || expr_contains_setjmp(rhs)
        }
        Expr::SizeofType { vla_bounds, .. } => {
            vla_bounds.iter().flatten().any(expr_contains_setjmp)
        }
        Expr::CompoundLiteral {
            vla_bounds,
            initializer,
            ..
        } => {
            vla_bounds.iter().flatten().any(expr_contains_setjmp)
                || initializer_contains_setjmp(initializer)
        }
        Expr::GenericSelection {
            control,
            associations,
            default,
            ..
        } => {
            expr_contains_setjmp(control)
                || associations
                    .iter()
                    .any(|assoc| expr_contains_setjmp(&assoc.expr))
                || default.as_deref().is_some_and(expr_contains_setjmp)
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            expr_contains_setjmp(condition)
                || expr_contains_setjmp(then_expr)
                || expr_contains_setjmp(else_expr)
        }
        Expr::Call { callee, args, .. } => {
            matches!(callee.as_ref(), Expr::Variable(name, _) if name == "__codex_setjmp")
                || expr_contains_setjmp(callee)
                || args.iter().any(expr_contains_setjmp)
        }
    }
}

fn align_address(value: u64, align: u64) -> u64 {
    let align = align.max(1);
    let remainder = value % align;
    if remainder == 0 {
        value
    } else {
        value.saturating_add(align - remainder)
    }
}

fn object_address_stride(size: u64, align: u64) -> u64 {
    align_address(size.max(1), align.max(1)).saturating_add(align.max(1))
}

fn cboxes_expression_has_side_effects(expr: &Expr) -> bool {
    match expr {
        Expr::Assign { .. }
        | Expr::CompoundAssign { .. }
        | Expr::Call { .. }
        | Expr::VaArg { .. } => true,
        Expr::Unary { op, expr, .. } => {
            matches!(op, UnaryOp::PreIncrement | UnaryOp::PreDecrement)
                || cboxes_expression_has_side_effects(expr)
        }
        Expr::Postfix { op, expr, .. } => {
            matches!(op, PostfixOp::PostIncrement | PostfixOp::PostDecrement)
                || cboxes_expression_has_side_effects(expr)
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Subscript {
            base: lhs,
            index: rhs,
            ..
        } => cboxes_expression_has_side_effects(lhs) || cboxes_expression_has_side_effects(rhs),
        Expr::SizeofType { vla_bounds, .. } => vla_bounds
            .iter()
            .flatten()
            .any(cboxes_expression_has_side_effects),
        Expr::SizeofExpr { expr, .. } => cboxes_expression_has_side_effects(expr),
        Expr::Cast {
            vla_bounds, expr, ..
        } => {
            vla_bounds
                .iter()
                .flatten()
                .any(cboxes_expression_has_side_effects)
                || cboxes_expression_has_side_effects(expr)
        }
        Expr::CompoundLiteral {
            vla_bounds,
            initializer,
            ..
        } => {
            vla_bounds
                .iter()
                .flatten()
                .any(cboxes_expression_has_side_effects)
                || cboxes_initializer_has_side_effects(initializer)
        }
        Expr::GenericSelection {
            control,
            associations,
            default,
            ..
        } => {
            cboxes_expression_has_side_effects(control)
                || associations
                    .iter()
                    .any(|association| cboxes_expression_has_side_effects(&association.expr))
                || default
                    .as_deref()
                    .is_some_and(cboxes_expression_has_side_effects)
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            cboxes_expression_has_side_effects(condition)
                || cboxes_expression_has_side_effects(then_expr)
                || cboxes_expression_has_side_effects(else_expr)
        }
        Expr::Member { base, .. } => cboxes_expression_has_side_effects(base),
        Expr::Number(..)
        | Expr::CharLiteral(..)
        | Expr::WideCharLiteral(..)
        | Expr::StringLiteral(..)
        | Expr::WideStringLiteral(..)
        | Expr::Utf16CharLiteral(..)
        | Expr::Utf32CharLiteral(..)
        | Expr::Utf16StringLiteral(..)
        | Expr::Utf32StringLiteral(..)
        | Expr::Variable(..)
        | Expr::OffsetOf { .. } => false,
    }
}

fn cboxes_initializer_has_side_effects(initializer: &Initializer) -> bool {
    match initializer {
        Initializer::Expr(expr) => cboxes_expression_has_side_effects(expr),
        Initializer::List { items, .. } => items
            .iter()
            .any(|item| cboxes_initializer_has_side_effects(&item.initializer)),
    }
}

fn cboxes_type_string(ty: &CType) -> String {
    cboxes_render_c_type(ty, String::new())
}

fn cboxes_type_help(ty: &CType) -> Option<String> {
    cboxes_type_contains_array_or_function(ty).then(|| cboxes_describe_type(ty))
}

fn cboxes_type_help_type_names(ty: &CType) -> Vec<String> {
    if !cboxes_type_contains_array_or_function(ty) {
        return Vec::new();
    }
    let mut names = Vec::new();
    cboxes_collect_type_names(ty, &mut names);
    let help = cboxes_describe_type(ty);
    names.retain(|name| help.contains(name));
    names.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    names.dedup();
    names
}

fn cboxes_type_help_tree(ty: &CType) -> Option<ProgramTypeHelpNode> {
    cboxes_type_contains_array_or_function(ty).then(|| cboxes_type_help_node(ty))
}

fn cboxes_type_help_node(ty: &CType) -> ProgramTypeHelpNode {
    let node = |kind: &str,
                label: String,
                type_name: Option<String>,
                children: Vec<ProgramTypeHelpChild>| ProgramTypeHelpNode {
        kind: kind.to_owned(),
        label,
        type_name,
        children,
    };
    let child = |relation: &str, ty: &CType| ProgramTypeHelpChild {
        relation: relation.to_owned(),
        node: Box::new(cboxes_type_help_node(ty)),
    };

    match ty {
        CType::Pointer(pointed_to) => node(
            "pointer",
            "pointer".to_owned(),
            None,
            vec![child("to", pointed_to)],
        ),
        CType::Array(element, len) => node(
            "array",
            if *len == 0 {
                "array of unknown length".to_owned()
            } else {
                format!("array of {len}")
            },
            None,
            vec![child("of", element)],
        ),
        CType::Function(return_type, params, is_variadic) => {
            let no_arguments = params.len() == 1 && matches!(params[0].unqualified(), CType::Void);
            let label = if no_arguments {
                "function taking no arguments".to_owned()
            } else if params.is_empty() && !is_variadic {
                "function with unspecified argument types".to_owned()
            } else if *is_variadic {
                "variadic function".to_owned()
            } else {
                "function".to_owned()
            };
            let mut children = if no_arguments {
                Vec::new()
            } else {
                params
                    .iter()
                    .enumerate()
                    .map(|(index, param)| child(&format!("parameter {}", index + 1), param))
                    .collect()
            };
            children.push(child("returns", return_type));
            node("function", label, None, children)
        }
        CType::Qualified(inner, qualifiers) => match inner.as_ref() {
            CType::Pointer(pointed_to) => node(
                "pointer",
                format!("{} pointer", cboxes_qualifier_string(*qualifiers)),
                None,
                vec![child("to", pointed_to)],
            ),
            CType::Array(element, len) => {
                let qualified_element = CType::qualified((**element).clone(), *qualifiers);
                node(
                    "array",
                    if *len == 0 {
                        "array of unknown length".to_owned()
                    } else {
                        format!("array of {len}")
                    },
                    None,
                    vec![child("of", &qualified_element)],
                )
            }
            _ => node(
                "type",
                cboxes_type_string(ty),
                Some(cboxes_type_string(ty)),
                Vec::new(),
            ),
        },
        _ => node(
            "type",
            cboxes_type_string(ty),
            Some(cboxes_type_string(ty)),
            Vec::new(),
        ),
    }
}

fn cboxes_collect_type_names(ty: &CType, names: &mut Vec<String>) {
    names.push(cboxes_type_string(ty));
    match ty {
        CType::Pointer(inner)
        | CType::Array(inner, _)
        | CType::Qualified(inner, _)
        | CType::Complex(inner) => cboxes_collect_type_names(inner, names),
        CType::Function(return_type, params, _) => {
            cboxes_collect_type_names(return_type, names);
            for param in params.iter() {
                cboxes_collect_type_names(param, names);
            }
        }
        _ => {}
    }
}

fn cboxes_type_contains_array_or_function(ty: &CType) -> bool {
    match ty {
        CType::Array(_, _) | CType::Function(_, _, _) => true,
        CType::Pointer(inner) | CType::Qualified(inner, _) | CType::Complex(inner) => {
            cboxes_type_contains_array_or_function(inner)
        }
        _ => false,
    }
}

fn cboxes_type_contains_declarator(ty: &CType) -> bool {
    match ty {
        CType::Pointer(_) | CType::Array(_, _) | CType::Function(_, _, _) => true,
        CType::Qualified(inner, _) | CType::Complex(inner) => {
            cboxes_type_contains_declarator(inner)
        }
        _ => false,
    }
}

fn cboxes_describe_type(ty: &CType) -> String {
    match ty {
        CType::Pointer(pointed_to) => cboxes_describe_pointer(pointed_to, None),
        CType::Array(element, len) => cboxes_describe_array(element, *len),
        CType::Function(return_type, params, is_variadic) => {
            cboxes_describe_function(return_type, params, *is_variadic)
        }
        CType::Qualified(inner, qualifiers) => match inner.as_ref() {
            CType::Pointer(pointed_to) => cboxes_describe_pointer(pointed_to, Some(*qualifiers)),
            CType::Array(element, len) => {
                let qualified_element = CType::qualified((**element).clone(), *qualifiers);
                cboxes_describe_array(&qualified_element, *len)
            }
            _ => cboxes_type_string(ty),
        },
        _ => cboxes_type_string(ty),
    }
}

fn cboxes_describe_pointer(pointed_to: &CType, qualifiers: Option<TypeQualifiers>) -> String {
    let qualifiers = qualifiers.map(cboxes_qualifier_string).unwrap_or_default();
    let pointer = if qualifiers.is_empty() {
        "pointer".to_owned()
    } else {
        format!("{qualifiers} pointer")
    };
    format!("{pointer} to {}", cboxes_describe_type_target(pointed_to))
}

fn cboxes_describe_array(element: &CType, len: usize) -> String {
    if len == 0 {
        format!(
            "array of unknown length containing {}",
            cboxes_describe_array_element(element)
        )
    } else {
        format!("array of {len} {}", cboxes_describe_array_element(element))
    }
}

fn cboxes_describe_array_element(element: &CType) -> String {
    match element {
        CType::Pointer(pointed_to) => {
            format!("pointers to {}", cboxes_describe_type_target(pointed_to))
        }
        CType::Array(inner, 0) => format!(
            "arrays of unknown length containing {}",
            cboxes_describe_array_element(inner)
        ),
        CType::Array(inner, len) => {
            format!("arrays of {len} {}", cboxes_describe_array_element(inner))
        }
        CType::Qualified(inner, qualifiers) => match inner.as_ref() {
            CType::Pointer(pointed_to) => format!(
                "{} pointers to {}",
                cboxes_qualifier_string(*qualifiers),
                cboxes_describe_type_target(pointed_to)
            ),
            CType::Array(element, len) => {
                let qualified_element = CType::qualified((**element).clone(), *qualifiers);
                if *len == 0 {
                    format!(
                        "arrays of unknown length containing {}",
                        cboxes_describe_array_element(&qualified_element)
                    )
                } else {
                    format!(
                        "arrays of {len} {}",
                        cboxes_describe_array_element(&qualified_element)
                    )
                }
            }
            _ => cboxes_plural_type_name(element),
        },
        _ => cboxes_plural_type_name(element),
    }
}

fn cboxes_plural_type_name(ty: &CType) -> String {
    let name = cboxes_type_string(ty);
    if name == "int"
        || name.ends_with(" int")
        || name == "char"
        || name.ends_with(" char")
        || name == "float"
        || name.ends_with(" float")
        || name == "double"
        || name.ends_with(" double")
    {
        format!("{name}s")
    } else {
        format!("{name} values")
    }
}

fn cboxes_describe_function(return_type: &CType, params: &[CType], is_variadic: bool) -> String {
    let parameters = if params.len() == 1 && matches!(params[0].unqualified(), CType::Void) {
        "taking no arguments".to_owned()
    } else if params.is_empty() && !is_variadic {
        "whose argument types are not specified".to_owned()
    } else {
        let fixed = params
            .iter()
            .map(cboxes_type_string)
            .collect::<Vec<_>>()
            .join(", ");
        if is_variadic {
            if fixed.is_empty() {
                "taking additional arguments".to_owned()
            } else {
                format!("taking arguments of types {fixed}, followed by additional arguments")
            }
        } else {
            format!("taking arguments of types {fixed}")
        }
    };
    format!(
        "function {parameters} and returning {}",
        cboxes_describe_type_target(return_type)
    )
}

fn cboxes_describe_type_target(ty: &CType) -> String {
    if cboxes_type_contains_declarator(ty) {
        cboxes_describe_type(ty)
    } else {
        cboxes_type_string(ty)
    }
}

fn cboxes_render_c_type(ty: &CType, declarator: String) -> String {
    match ty {
        CType::Pointer(inner) => {
            cboxes_render_c_type(inner, cboxes_pointer_declarator(declarator, None))
        }
        CType::Array(inner, len) => {
            let suffix = if *len == 0 {
                "[]".to_owned()
            } else {
                format!("[{len}]")
            };
            cboxes_render_c_type(inner, cboxes_suffix_declarator(declarator, &suffix))
        }
        CType::Function(return_type, params, is_variadic) => {
            let mut rendered_params = params.iter().map(cboxes_type_string).collect::<Vec<_>>();
            if *is_variadic {
                rendered_params.push("...".to_owned());
            }
            let suffix = format!("({})", rendered_params.join(", "));
            cboxes_render_c_type(return_type, cboxes_suffix_declarator(declarator, &suffix))
        }
        CType::Qualified(inner, qualifiers) => match inner.as_ref() {
            CType::Pointer(pointed_to) => cboxes_render_c_type(
                pointed_to,
                cboxes_pointer_declarator(declarator, Some(*qualifiers)),
            ),
            CType::Array(element, len) => {
                let qualified_element = CType::qualified((**element).clone(), *qualifiers);
                let suffix = if *len == 0 {
                    "[]".to_owned()
                } else {
                    format!("[{len}]")
                };
                cboxes_render_c_type(
                    &qualified_element,
                    cboxes_suffix_declarator(declarator, &suffix),
                )
            }
            _ => {
                let rendered = cboxes_render_c_type(inner, declarator);
                let qualifiers = cboxes_qualifier_string(*qualifiers);
                if qualifiers.is_empty() {
                    rendered
                } else {
                    format!("{qualifiers} {rendered}")
                }
            }
        },
        _ => {
            let base = ty.to_string();
            if declarator.is_empty() || declarator.starts_with(['*', '[']) {
                format!("{base}{declarator}")
            } else {
                format!("{base} {declarator}")
            }
        }
    }
}

fn cboxes_pointer_declarator(declarator: String, qualifiers: Option<TypeQualifiers>) -> String {
    let qualifiers = qualifiers.map(cboxes_qualifier_string).unwrap_or_default();
    if declarator.is_empty() {
        if qualifiers.is_empty() {
            "*".to_owned()
        } else {
            format!("* {qualifiers}")
        }
    } else if qualifiers.is_empty() {
        format!("*{declarator}")
    } else {
        format!("* {qualifiers} {declarator}")
    }
}

fn cboxes_suffix_declarator(declarator: String, suffix: &str) -> String {
    if declarator.trim_start().starts_with('*') {
        format!("({declarator}){suffix}")
    } else {
        format!("{declarator}{suffix}")
    }
}

fn cboxes_qualifier_string(qualifiers: TypeQualifiers) -> String {
    let mut words = Vec::new();
    if qualifiers.is_const {
        words.push("const");
    }
    if qualifiers.is_restrict {
        words.push("restrict");
    }
    if qualifiers.is_volatile {
        words.push("volatile");
    }
    words.join(" ")
}

fn cboxes_type_kind(ty: &CType) -> &'static str {
    match ty.unqualified() {
        CType::Void => "void",
        CType::Bool
        | CType::Char
        | CType::SignedChar
        | CType::UnsignedChar
        | CType::Short
        | CType::UnsignedShort
        | CType::Int
        | CType::UnsignedInt
        | CType::Long
        | CType::UnsignedLong
        | CType::LongLong
        | CType::UnsignedLongLong
        | CType::Enum(_, _) => "integer",
        CType::Float | CType::Double | CType::LongDouble => "floating",
        CType::Complex(_) => "complex",
        CType::Pointer(_) => "pointer",
        CType::Array(_, _) => "array",
        CType::Struct(_, _) | CType::Union(_, _) => "aggregate",
        CType::Function(_, _, _) => "function",
        CType::VaList => "va-list",
        CType::Qualified(_, _) => unreachable!("unqualified() removes top-level qualifiers"),
    }
}

fn cboxes_pointer_depth(ty: &CType) -> usize {
    let mut depth = 0;
    let mut current = ty;
    loop {
        match current.unqualified() {
            CType::Pointer(inner) => {
                depth += 1;
                current = inner;
            }
            _ => return depth,
        }
    }
}

fn cboxes_pointee_array_shape(ty: &CType) -> Vec<usize> {
    let mut current = ty;
    let mut saw_pointer = false;
    while let CType::Pointer(inner) = current.unqualified() {
        saw_pointer = true;
        current = inner;
    }
    if saw_pointer {
        cboxes_array_shape(current)
    } else {
        Vec::new()
    }
}

fn cboxes_float_string(value: f64) -> String {
    if value.is_nan() {
        return if value.is_sign_negative() {
            "-NaN".to_owned()
        } else {
            "NaN".to_owned()
        };
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        };
    }
    if value.fract() == 0.0 {
        return format!("{value:.1}");
    }
    value.to_string()
}

fn cboxes_float_special_string(value: f64) -> Option<String> {
    if value.is_nan() {
        return Some(
            if value.is_sign_negative() {
                "-nan"
            } else {
                "nan"
            }
            .to_owned(),
        );
    }
    if value.is_infinite() {
        return Some(
            if value.is_sign_negative() {
                "-inf"
            } else {
                "inf"
            }
            .to_owned(),
        );
    }
    None
}

fn cboxes_trim_fixed_float(mut value: String) -> String {
    if let Some(decimal) = value.find('.') {
        while value.len() > decimal + 2 && value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.push('0');
        }
    } else {
        value.push_str(".0");
    }
    value
}

fn cboxes_float_default_string(value: f64) -> String {
    if let Some(special) = cboxes_float_special_string(value) {
        return special;
    }
    cboxes_trim_fixed_float(format!("{value:.6}"))
}

fn cboxes_float_exact_string(value: f64) -> String {
    if let Some(special) = cboxes_float_special_string(value) {
        return special;
    }
    cboxes_trim_fixed_float(format!("{value:.1074}"))
}

fn cboxes_array_shape(ty: &CType) -> Vec<usize> {
    let mut shape = Vec::new();
    let mut current = ty;
    while let CType::Array(inner, len) = current.unqualified() {
        shape.push(*len);
        current = inner;
    }
    shape
}

fn cboxes_array_element_name(root_name: &str, indices: &[usize]) -> String {
    let mut name = root_name.to_owned();
    for index in indices {
        name.push('[');
        name.push_str(&index.to_string());
        name.push(']');
    }
    name
}

fn statement_span(stmt: &Statement) -> Span {
    match stmt {
        Statement::Block(block) => block.span,
        Statement::Break(span)
        | Statement::Continue(span)
        | Statement::DoWhile { span, .. }
        | Statement::Expression(_, span)
        | Statement::For { span, .. }
        | Statement::Goto { span, .. }
        | Statement::Labeled { span, .. }
        | Statement::Return(_, span)
        | Statement::Switch { span, .. }
        | Statement::UserLabeled { span, .. }
        | Statement::If { span, .. }
        | Statement::While { span, .. } => *span,
    }
}

#[cfg(test)]
mod tests;
