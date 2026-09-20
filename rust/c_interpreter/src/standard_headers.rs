//! Missing-header hints derived from the bundled system headers, including
//! macro-generated declarations, typedefs, tags, and constants.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::diag::Diagnostic;
use crate::preprocess::{Preprocessor, STANDARD_HEADERS};
use crate::source::Span;

pub(crate) fn header_for(name: &str) -> Option<&'static str> {
    static HEADERS: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    HEADERS
        .get_or_init(|| {
            let mut names = HashMap::new();
            for &(header, _) in STANDARD_HEADERS {
                if let Ok(identifiers) = Preprocessor::standard_header_identifiers(header) {
                    for name in identifiers {
                        names.entry(name).or_insert(header);
                    }
                }
            }
            names
        })
        .get(name)
        .copied()
}

pub(crate) fn undeclared_identifier(name: &str, span: Span) -> Diagnostic {
    let message = match header_for(name) {
        Some(header) => format!("To use {name}, add #include <{header}> at the top of the file."),
        None => format!("use of undeclared identifier {name}"),
    };
    Diagnostic::error(message, span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_header_can_be_inventoried() {
        for &(header, _) in STANDARD_HEADERS {
            let names = Preprocessor::standard_header_identifiers(header)
                .unwrap_or_else(|error| panic!("{header}: {error:?}"));
            assert!(!names.is_empty(), "{header}");
            for name in names {
                assert!(header_for(&name).is_some(), "{header}: {name}");
            }
        }
        for (name, header) in [
            ("INT_MAX", "limits.h"),
            ("fmax", "math.h"),
            ("fmaxl", "math.h"),
            ("size_t", "stddef.h"),
            ("uint64_t", "stdint.h"),
            ("printf", "stdio.h"),
            ("bool", "stdbool.h"),
            ("va_list", "stdarg.h"),
            ("tm", "time.h"),
            ("PRIu64", "inttypes.h"),
        ] {
            assert_eq!(header_for(name), Some(header), "{name}");
        }
        assert_eq!(header_for("user_function"), None);
        assert_eq!(header_for("__CBOXES_STDIO_H"), None);
    }

    #[test]
    fn missing_headers_are_reported_for_constants_types_and_functions() {
        for (source, header) in [
            ("int main(void) { return INT_MAX; }", "limits.h"),
            ("int main(void) { return fmax(1,2); }", "math.h"),
            ("int main(void) { return fmaxl(1,2); }", "math.h"),
            ("int main(void) { FILE *f; }", "stdio.h"),
            ("int main(void) { const uint32_t n=1; }", "stdint.h"),
            ("int main(void) { struct tm t; }", "time.h"),
            ("int main(void) { va_list args; }", "stdarg.h"),
            ("size_t n; int main(void) {}", "stddef.h"),
            ("const uint32_t n=1; int main(void) {}", "stdint.h"),
            ("int main(void) { bool b=true; }", "stdbool.h"),
            ("int main(void) { return EXIT_SUCCESS; }", "stdlib.h"),
            ("int main(void) { return errno; }", "errno.h"),
            ("int main(void) { return DBL_MAX > 0; }", "float.h"),
            ("int main(void) { return strlen(\"x\"); }", "string.h"),
        ] {
            let error = crate::run_source("test.c", format!("{source}\n")).unwrap_err();
            assert!(
                error.message().contains(&format!("#include <{header}>")),
                "{source}: {error:?}"
            );
            crate::run_source("test.c", format!("#include <{header}>\n{source}\n"))
                .unwrap_or_else(|error| panic!("suggestion must fix {source}: {error:?}"));
        }
        crate::run_source(
            "test.c",
            "int fmax=3; int main(void) { int INT_MAX=2; return fmax+INT_MAX; }\n",
        )
        .unwrap();
        for name in ["va_start", "va_end", "va_copy", "va_arg"] {
            let error = crate::run_source("test.c", format!("int main(void) {{ {name}(0); }}\n"))
                .unwrap_err();
            assert!(
                error.message().contains("#include <stdarg.h>"),
                "{name}: {error:?}"
            );
        }
        for (source, header) in [
            (
                "#include <stdio.h>\nint main(void) { printf(\"%\" PRId64, (long)1); }\n",
                "inttypes.h",
            ),
            ("int main(void) { return 1 and 0; }\n", "iso646.h"),
        ] {
            let error = crate::run_source("test.c", source).unwrap_err();
            assert!(
                error.message().contains(&format!("#include <{header}>")),
                "{error:?}"
            );
        }
    }
}
