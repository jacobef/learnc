use super::*;

impl<'a> Interpreter<'a> {
    fn collect_function_visible_global_declarations(
        program: &TranslationUnit,
    ) -> HashMap<String, HashMap<String, Declaration>> {
        let mut external = HashMap::<String, Declaration>::default();
        let mut internal = FileScopedMap::<Declaration>::default();
        let mut visible_by_function = HashMap::default();

        let merge_declaration = |declarations: &mut HashMap<String, Declaration>,
                                 declaration: &Declaration| {
            if let Some(existing) = declarations.get_mut(&declaration.name) {
                existing.ty = composite_type(
                    &existing.ty,
                    &declaration.ty,
                    &program.records,
                    &program.enums,
                )
                .expect("translation-unit normalization already checked compatible declarations");
            } else {
                declarations.insert(declaration.name.clone(), declaration.clone());
            }
        };

        for external_declaration in &program.externals {
            match external_declaration {
                ExternalDeclaration::ObjectDeclaration(declaration) => {
                    if declaration.linkage == Some(Linkage::Internal) {
                        merge_declaration(
                            internal.entry(declaration.span.file).or_default(),
                            declaration,
                        );
                    } else {
                        merge_declaration(&mut external, declaration);
                    }
                }
                ExternalDeclaration::Function(function) => {
                    let mut visible = external.clone();
                    if let Some(file_declarations) = internal.get(&function.span.file) {
                        visible.extend(file_declarations.clone());
                    }
                    visible_by_function.insert(function_symbol(function), visible);
                }
                ExternalDeclaration::FunctionDeclaration(_) => {}
            }
        }
        visible_by_function
    }

    pub(super) fn null_pointer() -> PointerValue {
        PointerValue::null()
    }

    pub(super) fn lookup_function(&self, name: &str, file: FileId) -> Option<&Rc<FunctionDef>> {
        self.internal_functions
            .get(&file)
            .and_then(|functions| functions.get(name))
            .or_else(|| self.functions.get(name))
    }

    pub(super) fn lookup_function_declaration(
        &self,
        name: &str,
        file: FileId,
    ) -> Option<&FunctionDecl> {
        self.internal_declared_functions
            .get(&file)
            .and_then(|decls| decls.get(name))
            .or_else(|| self.declared_functions.get(name))
    }

    pub(super) fn lookup_function_symbol(&self, symbol: &str) -> Option<&Rc<FunctionDef>> {
        self.function_symbols.get(symbol)
    }

    pub(super) fn function_may_setjmp_symbol(&self, symbol: &str) -> bool {
        self.function_may_setjmp
            .get(symbol)
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn builtin_function_type(&self, name: &str) -> Option<CType> {
        match name {
            "va_start" => Some(CType::function(CType::Void, vec![CType::VaList])),
            "va_end" => Some(CType::function(CType::Void, vec![CType::VaList])),
            "va_copy" => Some(CType::function(
                CType::Void,
                vec![CType::VaList, CType::VaList],
            )),
            _ => None,
        }
    }

    pub(super) fn lookup_global_binding(&self, name: &str, file: FileId) -> Option<ObjectId> {
        self.internal_global_bindings
            .get(&file)
            .and_then(|bindings| bindings.get(name))
            .copied()
            .or_else(|| self.global_bindings.get(name).copied())
    }

    pub(super) fn lookup_global_declaration(
        &self,
        name: &str,
        file: FileId,
    ) -> Option<&Declaration> {
        self.internal_declared_globals
            .get(&file)
            .and_then(|decls| decls.get(name))
            .or_else(|| self.declared_globals.get(name))
    }

    pub(super) fn missing_definition_use_diag(&self, name: &str, span: Span) -> Diagnostic {
        Diagnostic::ub(
            format!(
                "identifier {} is used in an expression, but the program does not provide a definition for it",
                name
            ),
            span,
            Some("6.9"),
        )
    }

    fn validate_full_expr_constraints(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.validate_linkage_uses(expr, frame, objects)?;
        self.validate_expr_constraints(expr, frame, objects)
    }

    fn validate_linkage_uses(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match expr {
            Expr::Variable(name, span) => {
                if let Some(declaration) = frame.object_decls.get(name) {
                    if declaration.linkage.is_some()
                        && self.lookup_global_binding(name, span.file).is_none()
                    {
                        return Err(self.missing_definition_use_diag(name, *span));
                    }
                } else if frame.function_decls.contains_key(name) {
                    if self.lookup_function(name, span.file).is_none()
                        && !Self::is_host_library_function(name)
                    {
                        return Err(self.missing_definition_use_diag(name, *span));
                    }
                } else if self.lookup_global_declaration(name, span.file).is_some()
                    && self.lookup_global_binding(name, span.file).is_none()
                {
                    return Err(self.missing_definition_use_diag(name, *span));
                } else if self.lookup_function_declaration(name, span.file).is_some()
                    && self.lookup_function(name, span.file).is_none()
                    && !Self::is_host_library_function(name)
                {
                    return Err(self.missing_definition_use_diag(name, *span));
                }
            }
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::Member { base: expr, .. }
            | Expr::VaArg { ap: expr, .. } => {
                self.validate_linkage_uses(expr, frame, objects)?;
            }
            Expr::Binary { lhs, rhs, .. }
            | Expr::Subscript {
                base: lhs,
                index: rhs,
                ..
            }
            | Expr::Assign { lhs, rhs, .. }
            | Expr::CompoundAssign { lhs, rhs, .. } => {
                self.validate_linkage_uses(lhs, frame, objects)?;
                self.validate_linkage_uses(rhs, frame, objects)?;
            }
            Expr::SizeofType { vla_bounds, .. } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_linkage_uses(bound, frame, objects)?;
                }
            }
            Expr::SizeofExpr { expr, .. } => {
                if self.expr_refers_to_vla(expr, frame, objects) {
                    self.validate_linkage_uses(expr, frame, objects)?;
                }
            }
            Expr::Cast {
                vla_bounds, expr, ..
            } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_linkage_uses(bound, frame, objects)?;
                }
                self.validate_linkage_uses(expr, frame, objects)?;
            }
            Expr::CompoundLiteral {
                vla_bounds,
                initializer,
                ..
            } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_linkage_uses(bound, frame, objects)?;
                }
                self.validate_initializer_linkage_uses(initializer, frame, objects)?;
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                ..
            } => {
                self.validate_linkage_uses(control, frame, objects)?;
                for association in associations {
                    self.validate_linkage_uses(&association.expr, frame, objects)?;
                }
                if let Some(default) = default {
                    self.validate_linkage_uses(default, frame, objects)?;
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.validate_linkage_uses(condition, frame, objects)?;
                self.validate_linkage_uses(then_expr, frame, objects)?;
                self.validate_linkage_uses(else_expr, frame, objects)?;
            }
            Expr::Call { callee, args, .. } => {
                self.validate_linkage_uses(callee, frame, objects)?;
                for argument in args {
                    self.validate_linkage_uses(argument, frame, objects)?;
                }
            }
            Expr::Number(..)
            | Expr::CharLiteral(..)
            | Expr::WideCharLiteral(..)
            | Expr::Utf16CharLiteral(..)
            | Expr::Utf32CharLiteral(..)
            | Expr::StringLiteral(..)
            | Expr::WideStringLiteral(..)
            | Expr::Utf16StringLiteral(..)
            | Expr::Utf32StringLiteral(..)
            | Expr::OffsetOf { .. } => {}
        }
        Ok(())
    }

    fn validate_initializer_linkage_uses(
        &self,
        initializer: &Initializer,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match initializer {
            Initializer::Expr(expr) => self.validate_linkage_uses(expr, frame, objects),
            Initializer::List { items, .. } => {
                for item in items {
                    self.validate_initializer_linkage_uses(&item.initializer, frame, objects)?;
                }
                Ok(())
            }
        }
    }

    fn is_host_math_base_function(name: &str) -> bool {
        matches!(
            name,
            "__codex_huge_val"
                | "acos"
                | "asin"
                | "atan"
                | "atan2"
                | "cos"
                | "sin"
                | "tan"
                | "acosh"
                | "asinh"
                | "atanh"
                | "cosh"
                | "sinh"
                | "tanh"
                | "exp"
                | "frexp"
                | "ldexp"
                | "log"
                | "log10"
                | "modf"
                | "exp2"
                | "expm1"
                | "ilogb"
                | "log1p"
                | "log2"
                | "logb"
                | "scalbn"
                | "scalbln"
                | "pow"
                | "sqrt"
                | "cbrt"
                | "hypot"
                | "erf"
                | "erfc"
                | "tgamma"
                | "lgamma"
                | "fabs"
                | "ceil"
                | "floor"
                | "fmod"
                | "trunc"
                | "round"
                | "lround"
                | "llround"
                | "rint"
                | "lrint"
                | "llrint"
                | "nearbyint"
                | "remainder"
                | "remquo"
                | "copysign"
                | "nan"
                | "nextafter"
                | "nexttoward"
                | "fdim"
                | "fmax"
                | "fmin"
                | "fma"
        )
    }

    fn is_host_math_function(name: &str) -> bool {
        matches!(
            name,
            "__codex_math_errhandling"
                | "__codex_fpclassify"
                | "__codex_fpclassifyf"
                | "__codex_fpclassifyl"
                | "__codex_isfinite"
                | "__codex_isinf"
                | "__codex_isnan"
                | "__codex_isnormal"
                | "__codex_isnormalf"
                | "__codex_isnormall"
                | "__codex_signbit"
                | "__codex_isgreater"
                | "__codex_isgreaterequal"
                | "__codex_isless"
                | "__codex_islessequal"
                | "__codex_islessgreater"
                | "__codex_isunordered"
        ) || Self::is_host_math_base_function(name)
            || name
                .strip_suffix(['f', 'l'])
                .is_some_and(Self::is_host_math_base_function)
    }

    pub(super) fn complex_function_descriptor(
        name: &str,
    ) -> Option<(ComplexFunctionKind, CType, &str)> {
        let classify = |base: &str| match base {
            "__codex_cmplx" => Some(ComplexFunctionKind::Constructor),
            "cacos" | "casin" | "catan" | "ccos" | "csin" | "ctan" | "cacosh" | "casinh"
            | "catanh" | "ccosh" | "csinh" | "ctanh" | "cexp" | "clog" | "conj" | "cproj"
            | "csqrt" => Some(ComplexFunctionKind::UnaryComplex),
            "cpow" => Some(ComplexFunctionKind::BinaryComplex),
            "cabs" | "carg" | "cimag" | "creal" => Some(ComplexFunctionKind::UnaryReal),
            _ => None,
        };
        if let Some(kind) = classify(name) {
            return Some((kind, CType::Double, name));
        }
        if let Some(base) = name.strip_suffix('f')
            && let Some(kind) = classify(base)
        {
            return Some((kind, CType::Float, base));
        }
        if let Some(base) = name.strip_suffix('l')
            && let Some(kind) = classify(base)
        {
            return Some((kind, CType::LongDouble, base));
        }
        None
    }

    pub(super) fn is_host_complex_function(name: &str) -> bool {
        Self::complex_function_descriptor(name).is_some()
    }

    pub(super) fn is_host_library_function(name: &str) -> bool {
        matches!(
            name,
            "remove"
                | "rename"
                | "tmpfile"
                | "tmpnam"
                | "fclose"
                | "fflush"
                | "fopen"
                | "freopen"
                | "setbuf"
                | "setvbuf"
                | "fprintf"
                | "fscanf"
                | "printf"
                | "scanf"
                | "snprintf"
                | "sprintf"
                | "sscanf"
                | "vfprintf"
                | "vfscanf"
                | "vprintf"
                | "vscanf"
                | "vsnprintf"
                | "vsprintf"
                | "vsscanf"
                | "fgetc"
                | "fgets"
                | "fputc"
                | "fputs"
                | "getc"
                | "getchar"
                | "gets"
                | "putc"
                | "putchar"
                | "puts"
                | "ungetc"
                | "fread"
                | "fwrite"
                | "fgetpos"
                | "fseek"
                | "fsetpos"
                | "ftell"
                | "rewind"
                | "clearerr"
                | "feof"
                | "ferror"
                | "perror"
                | "__codex_assert_fail"
                | "__codex_errno_location"
                | "isalnum"
                | "isalpha"
                | "isblank"
                | "iscntrl"
                | "isdigit"
                | "isgraph"
                | "islower"
                | "isprint"
                | "ispunct"
                | "isspace"
                | "isupper"
                | "isxdigit"
                | "tolower"
                | "toupper"
                | "iswalnum"
                | "iswalpha"
                | "iswblank"
                | "iswcntrl"
                | "iswdigit"
                | "iswgraph"
                | "iswlower"
                | "iswprint"
                | "iswpunct"
                | "iswspace"
                | "iswupper"
                | "iswxdigit"
                | "iswctype"
                | "towlower"
                | "towupper"
                | "towctrans"
                | "wctrans"
                | "wctype"
                | "abs"
                | "labs"
                | "llabs"
                | "__codex_mb_cur_max"
                | "atof"
                | "atoi"
                | "atol"
                | "atoll"
                | "btowc"
                | "wctob"
                | "mbsinit"
                | "mblen"
                | "mbrlen"
                | "mbrtowc"
                | "mbtowc"
                | "wcrtomb"
                | "mbrtoc16"
                | "c16rtomb"
                | "mbrtoc32"
                | "c32rtomb"
                | "wctomb"
                | "mbstowcs"
                | "mbsrtowcs"
                | "wcstombs"
                | "wcsrtombs"
                | "fwprintf"
                | "fwscanf"
                | "swprintf"
                | "swscanf"
                | "vfwprintf"
                | "vfwscanf"
                | "vswprintf"
                | "vswscanf"
                | "vwprintf"
                | "vwscanf"
                | "wprintf"
                | "wscanf"
                | "fgetwc"
                | "fgetws"
                | "fputwc"
                | "fputws"
                | "fwide"
                | "getwc"
                | "getwchar"
                | "putwc"
                | "putwchar"
                | "ungetwc"
                | "wcstof"
                | "wcstod"
                | "wcstold"
                | "wcstol"
                | "wcstoll"
                | "wcstoul"
                | "wcstoull"
                | "wcscpy"
                | "wcsncpy"
                | "wmemcpy"
                | "wmemmove"
                | "wcscat"
                | "wcsncat"
                | "wcslen"
                | "wcscmp"
                | "wcscoll"
                | "wcsncmp"
                | "wcsxfrm"
                | "wcschr"
                | "wcsrchr"
                | "wcsspn"
                | "wcscspn"
                | "wcspbrk"
                | "wcsstr"
                | "wmemchr"
                | "wmemcmp"
                | "wmemset"
                | "wcstok"
                | "wcsftime"
                | "strtof"
                | "strtod"
                | "strtold"
                | "strtol"
                | "strtoul"
                | "strtoll"
                | "strtoull"
                | "rand"
                | "srand"
                | "div"
                | "ldiv"
                | "lldiv"
                | "imaxabs"
                | "imaxdiv"
                | "strtoimax"
                | "strtoumax"
                | "malloc"
                | "aligned_alloc"
                | "calloc"
                | "realloc"
                | "free"
                | "abort"
                | "atexit"
                | "at_quick_exit"
                | "exit"
                | "quick_exit"
                | "_Exit"
                | "bsearch"
                | "qsort"
                | "getenv"
                | "system"
                | "setlocale"
                | "localeconv"
                | "signal"
                | "raise"
                | "__codex_sig_dfl"
                | "__codex_sig_ign"
                | "__codex_sig_err"
                | "clock"
                | "difftime"
                | "mktime"
                | "time"
                | "asctime"
                | "ctime"
                | "gmtime"
                | "localtime"
                | "strftime"
                | "timespec_get"
                | "feclearexcept"
                | "fegetexceptflag"
                | "feraiseexcept"
                | "fesetexceptflag"
                | "fetestexcept"
                | "fegetround"
                | "fesetround"
                | "fegetenv"
                | "feholdexcept"
                | "fesetenv"
                | "feupdateenv"
                | "__codex_fe_dfl_env"
                | "memcpy"
                | "memmove"
                | "memset"
                | "memchr"
                | "memcmp"
                | "__codex_setjmp"
                | "longjmp"
                | "strlen"
                | "strcmp"
                | "strncmp"
                | "strcoll"
                | "strcpy"
                | "strncpy"
                | "strcat"
                | "strncat"
                | "strxfrm"
                | "strerror"
                | "strdup"
                | "strchr"
                | "strrchr"
                | "strspn"
                | "strcspn"
                | "strpbrk"
                | "strstr"
                | "strtok"
        ) || Self::is_host_math_function(name)
            || Self::is_host_complex_function(name)
    }

    pub fn new(
        sources: &'a SourceManager,
        program: TranslationUnit,
        run_options: &RunOptions,
    ) -> Self {
        install_control_flow_panic_hook();
        let mut functions = HashMap::default();
        let mut internal_functions: FileScopedMap<Rc<FunctionDef>> = HashMap::default();
        let mut declared_functions = HashMap::default();
        let mut internal_declared_functions: FileScopedMap<FunctionDecl> = HashMap::default();
        let mut function_symbols = HashMap::default();
        let mut function_may_setjmp = HashMap::default();
        let mut declared_globals = HashMap::default();
        let mut internal_declared_globals: FileScopedMap<Declaration> = HashMap::default();
        let function_visible_global_declarations =
            Self::collect_function_visible_global_declarations(&program);
        let restrict_tracking_required = program
            .globals
            .iter()
            .chain(&program.global_definitions)
            .any(|declaration| declaration.ty.top_level_qualifiers().is_restrict)
            || program
                .functions
                .iter()
                .chain(&program.inline_function_definitions)
                .any(function_contains_restrict_declaration);
        for function in &program.function_declarations {
            if function.storage_class == Some(StorageClass::Static) {
                internal_declared_functions
                    .entry(function.span.file)
                    .or_default()
                    .insert(function.name.clone(), function.clone());
            } else {
                declared_functions.insert(function.name.clone(), function.clone());
            }
        }
        for function in &program.functions {
            let function = Rc::new(function.clone());
            let symbol = function_symbol(function.as_ref());
            function_may_setjmp.insert(
                symbol.clone(),
                function_body_contains_setjmp(&function.body),
            );
            function_symbols.insert(symbol, Rc::clone(&function));
            if function.storage_class == Some(StorageClass::Static) {
                internal_functions
                    .entry(function.span.file)
                    .or_default()
                    .insert(function.name.clone(), Rc::clone(&function));
            } else {
                functions.insert(function.name.clone(), function);
            }
        }
        for global in &program.globals {
            if global.storage_class == Some(StorageClass::Static) {
                internal_declared_globals
                    .entry(global.span.file)
                    .or_default()
                    .insert(global.name.clone(), global.clone());
            } else {
                declared_globals.insert(global.name.clone(), global.clone());
            }
        }
        let mut next_encoded_pointer = run_options.synthetic_address_base.max(1);
        let mut encoded_function_pointers = HashMap::default();
        let mut decoded_pointers = HashMap::default();
        let mut addressable_functions = function_symbols.keys().cloned().collect::<Vec<_>>();
        addressable_functions.extend(
            declared_functions
                .keys()
                .filter(|name| Self::is_host_library_function(name))
                .cloned(),
        );
        for declarations in internal_declared_functions.values() {
            addressable_functions.extend(
                declarations
                    .keys()
                    .filter(|name| Self::is_host_library_function(name))
                    .cloned(),
            );
        }
        addressable_functions.sort_unstable();
        addressable_functions.dedup();
        for name in addressable_functions {
            let address = next_encoded_pointer;
            next_encoded_pointer = next_encoded_pointer.saturating_add(0x1000);
            encoded_function_pointers.insert(name.clone(), address);
            decoded_pointers.insert(address, vec![EncodedPointer::Function(name)]);
        }
        let mut startup_fenv = HostFEnv { opaque: [0; 16] };
        let _ = unsafe { fegetenv(&mut startup_fenv) };
        Self {
            sources,
            program,
            run_options: run_options.clone(),
            functions,
            internal_functions,
            declared_functions,
            internal_declared_functions,
            function_symbols,
            function_may_setjmp,
            declared_globals,
            internal_declared_globals,
            function_visible_global_declarations,
            global_bindings: HashMap::default(),
            internal_global_bindings: HashMap::default(),
            local_static_bindings: HashMap::default(),
            automatic_object_bindings: HashMap::default(),
            compound_literal_bindings: HashMap::default(),
            full_expression_temporaries: Vec::new(),
            next_object: 0,
            reusable_automatic_object_ids: Vec::new(),
            retired_objects: HashMap::default(),
            live_non_dynamic_bytes: 0,
            expr_state: ExprState::default(),
            assignment_targets: Vec::new(),
            initializer_sequencing: None,
            restrict_trackers: Vec::new(),
            restrict_tracking_required,
            formatted_io_accesses: None,
            pending_stream_buffer_lifetime_ub: None,
            next_encoded_pointer,
            object_type_registry: HashMap::default(),
            object_base_addresses: HashMap::default(),
            dynamic_allocations: HashSet::default(),
            dynamic_bytes_allocated: 0,
            virtual_filesystem: VirtualFileSystem::new(),
            host_streams: HashMap::default(),
            configured_stream_buffers: 0,
            host_stdio_bindings: HashMap::default(),
            host_capture_streams: HashMap::default(),
            errno_binding: None,
            signgam_binding: None,
            tmpnam_binding: None,
            strerror_binding: None,
            time_text_binding: None,
            getenv_binding: None,
            locale_generation: 0,
            signal_handlers: HashMap::default(),
            fe_dfl_env_binding: None,
            startup_fenv: startup_fenv.opaque,
            fexcept_provenance: HashMap::default(),
            fenv_provenance: HashMap::default(),
            mbstate_provenance: HashMap::default(),
            internal_mbstate_generations: HashMap::default(),
            wcstok_state_provenance: HashMap::default(),
            fpos_provenance: HashMap::default(),
            ftell_provenance: HashSet::default(),
            pending_qsort_order: None,
            pending_bsearch_result: None,
            pending_heap_call: None,
            wctrans_descriptors: HashMap::default(),
            wctype_descriptors: HashMap::default(),
            atexit_handlers: Vec::new(),
            running_atexit: false,
            quick_exit_handlers: Vec::new(),
            running_quick_exit: false,
            func_name_bindings: HashMap::default(),
            mbrtoc16_pending: HashMap::default(),
            mbrtoc16_null_pending: None,
            c16rtomb_pending: HashMap::default(),
            c16rtomb_null_pending: None,
            encoded_object_pointers: HashMap::default(),
            encoded_function_pointers,
            decoded_pointers,
            opaque_integer_pointer_addresses: RefCell::new(HashSet::default()),
            current_variadic_args: Vec::new(),
            va_lists: HashMap::default(),
            next_va_list_handle: 1,
            current_functions: Vec::new(),
            current_frame_ids: Vec::new(),
            active_block_scopes: Vec::new(),
            next_frame_id: 1,
            setjmp_envs: HashMap::default(),
            setjmp_provenance: HashMap::default(),
            live_setjmp_frames: HashSet::default(),
            next_setjmp_handle: 1,
            active_setjmp_contexts: Vec::new(),
            expr_setjmp_cache: HashMap::default(),
            expr_side_effect_cache: HashMap::default(),
            switch_dispatch_cache: HashMap::default(),
            switch_entry_index_cache: HashMap::default(),
            type_contains_union_cache: HashMap::default(),
            variable_cache: HashMap::default(),
            member_access_cache: RefCell::new(HashMap::default()),
            float_display_cache: RefCell::new(HashMap::default()),
            pending_longjmp_return: None,
            host_library_runtime_depth: 0,
            active_switch_dispatch_depth: 0,
            strtok_state: None,
            strtok_started: false,
            cboxes_main_state: Vec::new(),
            cboxes_trace: Vec::new(),
            cboxes_expression_request: None,
            cboxes_expression_result: None,
            execution_steps_remaining: run_options.execution_step_limit,
            execution_steps_completed: 0,
        }
    }

    pub fn run(mut self) -> Result<ProgramOutput, Diagnostic> {
        let main = self
            .lookup_function("main", FileId(0))
            .cloned()
            .ok_or_else(|| {
                let representative_span = self
                    .program
                    .functions
                    .first()
                    .map(|function| function.span)
                    .or_else(|| {
                        self.program
                            .function_declarations
                            .first()
                            .map(|function| function.span)
                    })
                    .or_else(|| self.program.globals.first().map(|global| global.span));
                let entry_file = representative_span
                    .and_then(|span| {
                        let (path, ..) = self.sources.span_display_range(span);
                        self.sources.find_file(&path)
                    })
                    .unwrap_or(FileId(0));
                let end = self.sources.file(entry_file).text().trim_end().len();
                Diagnostic::error(
                    "program does not define main",
                    Span::new(entry_file, end, end),
                )
            })?;
        let mut objects = ObjectFrames::new();
        self.initialize_globals(&mut objects)?;
        self.initialize_host_stdio(&mut objects)?;
        self.initialize_host_math(&mut objects)?;
        self.initialize_host_errno(&mut objects)?;
        self.validate_program_constraints(&objects)?;
        let main_args = self.build_main_arguments(&main, &mut objects)?;
        let main_symbol = function_symbol(main.as_ref());
        let entry = catch_unwind(AssertUnwindSafe(|| {
            self.call_function(
                &main,
                self.function_may_setjmp_symbol(&main_symbol),
                main_args,
                main.span,
                &mut objects,
            )
        }));
        let (exit_status, blocked, execution_limit_span) = match entry {
            Ok(Ok(result)) => {
                let exit_status = if main.return_type == CType::Void {
                    0
                } else {
                    result
                        .to_int()
                        .map_err(|diag| self.with_cboxes_runtime_context(diag))?
                        as c_int
                };
                (exit_status, None, None)
            }
            Ok(Err(diag)) => {
                if let Some(span) = diag.execution_step_limit_span() {
                    (0, None, Some(span))
                } else {
                    let Some((function_name, span)) = diag.blocked_info() else {
                        return Err(self.with_cboxes_runtime_context(diag));
                    };
                    (
                        0,
                        Some(self.cboxes_blocked_result(function_name, span)),
                        None,
                    )
                }
            }
            Err(payload) => {
                if let Some(signal) = payload.downcast_ref::<TerminationSignal>().copied() {
                    let mut exit_status = signal.status;
                    let mut blocked = None;
                    let mut execution_limit_span = None;
                    if signal.run_atexit {
                        if let Err(diag) = self.take_pending_stream_buffer_lifetime_ub() {
                            return Err(self.with_cboxes_runtime_context(diag));
                        }
                        let handlers = catch_unwind(AssertUnwindSafe(|| {
                            self.run_atexit_handlers(&mut objects, main.span)
                        }));
                        match handlers {
                            Ok(Ok(())) => {}
                            Ok(Err(diag)) => {
                                if let Some(span) = diag.execution_step_limit_span() {
                                    execution_limit_span = Some(span);
                                } else {
                                    let Some((function_name, span)) = diag.blocked_info() else {
                                        return Err(self.with_cboxes_runtime_context(diag));
                                    };
                                    blocked = Some(self.cboxes_blocked_result(function_name, span));
                                }
                            }
                            Err(payload) => {
                                if let Some(signal) =
                                    payload.downcast_ref::<TerminationSignal>().copied()
                                {
                                    exit_status = signal.status;
                                } else {
                                    resume_unwind(payload);
                                }
                            }
                        }
                    } else if signal.run_quick_exit {
                        let handlers = catch_unwind(AssertUnwindSafe(|| {
                            self.run_quick_exit_handlers(&mut objects, main.span)
                        }));
                        match handlers {
                            Ok(Ok(())) => {}
                            Ok(Err(diag)) => {
                                return Err(self.with_cboxes_runtime_context(diag));
                            }
                            Err(payload) => {
                                if let Some(signal) =
                                    payload.downcast_ref::<TerminationSignal>().copied()
                                {
                                    exit_status = signal.status;
                                } else {
                                    resume_unwind(payload);
                                }
                            }
                        }
                    }
                    (exit_status, blocked, execution_limit_span)
                } else {
                    resume_unwind(payload);
                }
            }
        };
        let main_close_offset = main
            .body
            .span
            .end
            .saturating_sub(1)
            .max(main.body.span.start);
        let (main_close_file, main_close_line, _) = self.sources.span_location(Span::new(
            main.body.span.file,
            main_close_offset,
            main_close_offset.saturating_add(1),
        ));
        let (execution_limit, execution_limit_state) = if let Some(span) = execution_limit_span {
            let (limit, state) = self.rewind_cboxes_trace_before_repeated_execution(span);
            (Some(limit), Some(state))
        } else {
            (None, None)
        };
        let state = if let Some(blocked) = blocked.as_ref() {
            blocked.state.clone()
        } else if let Some(state) = execution_limit_state {
            state
        } else {
            self.cboxes_main_state.clone()
        };
        let stdout = self
            .capture_stream_contents(CaptureStream::Stdout)
            .map_err(|diag| self.with_cboxes_runtime_context(diag))?;
        let stderr = self
            .capture_stream_contents(CaptureStream::Stderr)
            .map_err(|diag| self.with_cboxes_runtime_context(diag))?;
        Ok(ProgramOutput {
            stdout,
            stderr,
            exit_status,
            state,
            trace: self.cboxes_trace.clone(),
            main_close: ProgramSourceLocation {
                file: main_close_file.display().to_string(),
                line: main_close_line.saturating_sub(1),
            },
            blocked,
            execution_limit,
            expression: self.cboxes_expression_result.clone(),
        })
    }

    pub(super) fn consume_execution_step(&mut self, span: Span) -> Result<(), Diagnostic> {
        self.take_pending_stream_buffer_lifetime_ub()?;
        let Some(remaining) = self.execution_steps_remaining.as_mut() else {
            return Ok(());
        };
        if *remaining == 0 {
            return Err(Diagnostic::execution_step_limit(span));
        }
        *remaining -= 1;
        self.execution_steps_completed += 1;
        Ok(())
    }

    fn with_cboxes_runtime_context(&self, diagnostic: Diagnostic) -> Diagnostic {
        if !self.run_options.capture_visualization {
            return diagnostic;
        }
        let diagnostic_location = diagnostic.span().map(|span| {
            let (file, start_line, _) = self.sources.span_location(span);
            (file.display().to_string(), start_line.saturating_sub(1))
        });
        let line_execution_count = diagnostic_location.and_then(|(file, line)| {
            let count = self
                .cboxes_trace
                .iter()
                .filter(|event| {
                    event.file == file && event.start_line <= line && line <= event.end_line
                })
                .count();
            (count > 0).then_some(count + 1)
        });
        let state = self
            .cboxes_trace
            .last()
            .map(|event| event.state.clone())
            .unwrap_or_else(|| self.cboxes_main_state.clone());
        diagnostic.with_runtime_context(DiagnosticRuntimeContext {
            executed_steps: self.execution_steps_completed,
            line_execution_count,
            state,
        })
    }

    fn rewind_cboxes_trace_before_repeated_execution(
        &mut self,
        fallback_span: Span,
    ) -> (ProgramExecutionLimit, Vec<ProgramStateBox>) {
        let repeated_start = {
            let mut locations: HashMap<(&str, usize, usize, &str), (usize, usize)> =
                HashMap::default();
            for (index, event) in self.cboxes_trace.iter().enumerate() {
                let stats = locations
                    .entry((
                        event.file.as_str(),
                        event.start_line,
                        event.end_line,
                        event.kind.as_str(),
                    ))
                    .or_insert((0, index));
                stats.0 += 1;
            }
            locations
                .values()
                .filter(|(count, _)| *count > 1)
                .copied()
                .max_by(|(left_count, left_first), (right_count, right_first)| {
                    left_count
                        .cmp(right_count)
                        .then_with(|| right_first.cmp(left_first))
                })
                .map(|(_, first)| first)
        };

        let stop_index = repeated_start.unwrap_or(self.cboxes_trace.len());
        let following_limit = self.run_options.execution_trace_following_limit;
        let location = self
            .cboxes_trace
            .get(stop_index)
            .map(|event| (event.file.clone(), event.start_line, event.end_line));
        let state_before = stop_index
            .checked_sub(1)
            .and_then(|index| self.cboxes_trace.get(index))
            .map(|event| event.state.clone())
            .unwrap_or_default();
        let trace_position = if stop_index > CBOXES_STEP_LIMIT_TRACE_PREFIX_CAP {
            let immediately_before = self.cboxes_trace[stop_index - 1].clone();
            let following_end = self
                .cboxes_trace
                .len()
                .min(stop_index.saturating_add(following_limit));
            let following = self.cboxes_trace[stop_index..following_end].to_vec();
            self.cboxes_trace
                .truncate(CBOXES_STEP_LIMIT_TRACE_PREFIX_CAP - 1);
            self.cboxes_trace.push(immediately_before);
            let trace_position = self.cboxes_trace.len();
            self.cboxes_trace.extend(following);
            trace_position
        } else {
            let following_end = self
                .cboxes_trace
                .len()
                .min(stop_index.saturating_add(following_limit));
            self.cboxes_trace.truncate(following_end);
            stop_index
        };

        let (file, start_line, end_line) = location.unwrap_or_else(|| {
            let (file, start_line, end_line) = self.sources.span_location(fallback_span);
            (
                file.display().to_string(),
                start_line.saturating_sub(1),
                end_line.saturating_sub(1),
            )
        });
        (
            ProgramExecutionLimit {
                file,
                start_line,
                end_line,
                trace_position,
            },
            state_before,
        )
    }

    pub fn set_cboxes_expression_eval(&mut self, request: ProgramExpressionEvalRequest) {
        self.cboxes_expression_request = Some(request);
    }

    fn cboxes_blocked_result(&self, function_name: &'static str, span: Span) -> ProgramBlocked {
        let (file, start_line, end_line) = self.sources.span_location(span);
        let state = self
            .cboxes_trace
            .last()
            .map(|event| event.state.clone())
            .unwrap_or_default();
        ProgramBlocked {
            file: file.display().to_string(),
            start_line: start_line.saturating_sub(1),
            end_line: end_line.saturating_sub(1),
            function: function_name.to_owned(),
            state,
        }
    }

    fn validate_program_constraints(&self, objects: &ObjectFrames) -> Result<(), Diagnostic> {
        for function in self
            .program
            .functions
            .iter()
            .chain(self.program.inline_function_definitions.iter())
        {
            self.validate_jump_scopes(function)?;
            let mut frame = Frame {
                id: usize::MAX,
                bindings: HashMap::default(),
                object_decls: HashMap::default(),
                function_decls: HashMap::default(),
            };
            if let Some(visible_globals) = self
                .function_visible_global_declarations
                .get(&function_symbol(function))
            {
                frame.object_decls.extend(visible_globals.clone());
            }
            frame.object_decls.insert(
                "__func__".to_owned(),
                Declaration {
                    name: "__func__".to_owned(),
                    ty: CType::array_of(
                        CType::qualified(
                            CType::Char,
                            crate::types::TypeQualifiers {
                                is_const: true,
                                ..crate::types::TypeQualifiers::default()
                            },
                        ),
                        function.name.len() + 1,
                    ),
                    vla_bounds: Vec::new(),
                    storage_class: Some(StorageClass::Static),
                    linkage: None,
                    alignment: None,
                    init: None,
                    declarator_span: function.span,
                    span: function.span,
                },
            );
            for param in &function.params {
                if param.ty == CType::Void {
                    continue;
                }
                if let Some(name) = &param.name {
                    let parameter_ty =
                        Self::resolve_vla_type_for_constraints(&param.ty, &param.vla_bounds).0;
                    frame.object_decls.insert(
                        name.clone(),
                        Declaration {
                            name: name.clone(),
                            ty: parameter_ty,
                            vla_bounds: param.vla_bounds.clone(),
                            storage_class: param.storage_class,
                            linkage: None,
                            alignment: None,
                            init: None,
                            declarator_span: param.span,
                            span: param.span,
                        },
                    );
                }
            }
            for param in &function.params {
                for bound in param.vla_bounds.iter().flatten() {
                    self.validate_full_expr_constraints(bound, &frame, objects)?;
                    let bound_ty = self.value_expr_type(bound, &frame, objects)?;
                    if !bound_ty.is_integer() {
                        return Err(Diagnostic::error(
                            "array bound must have integer type",
                            bound.span(),
                        ));
                    }
                }
                if let Some(bound) = &param.static_array_bound {
                    self.validate_full_expr_constraints(bound, &frame, objects)?;
                }
            }
            let mut context = ConstraintContext {
                return_type: &function.return_type,
                return_type_span: function.return_type_span,
                loop_depth: 0,
                switch_depth: 0,
            };
            self.validate_block_constraints(&function.body, &mut frame, objects, &mut context)?;
        }
        Ok(())
    }

    fn validate_jump_scopes(&self, function: &FunctionDef) -> Result<(), Diagnostic> {
        let mut validation = JumpScopeValidation::default();
        Self::collect_jump_scopes_in_block(
            &function.body,
            &HashSet::default(),
            &mut Vec::new(),
            &mut validation,
        );
        for (label, source_scope, _) in &validation.gotos {
            let Some((target_scope, _)) = validation.labels.get(label) else {
                continue;
            };
            if let Some(declaration_span) = target_scope.difference(source_scope).next() {
                return Err(Diagnostic::error(
                    "goto enters the scope of an object with variably modified type",
                    *declaration_span,
                )
                .with_note("the jump originates outside this variably modified object's scope"));
            }
        }
        for switch in &validation.switches {
            for (target_scope, _) in &switch.label_scopes {
                if let Some(declaration_span) = target_scope.difference(&switch.source_scope).next()
                {
                    return Err(Diagnostic::error(
                        "switch dispatch enters the scope of an object with variably modified type",
                        *declaration_span,
                    )
                    .with_note(
                        "the switch statement is outside this variably modified object's scope",
                    ));
                }
            }
        }
        Ok(())
    }

    fn collect_jump_scopes_in_block(
        block: &Block,
        inherited_scope: &HashSet<Span>,
        switch_stack: &mut Vec<usize>,
        validation: &mut JumpScopeValidation,
    ) {
        let mut scope = inherited_scope.clone();
        for item in &block.items {
            match item {
                BlockItem::Declaration(decl) => {
                    if decl.vla_bounds.iter().any(Option::is_some) {
                        scope.insert(decl.span);
                    }
                }
                BlockItem::FunctionDeclaration(_) => {}
                BlockItem::Statement(statement) => Self::collect_jump_scopes_in_statement(
                    statement,
                    &scope,
                    switch_stack,
                    validation,
                ),
            }
        }
    }

    fn collect_jump_scopes_in_statement(
        statement: &Statement,
        scope: &HashSet<Span>,
        switch_stack: &mut Vec<usize>,
        validation: &mut JumpScopeValidation,
    ) {
        match statement {
            Statement::Block(block) => {
                Self::collect_jump_scopes_in_block(block, scope, switch_stack, validation)
            }
            Statement::DoWhile { body, .. } | Statement::While { body, .. } => {
                Self::collect_jump_scopes_in_statement(body, scope, switch_stack, validation)
            }
            Statement::For { init, body, .. } => {
                let mut for_scope = scope.clone();
                if let Some(ForInit::Declarations(decls)) = init {
                    for decl in decls {
                        if decl.vla_bounds.iter().any(Option::is_some) {
                            for_scope.insert(decl.span);
                        }
                    }
                }
                Self::collect_jump_scopes_in_statement(body, &for_scope, switch_stack, validation);
            }
            Statement::Goto { label, span } => {
                validation.gotos.push((label.clone(), scope.clone(), *span));
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_jump_scopes_in_statement(
                    then_branch,
                    scope,
                    switch_stack,
                    validation,
                );
                if let Some(else_branch) = else_branch {
                    Self::collect_jump_scopes_in_statement(
                        else_branch,
                        scope,
                        switch_stack,
                        validation,
                    );
                }
            }
            Statement::Labeled {
                statement, span, ..
            } => {
                if let Some(switch_index) = switch_stack.last().copied() {
                    validation.switches[switch_index]
                        .label_scopes
                        .push((scope.clone(), *span));
                }
                Self::collect_jump_scopes_in_statement(statement, scope, switch_stack, validation);
            }
            Statement::Switch { body, .. } => {
                let switch_index = validation.switches.len();
                validation.switches.push(SwitchScopeValidation {
                    source_scope: scope.clone(),
                    label_scopes: Vec::new(),
                });
                switch_stack.push(switch_index);
                Self::collect_jump_scopes_in_block(body, scope, switch_stack, validation);
                switch_stack.pop();
            }
            Statement::UserLabeled {
                label,
                statement,
                span,
            } => {
                validation
                    .labels
                    .insert(label.clone(), (scope.clone(), *span));
                Self::collect_jump_scopes_in_statement(statement, scope, switch_stack, validation);
            }
            Statement::Break(..)
            | Statement::Continue(..)
            | Statement::Expression(..)
            | Statement::Return(..) => {}
        }
    }

    fn validate_block_constraints(
        &self,
        block: &Block,
        frame: &mut Frame,
        objects: &ObjectFrames,
        context: &mut ConstraintContext<'_>,
    ) -> Result<(), Diagnostic> {
        let saved_object_decls = frame.object_decls.clone();
        let saved_function_decls = frame.function_decls.clone();
        let result = (|| {
            for item in &block.items {
                match item {
                    BlockItem::Declaration(decl) => {
                        if self.array_element_type_is_incomplete(&decl.ty, &decl.vla_bounds) {
                            let CType::Array(inner, _) = decl.ty.unqualified() else {
                                unreachable!();
                            };
                            return Err(Diagnostic::error(
                                format!("array element has incomplete type {inner}"),
                                decl.span,
                            ));
                        }
                        for bound in decl.vla_bounds.iter().flatten() {
                            self.validate_full_expr_constraints(bound, frame, objects)?;
                            if !self.value_expr_type(bound, frame, objects)?.is_integer() {
                                return Err(Diagnostic::error(
                                    "array bound must have integer type",
                                    bound.span(),
                                ));
                            }
                        }
                        let mut visible_decl = decl.clone();
                        visible_decl.ty = Self::resolve_vla_type_for_constraints(
                            &visible_decl.ty,
                            &visible_decl.vla_bounds,
                        )
                        .0;
                        if let Some(initializer) = &decl.init {
                            visible_decl.ty = self.complete_array_initializer_type(
                                &visible_decl.ty,
                                initializer,
                                frame,
                                objects,
                                false,
                            )?;
                        }
                        self.validate_declared_object_type(&visible_decl)?;
                        frame
                            .object_decls
                            .insert(decl.name.clone(), visible_decl.clone());
                        frame.bindings.remove(&decl.name);
                        frame.function_decls.remove(&decl.name);
                        if let Some(initializer) = &decl.init {
                            self.validate_initializer_constraints(
                                &visible_decl.ty,
                                initializer,
                                frame,
                                objects,
                            )?;
                        }
                    }
                    BlockItem::FunctionDeclaration(decl) => {
                        frame.object_decls.remove(&decl.name);
                        frame.bindings.remove(&decl.name);
                        frame.function_decls.insert(decl.name.clone(), decl.clone());
                    }
                    BlockItem::Statement(statement) => {
                        self.validate_statement_constraints(statement, frame, objects, context)?;
                    }
                }
            }
            Ok(())
        })();
        frame.object_decls = saved_object_decls;
        frame.function_decls = saved_function_decls;
        result
    }

    fn validate_statement_constraints(
        &self,
        statement: &Statement,
        frame: &mut Frame,
        objects: &ObjectFrames,
        context: &mut ConstraintContext<'_>,
    ) -> Result<(), Diagnostic> {
        match statement {
            Statement::Block(block) => {
                self.validate_block_constraints(block, frame, objects, context)
            }
            Statement::Break(span) => {
                if context.loop_depth == 0 && context.switch_depth == 0 {
                    Err(Diagnostic::error(
                        "break statement is not within a loop or switch",
                        *span,
                    ))
                } else {
                    Ok(())
                }
            }
            Statement::Continue(span) => {
                if context.loop_depth == 0 {
                    Err(Diagnostic::error(
                        "continue statement is not within a loop",
                        *span,
                    ))
                } else {
                    Ok(())
                }
            }
            Statement::DoWhile {
                body, condition, ..
            }
            | Statement::While {
                condition, body, ..
            } => {
                self.validate_full_expr_constraints(condition, frame, objects)?;
                self.require_scalar_expr(condition, condition.span(), frame, objects)?;
                context.loop_depth += 1;
                let result = self.validate_statement_constraints(body, frame, objects, context);
                context.loop_depth -= 1;
                result
            }
            Statement::Expression(expr, _) => {
                if let Some(expr) = expr {
                    self.validate_full_expr_constraints(expr, frame, objects)?;
                }
                Ok(())
            }
            Statement::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                let saved_object_decls = frame.object_decls.clone();
                let saved_function_decls = frame.function_decls.clone();
                let result = (|| {
                    if let Some(init) = init {
                        match init {
                            ForInit::Declarations(decls) => {
                                for decl in decls {
                                    if self.array_element_type_is_incomplete(
                                        &decl.ty,
                                        &decl.vla_bounds,
                                    ) {
                                        let CType::Array(inner, _) = decl.ty.unqualified() else {
                                            unreachable!();
                                        };
                                        return Err(Diagnostic::error(
                                            format!("array element has incomplete type {inner}"),
                                            decl.span,
                                        ));
                                    }
                                    for bound in decl.vla_bounds.iter().flatten() {
                                        self.validate_full_expr_constraints(bound, frame, objects)?;
                                    }
                                    let mut visible_decl = decl.clone();
                                    visible_decl.ty = Self::resolve_vla_type_for_constraints(
                                        &visible_decl.ty,
                                        &visible_decl.vla_bounds,
                                    )
                                    .0;
                                    if let Some(initializer) = &decl.init {
                                        visible_decl.ty = self.complete_array_initializer_type(
                                            &visible_decl.ty,
                                            initializer,
                                            frame,
                                            objects,
                                            false,
                                        )?;
                                    }
                                    self.validate_declared_object_type(&visible_decl)?;
                                    frame
                                        .object_decls
                                        .insert(decl.name.clone(), visible_decl.clone());
                                    frame.function_decls.remove(&decl.name);
                                    if let Some(initializer) = &decl.init {
                                        self.validate_initializer_constraints(
                                            &visible_decl.ty,
                                            initializer,
                                            frame,
                                            objects,
                                        )?;
                                    }
                                }
                            }
                            ForInit::Expression(expr) => {
                                self.validate_full_expr_constraints(expr, frame, objects)?;
                            }
                        }
                    }
                    if let Some(condition) = condition {
                        self.validate_full_expr_constraints(condition, frame, objects)?;
                        self.require_scalar_expr(condition, condition.span(), frame, objects)?;
                    }
                    if let Some(step) = step {
                        self.validate_full_expr_constraints(step, frame, objects)?;
                    }
                    context.loop_depth += 1;
                    let body_result =
                        self.validate_statement_constraints(body, frame, objects, context);
                    context.loop_depth -= 1;
                    body_result
                })();
                frame.object_decls = saved_object_decls;
                frame.function_decls = saved_function_decls;
                result
            }
            Statement::Goto { .. } => Ok(()),
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.validate_full_expr_constraints(condition, frame, objects)?;
                self.require_scalar_expr(condition, condition.span(), frame, objects)?;
                self.validate_statement_constraints(then_branch, frame, objects, context)?;
                if let Some(else_branch) = else_branch {
                    self.validate_statement_constraints(else_branch, frame, objects, context)?;
                }
                Ok(())
            }
            Statement::Labeled {
                statement, span, ..
            } => {
                if context.switch_depth == 0 {
                    return Err(Diagnostic::error(
                        "case/default label is not within a switch statement",
                        *span,
                    ));
                }
                self.validate_statement_constraints(statement, frame, objects, context)
            }
            Statement::Return(expr, span) => match (context.return_type.unqualified(), expr) {
                (CType::Void, None) => Ok(()),
                (CType::Void, Some(_)) => Err(Diagnostic::error(
                    "a void function cannot return a value",
                    *span,
                )),
                (_, None) => Err(Diagnostic::error(
                    "a non-void function must return a value in a return statement",
                    *span,
                )),
                (_, Some(expr)) => {
                    self.validate_full_expr_constraints(expr, frame, objects)?;
                    self.validate_implicit_conversion(
                        expr,
                        context.return_type,
                        frame,
                        objects,
                        expr.span(),
                    )
                    .map_err(|diagnostic| {
                        diagnostic.with_related_span(
                            "destination",
                            "the function's return type is declared here",
                            context.return_type_span,
                        )
                    })
                }
            },
            Statement::Switch { expr, body, .. } => {
                self.validate_full_expr_constraints(expr, frame, objects)?;
                let control_ty = self.value_expr_type(expr, frame, objects)?;
                if !control_ty.is_integer() {
                    return Err(Diagnostic::error(
                        "switch expression must have integer type",
                        expr.span(),
                    ));
                }
                let control_ty = self.promoted_integer_expr_type(expr, frame, objects)?;
                let mut validation_objects = objects.clone();
                self.validate_switch_dispatch(body, &control_ty, frame, &mut validation_objects)?;
                context.switch_depth += 1;
                let result = self.validate_block_constraints(body, frame, objects, context);
                context.switch_depth -= 1;
                result
            }
            Statement::UserLabeled { statement, .. } => {
                self.validate_statement_constraints(statement, frame, objects, context)
            }
        }
    }

    pub(super) fn validate_initializer_constraints(
        &self,
        target: &CType,
        initializer: &Initializer,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let initializer = self.unwrap_braced_string_initializer(target, initializer);
        match initializer {
            Initializer::Expr(expr) => {
                self.validate_full_expr_constraints(expr, frame, objects)?;
                if let (CType::Array(inner, len), Expr::StringLiteral(text, span)) =
                    (target.unqualified(), expr)
                    && inner.is_character()
                {
                    self.string_literal_array_initializer(inner, *len, text, *span)?;
                    return Ok(());
                }
                if let (CType::Array(inner, len), Expr::WideStringLiteral(text, span)) =
                    (target.unqualified(), expr)
                    && *inner.unqualified() == self.wchar_type()
                {
                    self.wide_string_literal_array_initializer(*len, text, *span)?;
                    return Ok(());
                }
                if let CType::Array(inner, len) = target.unqualified() {
                    let unicode = match expr {
                        Expr::Utf16StringLiteral(text, span)
                            if *inner.unqualified() == CType::UnsignedShort =>
                        {
                            Some((text, true, *span))
                        }
                        Expr::Utf32StringLiteral(text, span)
                            if *inner.unqualified() == CType::UnsignedInt =>
                        {
                            Some((text, false, *span))
                        }
                        _ => None,
                    };
                    if let Some((text, utf16, span)) = unicode {
                        self.unicode_string_literal_array_initializer(*len, text, utf16, span)?;
                        return Ok(());
                    }
                }
                if let CType::Array(inner, _) = target.unqualified() {
                    let required_form = if inner.is_character() {
                        "a string literal or a brace-enclosed list"
                    } else {
                        "a brace-enclosed list"
                    };
                    let literal_kind = match expr {
                        Expr::StringLiteral(..) => Some(("an ordinary string literal", "char")),
                        Expr::WideStringLiteral(..) => Some(("a wide string literal", "wchar_t")),
                        Expr::Utf16StringLiteral(..) => {
                            Some(("a UTF-16 string literal", "char16_t"))
                        }
                        Expr::Utf32StringLiteral(..) => {
                            Some(("a UTF-32 string literal", "char32_t"))
                        }
                        _ => None,
                    };
                    if let Some((literal_kind, element_type)) = literal_kind {
                        return Err(Diagnostic::error(
                            format!(
                                "cannot initialize {} with {}; use a brace-enclosed list or change the array element type to {}",
                                target, literal_kind, element_type
                            ),
                            expr.span(),
                        ));
                    }
                    if matches!(
                        self.expr_type(expr, frame, objects)?.unqualified(),
                        CType::Array(..)
                    ) {
                        return Err(Diagnostic::error(
                            "an array cannot be initialized by copying another array; use a brace-enclosed list instead",
                            expr.span(),
                        ));
                    }
                    let source = self.value_expr_type(expr, frame, objects)?;
                    return Err(Diagnostic::error(
                        format!(
                            "cannot convert {} to {}; an array of this type must use {}",
                            source, target, required_form
                        ),
                        expr.span(),
                    ));
                }
                self.validate_implicit_conversion(expr, target, frame, objects, expr.span())
            }
            Initializer::List { items, .. } => {
                if !matches!(
                    target.unqualified(),
                    CType::Array(..) | CType::Struct(..) | CType::Union(..)
                ) {
                    if items.len() != 1 || !items[0].designators.is_empty() {
                        let error_span = if items.len() > 1 {
                            items[1].span
                        } else {
                            items[0].span
                        };
                        return Err(Diagnostic::error(
                            "scalar initializer list must contain a single un-designated initializer",
                            error_span,
                        ));
                    }
                    return self.validate_initializer_constraints(
                        target,
                        &items[0].initializer,
                        frame,
                        objects,
                    );
                }
                let used = self.validate_initializer_sequence(target, items, frame, objects)?;
                if used != items.len() {
                    return Err(Diagnostic::error(
                        "too many initializer elements",
                        items[used].span,
                    ));
                }
                Ok(())
            }
        }
    }

    fn validate_initializer_items_into_type(
        &self,
        target: &CType,
        items: &[crate::ast::InitializerItem],
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        let Some(first) = items.first() else {
            return Ok(0);
        };
        if first.designators.is_empty() {
            if let Initializer::Expr(expr) = &first.initializer {
                let expr_ty = self.expr_type(expr, frame, objects)?;
                if matches!(target.unqualified(), CType::Struct(..) | CType::Union(..))
                    && self.cross_unit_tagged_type_compatible(
                        target.unqualified(),
                        expr_ty.unqualified(),
                    )
                {
                    self.validate_initializer_constraints(
                        target,
                        &first.initializer,
                        frame,
                        objects,
                    )?;
                    return Ok(1);
                }
                if let CType::Array(inner, _) = target.unqualified()
                    && ((matches!(expr, Expr::StringLiteral(..)) && inner.is_character())
                        || (matches!(expr, Expr::WideStringLiteral(..))
                            && *inner.unqualified() == self.wchar_type())
                        || (matches!(expr, Expr::Utf16StringLiteral(..))
                            && *inner.unqualified() == CType::UnsignedShort)
                        || (matches!(expr, Expr::Utf32StringLiteral(..))
                            && *inner.unqualified() == CType::UnsignedInt))
                {
                    self.validate_initializer_constraints(
                        target,
                        &first.initializer,
                        frame,
                        objects,
                    )?;
                    return Ok(1);
                }
            }
            if matches!(first.initializer, Initializer::List { .. }) {
                self.validate_initializer_constraints(target, &first.initializer, frame, objects)?;
                return Ok(1);
            }
        }
        match target.unqualified() {
            CType::Array(..) | CType::Struct(..) | CType::Union(..) => {
                self.validate_initializer_sequence(target, items, frame, objects)
            }
            _ => {
                if !first.designators.is_empty() {
                    return Err(Diagnostic::error(
                        "designators require an aggregate or union initializer",
                        first.span,
                    ));
                }
                self.validate_initializer_constraints(target, &first.initializer, frame, objects)?;
                Ok(1)
            }
        }
    }

    fn validate_initializer_sequence(
        &self,
        target: &CType,
        items: &[crate::ast::InitializerItem],
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        if items.is_empty() {
            return Ok(0);
        }
        match target.unqualified() {
            CType::Array(inner, len) => {
                let mut item_index = 0;
                let mut current = 0;
                while item_index < items.len() {
                    let item = &items[item_index];
                    if !item.designators.is_empty() {
                        let selectors =
                            self.initializer_selectors(target, &item.designators, item.span)?;
                        let Some((InitSelector::Index(index), _)) = selectors.split_first() else {
                            return Err(Diagnostic::error(
                                "array initializer designator must begin with [index]",
                                item.span,
                            ));
                        };
                        if *len != 0 && *index >= *len {
                            let designator_span = match &item.designators[0] {
                                Designator::Member(_, span) | Designator::Index(_, span) => *span,
                            };
                            return Err(Diagnostic::error(
                                "array designator is outside the bounds of the array",
                                designator_span,
                            ));
                        }
                        let selected =
                            self.initializer_selector_type(target, &selectors, item.span)?;
                        self.validate_initializer_constraints(
                            &selected,
                            &item.initializer,
                            frame,
                            objects,
                        )
                        .map_err(|diagnostic| {
                            match self.initializer_selected_record_member(target, &selectors) {
                                Some(member) => {
                                    Self::annotate_member_initializer_error(diagnostic, &member)
                                }
                                None => diagnostic,
                            }
                        })?;
                        current = index.saturating_add(1);
                        item_index += 1;
                        item_index += self.validate_after_designated_subobject(
                            inner,
                            &selectors[1..],
                            &items[item_index..],
                            frame,
                            objects,
                        )?;
                    } else {
                        if *len != 0 && current >= *len {
                            break;
                        }
                        let used = self.validate_initializer_items_into_type(
                            inner,
                            &items[item_index..],
                            frame,
                            objects,
                        )?;
                        item_index += used;
                        current += 1;
                    }
                }
                Ok(item_index)
            }
            CType::Struct(..) => {
                let positions = self.initializable_record_member_positions(target);
                let mut item_index = 0;
                let mut current = 0;
                while item_index < items.len() {
                    let item = &items[item_index];
                    if !item.designators.is_empty() {
                        let selectors =
                            self.initializer_selectors(target, &item.designators, item.span)?;
                        let Some((InitSelector::Member(name), _)) = selectors.split_first() else {
                            return Err(Diagnostic::error(
                                "structure initializer designator must begin with .member",
                                item.span,
                            ));
                        };
                        let top_pos = positions
                            .iter()
                            .position(|member| member.storage_name == *name)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "invalid structure initializer designator",
                                    item.span,
                                )
                            })?;
                        let selected =
                            self.initializer_selector_type(target, &selectors, item.span)?;
                        self.validate_initializer_constraints(
                            &selected,
                            &item.initializer,
                            frame,
                            objects,
                        )
                        .map_err(|diagnostic| {
                            match self.initializer_selected_record_member(target, &selectors) {
                                Some(member) => {
                                    Self::annotate_member_initializer_error(diagnostic, &member)
                                }
                                None => diagnostic,
                            }
                        })?;
                        current = top_pos + 1;
                        item_index += 1;
                        item_index += self.validate_after_designated_subobject(
                            &positions[top_pos].ty,
                            &selectors[1..],
                            &items[item_index..],
                            frame,
                            objects,
                        )?;
                    } else {
                        if current >= positions.len() {
                            break;
                        }
                        let member = &positions[current];
                        let used = self
                            .validate_initializer_items_into_type(
                                &member.ty,
                                &items[item_index..],
                                frame,
                                objects,
                            )
                            .map_err(|diagnostic| {
                                Self::annotate_member_initializer_error(diagnostic, member)
                            })?;
                        item_index += used;
                        current += 1;
                    }
                }
                Ok(item_index)
            }
            CType::Union(..) => {
                let item = &items[0];
                if item.designators.is_empty() {
                    let member =
                        self.first_initializable_union_member(target)
                            .ok_or_else(|| {
                                Diagnostic::error("union has no initializable members", item.span)
                            })?;
                    self.validate_initializer_items_into_type(&member.ty, items, frame, objects)
                        .map_err(|diagnostic| {
                            Self::annotate_member_initializer_error(diagnostic, &member)
                        })
                } else {
                    let selectors =
                        self.initializer_selectors(target, &item.designators, item.span)?;
                    if !matches!(selectors.first(), Some(InitSelector::Member(_))) {
                        return Err(Diagnostic::error(
                            "union initializer designator must begin with .member",
                            item.span,
                        ));
                    }
                    let selected = self.initializer_selector_type(target, &selectors, item.span)?;
                    self.validate_initializer_constraints(
                        &selected,
                        &item.initializer,
                        frame,
                        objects,
                    )
                    .map_err(|diagnostic| {
                        match self.initializer_selected_record_member(target, &selectors) {
                            Some(member) => {
                                Self::annotate_member_initializer_error(diagnostic, &member)
                            }
                            None => diagnostic,
                        }
                    })?;
                    let member_ty =
                        self.initializer_selector_type(target, &selectors[..1], item.span)?;
                    Ok(1 + self.validate_after_designated_subobject(
                        &member_ty,
                        &selectors[1..],
                        &items[1..],
                        frame,
                        objects,
                    )?)
                }
            }
            _ => Err(Diagnostic::error(
                "initializer sequence requires an aggregate or union type",
                items[0].span,
            )),
        }
    }

    fn validate_after_designated_subobject(
        &self,
        target: &CType,
        selectors: &[InitSelector],
        items: &[crate::ast::InitializerItem],
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        let Some((first, rest)) = selectors.split_first() else {
            return Ok(0);
        };
        let count = items
            .iter()
            .position(|item| !item.designators.is_empty())
            .unwrap_or(items.len());
        let items = &items[..count];
        if items.is_empty() {
            return Ok(0);
        }
        let selected = self.initializer_selector_type(target, &selectors[..1], items[0].span)?;
        let mut used =
            self.validate_after_designated_subobject(&selected, rest, items, frame, objects)?;
        match (target.unqualified(), first) {
            (CType::Array(inner, len), InitSelector::Index(index)) => {
                for _ in index + 1..*len {
                    if used == items.len() {
                        break;
                    }
                    used += self.validate_initializer_items_into_type(
                        inner,
                        &items[used..],
                        frame,
                        objects,
                    )?;
                }
            }
            (CType::Struct(..), InitSelector::Member(name)) => {
                let members = self.initializable_record_member_positions(target);
                let index = members
                    .iter()
                    .position(|member| member.storage_name == *name)
                    .expect("validated initializer member");
                for member in members.iter().skip(index + 1) {
                    if used == items.len() {
                        break;
                    }
                    used += self
                        .validate_initializer_items_into_type(
                            &member.ty,
                            &items[used..],
                            frame,
                            objects,
                        )
                        .map_err(|diagnostic| {
                            Self::annotate_member_initializer_error(diagnostic, member)
                        })?;
                }
            }
            (CType::Union(..), InitSelector::Member(_)) => {}
            _ => unreachable!("validated initializer selector"),
        }
        Ok(used)
    }

    fn initializer_selector_type(
        &self,
        target: &CType,
        selectors: &[InitSelector],
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let Some((first, rest)) = selectors.split_first() else {
            return Ok(target.clone());
        };
        let selected = match first {
            InitSelector::Index(index) => match target.unqualified() {
                CType::Array(inner, len) if *len == 0 || *index < *len => (**inner).clone(),
                CType::Array(..) => {
                    return Err(Diagnostic::error(
                        "array designator is outside the bounds of the array",
                        span,
                    ));
                }
                _ => {
                    return Err(Diagnostic::error(
                        "array designator requires an array type",
                        span,
                    ));
                }
            },
            InitSelector::Member(name) => self
                .direct_member_by_storage_name(target, name)
                .map(|member| member.ty.clone())
                .ok_or_else(|| Diagnostic::error("invalid member initializer designator", span))?,
        };
        self.initializer_selector_type(&selected, rest, span)
    }

    fn require_scalar_expr(
        &self,
        expr: &Expr,
        diagnostic_span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let ty = self.value_expr_type(expr, frame, objects)?;
        if ty.is_arithmetic() || ty.is_pointer() {
            Ok(())
        } else {
            Err(Diagnostic::error(
                format!("expression of type {} is not scalar", ty),
                diagnostic_span,
            ))
        }
    }

    fn validate_declared_object_type(&self, decl: &Declaration) -> Result<(), Diagnostic> {
        if matches!(decl.ty.unqualified(), CType::Void | CType::Function(..)) {
            return Err(Diagnostic::error(
                format!("an object cannot have type {}", decl.ty),
                decl.declarator_span,
            ));
        }
        if decl.storage_class != Some(StorageClass::Extern) && !self.type_is_complete(&decl.ty) {
            return Err(Diagnostic::error(
                format!("object definition has incomplete type {}", decl.ty),
                decl.declarator_span,
            ));
        }
        Ok(())
    }

    fn validate_implicit_conversion(
        &self,
        expr: &Expr,
        target: &CType,
        frame: &Frame,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let source = self.value_expr_type(expr, frame, objects)?;
        let compatible = if target == &source {
            true
        } else if matches!(target.unqualified(), CType::Bool) {
            source.is_arithmetic() || source.is_pointer()
        } else if target.is_arithmetic() && source.is_arithmetic() {
            true
        } else if target.is_pointer() && source.is_pointer() {
            self.is_null_pointer_constant(expr, frame, objects)?
                || self.pointer_assignment_compatible(target, &source)
        } else if target.is_pointer() && source.is_integer() {
            self.is_null_pointer_constant(expr, frame, objects)?
        } else if matches!(target.unqualified(), CType::Struct(..) | CType::Union(..)) {
            self.cross_unit_tagged_type_compatible(target.unqualified(), source.unqualified())
        } else {
            false
        };
        if compatible {
            Ok(())
        } else {
            let message = if target.is_pointer() && source.is_integer() {
                if matches!(
                    expr,
                    Expr::CharLiteral(..)
                        | Expr::WideCharLiteral(..)
                        | Expr::Utf16CharLiteral(..)
                        | Expr::Utf32CharLiteral(..)
                ) {
                    format!("cannot convert a character constant to pointer type {target}")
                } else {
                    format!(
                        "cannot implicitly convert integer expression of type {source} to pointer type {target}; the expression is not a null pointer constant"
                    )
                }
            } else {
                format!("cannot convert {} to {}", source, target)
            };
            Err(Diagnostic::error(message, span))
        }
    }

    pub(super) fn expr_is_lvalue(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<bool, Diagnostic> {
        Ok(match expr {
            Expr::Variable(name, span) => {
                frame.object_decls.contains_key(name)
                    || self.lookup_global_declaration(name, span.file).is_some()
            }
            Expr::StringLiteral(..)
            | Expr::WideStringLiteral(..)
            | Expr::CompoundLiteral { .. }
            | Expr::Subscript { .. }
            | Expr::Unary {
                op: UnaryOp::Dereference,
                ..
            } => true,
            Expr::Member { base, .. } => self.expr_is_lvalue(base, frame, objects)?,
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => self.expr_is_lvalue(
                self.select_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                    frame,
                    objects,
                )?,
                frame,
                objects,
            )?,
            _ => false,
        })
    }

    fn expr_is_function_designator(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<bool, Diagnostic> {
        Ok(match expr {
            Expr::Variable(name, span) => {
                frame.function_decls.contains_key(name)
                    || self.lookup_function_declaration(name, span.file).is_some()
                    || self.lookup_function(name, span.file).is_some()
                    || self.builtin_function_type(name).is_some()
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => self.expr_is_function_designator(
                self.select_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                    frame,
                    objects,
                )?,
                frame,
                objects,
            )?,
            _ => false,
        })
    }

    pub(super) fn expr_refers_to_vla(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> bool {
        match expr {
            Expr::Variable(name, _) => {
                frame
                    .bindings
                    .get(name)
                    .and_then(|object| self.lookup_object(objects, *object))
                    .is_some_and(|state| state.variably_modified)
                    || frame
                        .object_decls
                        .get(name)
                        .is_some_and(|decl| decl.vla_bounds.iter().any(Option::is_some))
            }
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::SizeofExpr { expr, .. }
            | Expr::Cast { expr, .. } => self.expr_refers_to_vla(expr, frame, objects),
            Expr::Subscript { base, .. } | Expr::Member { base, .. } => {
                self.expr_refers_to_vla(base, frame, objects)
            }
            _ => false,
        }
    }

    fn array_element_type_is_incomplete(&self, ty: &CType, vla_bounds: &[Option<Expr>]) -> bool {
        let CType::Array(inner, outer_len) = ty.unqualified() else {
            return false;
        };
        let bounds = if *outer_len == 0 && !vla_bounds.is_empty() {
            &vla_bounds[1..]
        } else {
            vla_bounds
        };
        Self::type_has_unresolved_incomplete_array(inner, bounds).0
    }

    fn type_has_unresolved_incomplete_array(
        ty: &CType,
        vla_bounds: &[Option<Expr>],
    ) -> (bool, usize) {
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let (incomplete_here, used_here) = if *len == 0 {
                    (
                        !matches!(vla_bounds.first(), Some(Some(_))),
                        usize::from(!vla_bounds.is_empty()),
                    )
                } else {
                    (false, 0)
                };
                let (incomplete_inner, used_inner) =
                    Self::type_has_unresolved_incomplete_array(inner, &vla_bounds[used_here..]);
                (incomplete_here || incomplete_inner, used_here + used_inner)
            }
            _ => (false, 0),
        }
    }

    pub(super) fn resolve_vla_type_for_constraints(
        ty: &CType,
        vla_bounds: &[Option<Expr>],
    ) -> (CType, usize) {
        match ty {
            CType::Array(inner, len) => {
                let (resolved_len, used_here) = if *len == 0 && !vla_bounds.is_empty() {
                    (if vla_bounds[0].is_some() { 1 } else { 0 }, 1)
                } else {
                    (*len, 0)
                };
                let (inner, used_inner) =
                    Self::resolve_vla_type_for_constraints(inner, &vla_bounds[used_here..]);
                (CType::array_of(inner, resolved_len), used_here + used_inner)
            }
            CType::Pointer(inner) => {
                let (inner, used) = Self::resolve_vla_type_for_constraints(inner, vla_bounds);
                (CType::pointer_to(inner), used)
            }
            CType::Qualified(inner, qualifiers) => {
                let (inner, used) = Self::resolve_vla_type_for_constraints(inner, vla_bounds);
                (CType::qualified(inner, *qualifiers), used)
            }
            _ => (ty.clone(), 0),
        }
    }

    fn validate_expr_constraints(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
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
            | Expr::OffsetOf { .. } => {}
            Expr::Unary { op, expr, span } => {
                if *op == UnaryOp::AddressOf
                    && let Expr::Unary {
                        op: UnaryOp::Dereference,
                        expr: pointer_expr,
                        ..
                    } = expr.as_ref()
                {
                    self.validate_expr_constraints(pointer_expr, frame, objects)?;
                    if !self
                        .value_expr_type(pointer_expr, frame, objects)?
                        .is_pointer()
                    {
                        return Err(Diagnostic::error(
                            "the * operator requires a pointer",
                            *span,
                        ));
                    }
                    return Ok(());
                }
                self.validate_expr_constraints(expr, frame, objects)?;
                let ty = self.expr_type(expr, frame, objects)?;
                match op {
                    UnaryOp::AddressOf => {
                        if !self.expr_is_lvalue(expr, frame, objects)?
                            && !self.expr_is_function_designator(expr, frame, objects)?
                        {
                            return Err(Diagnostic::error(
                                "the & operator requires an object or function; a temporary value does not have an address",
                                *span,
                            ));
                        }
                        if self.expr_designates_bit_field(expr, frame, objects)? {
                            return Err(Diagnostic::error(
                                "cannot take the address of a bit-field",
                                *span,
                            ));
                        }
                        if let Expr::Variable(name, _) = expr.as_ref()
                            && frame.object_decls.get(name).is_some_and(|decl| {
                                decl.storage_class == Some(StorageClass::Register)
                            })
                        {
                            return Err(Diagnostic::error(
                                "cannot take the address of a register object",
                                *span,
                            ));
                        }
                    }
                    UnaryOp::LogicalNot => self.require_scalar_expr(expr, *span, frame, objects)?,
                    UnaryOp::PreIncrement | UnaryOp::PreDecrement => {
                        if !self.expr_is_lvalue(expr, frame, objects)?
                            || self.type_has_const_subobject(&ty)
                            || !(ty.is_integer() || ty.is_floating() || ty.is_pointer())
                        {
                            return Err(Diagnostic::error(
                                "++ and -- require a changeable arithmetic variable or pointer",
                                *span,
                            ));
                        }
                        if ty.is_pointer() {
                            self.require_complete_pointer_arithmetic_type(&ty, *span)?;
                        }
                    }
                    _ => {}
                }
            }
            Expr::Postfix { expr, span, .. } => {
                self.validate_expr_constraints(expr, frame, objects)?;
                let ty = self.expr_type(expr, frame, objects)?;
                if !self.expr_is_lvalue(expr, frame, objects)?
                    || self.type_has_const_subobject(&ty)
                    || !(ty.is_integer() || ty.is_floating() || ty.is_pointer())
                {
                    return Err(Diagnostic::error(
                        "++ and -- require a changeable arithmetic variable or pointer",
                        *span,
                    ));
                }
                if ty.is_pointer() {
                    self.require_complete_pointer_arithmetic_type(&ty, *span)?;
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.validate_expr_constraints(lhs, frame, objects)?;
                self.validate_expr_constraints(rhs, frame, objects)?;
            }
            Expr::Subscript { base, index, .. } => {
                self.validate_expr_constraints(base, frame, objects)?;
                self.validate_expr_constraints(index, frame, objects)?;
            }
            Expr::Assign { lhs, rhs, span } => {
                self.validate_expr_constraints(lhs, frame, objects)?;
                self.validate_expr_constraints(rhs, frame, objects)?;
                let lhs_ty = self.expr_type(lhs, frame, objects)?;
                if !self.expr_is_lvalue(lhs, frame, objects)? {
                    return Err(Diagnostic::error(
                        "the left side of = is not a stored object that can be changed",
                        *span,
                    ));
                }
                if matches!(lhs_ty.unqualified(), CType::Array(..)) {
                    return Err(Diagnostic::error(
                        "arrays cannot be assigned; assign their elements individually",
                        *span,
                    ));
                }
                if self.type_has_const_subobject(&lhs_ty) {
                    return Err(Diagnostic::error(
                        "the left side of = is const and cannot be changed",
                        *span,
                    ));
                }
                if matches!(lhs_ty.unqualified(), CType::Void) {
                    return Err(Diagnostic::error(
                        "the left side of = has type void and cannot store a value",
                        *span,
                    ));
                }
                self.validate_implicit_conversion(rhs, &lhs_ty, frame, objects, rhs.span())?;
            }
            Expr::CompoundAssign { op, lhs, rhs, span } => {
                self.validate_expr_constraints(lhs, frame, objects)?;
                self.validate_expr_constraints(rhs, frame, objects)?;
                let lhs_ty = self.expr_type(lhs, frame, objects)?;
                if !self.expr_is_lvalue(lhs, frame, objects)? {
                    return Err(Diagnostic::error(
                        "the left side of this assignment is not a stored object that can be changed",
                        *span,
                    ));
                }
                if matches!(lhs_ty.unqualified(), CType::Array(..)) {
                    return Err(Diagnostic::error(
                        "arrays cannot be assigned; assign their elements individually",
                        *span,
                    ));
                }
                if self.type_has_const_subobject(&lhs_ty) {
                    return Err(Diagnostic::error(
                        "the left side of this assignment is const and cannot be changed",
                        *span,
                    ));
                }
                if matches!(lhs_ty.unqualified(), CType::Void) {
                    return Err(Diagnostic::error(
                        "the left side of this assignment has type void and cannot store a value",
                        *span,
                    ));
                }
                let binary = Expr::Binary {
                    op: *op,
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    span: *span,
                };
                let _ = self.expr_type(&binary, frame, objects)?;
                self.validate_implicit_conversion(&binary, &lhs_ty, frame, objects, *span)?;
            }
            Expr::SizeofType {
                ty,
                vla_bounds,
                span,
            } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_expr_constraints(bound, frame, objects)?;
                }
                if vla_bounds.iter().all(Option::is_none) {
                    let _ = self.eval_sizeof_type(ty, *span)?;
                }
            }
            Expr::SizeofExpr { expr, span } => {
                self.validate_expr_constraints(expr, frame, objects)?;
                if self.expr_designates_bit_field(expr, frame, objects)? {
                    return Err(Diagnostic::error(
                        "sizeof cannot be applied to a bit-field",
                        *span,
                    ));
                }
                let ty = self.expr_type(expr, frame, objects)?;
                if !matches!(ty.unqualified(), CType::Array(..))
                    || !self.expr_refers_to_vla(expr, frame, objects)
                {
                    let _ = self.eval_sizeof_type(&ty, *span)?;
                }
            }
            Expr::Cast {
                ty,
                vla_bounds,
                expr,
                span,
            } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_expr_constraints(bound, frame, objects)?;
                }
                self.validate_expr_constraints(expr, frame, objects)?;
                let source = self.value_expr_type(expr, frame, objects)?;
                if !matches!(ty.unqualified(), CType::Void)
                    && (!(ty.is_arithmetic() || ty.is_pointer())
                        || !(source.is_arithmetic() || source.is_pointer()))
                {
                    return Err(Diagnostic::error(
                        format!("invalid cast from {} to {}", source, ty),
                        *span,
                    ));
                }
            }
            Expr::CompoundLiteral {
                ty,
                vla_bounds,
                initializer,
                ..
            } => {
                if matches!(ty.unqualified(), CType::Struct(..) | CType::Union(..))
                    && !self.type_is_complete(ty)
                {
                    return Err(Diagnostic::error(
                        format!("compound literal has incomplete type {ty}"),
                        expr.span(),
                    ));
                }
                if matches!(ty.unqualified(), CType::Array(..))
                    && vla_bounds.iter().any(Option::is_some)
                {
                    return Err(Diagnostic::error(
                        "compound literal cannot have variable length array type",
                        expr.span(),
                    ));
                }
                if let CType::Array(inner, _) = ty.unqualified()
                    && Self::has_incomplete_array_dimension(inner)
                {
                    return Err(Diagnostic::error(
                        format!("array element has incomplete type {inner}"),
                        expr.span(),
                    ));
                }
                for bound in vla_bounds.iter().flatten() {
                    self.validate_expr_constraints(bound, frame, objects)?;
                }
                let completed =
                    self.complete_compound_literal_type(ty, initializer, frame, objects)?;
                self.validate_initializer_constraints(&completed, initializer, frame, objects)?;
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                ..
            } => {
                self.validate_expr_constraints(control, frame, objects)?;
                for association in associations {
                    self.validate_expr_constraints(&association.expr, frame, objects)?;
                }
                if let Some(default) = default {
                    self.validate_expr_constraints(default, frame, objects)?;
                }
            }
            Expr::VaArg { ap, ty, span } => {
                self.validate_expr_constraints(ap, frame, objects)?;
                if !self.expr_is_lvalue(ap, frame, objects)?
                    || self.expr_type(ap, frame, objects)? != CType::VaList
                {
                    return Err(Diagnostic::error(
                        "va_arg requires a va_list lvalue",
                        ap.span(),
                    ));
                }
                if matches!(ty.unqualified(), CType::Void | CType::Function(..))
                    || !self.type_is_complete(ty)
                {
                    return Err(Diagnostic::error(
                        "va_arg requires a complete object type",
                        *span,
                    ));
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.validate_expr_constraints(condition, frame, objects)?;
                self.require_scalar_expr(condition, condition.span(), frame, objects)?;
                self.validate_expr_constraints(then_expr, frame, objects)?;
                self.validate_expr_constraints(else_expr, frame, objects)?;
            }
            Expr::Call {
                callee,
                args,
                declared_callee_type,
                span,
            } => {
                self.validate_expr_constraints(callee, frame, objects)?;
                for arg in args {
                    self.validate_expr_constraints(arg, frame, objects)?;
                }
                if matches!(callee.as_ref(), Expr::Variable(name, _) if matches!(name.as_str(), "va_start" | "va_end" | "va_copy"))
                {
                    let _ = self.expr_type(expr, frame, objects)?;
                    return Ok(());
                }
                let callee_ty = declared_callee_type
                    .clone()
                    .unwrap_or(self.value_expr_type(callee, frame, objects)?);
                let function_ty = match callee_ty.unqualified() {
                    CType::Function(..) => callee_ty,
                    CType::Pointer(inner) if matches!(inner.unqualified(), CType::Function(..)) => {
                        (**inner).clone()
                    }
                    _ => {
                        return Err(Diagnostic::error("call target is not a function", *span));
                    }
                };
                let CType::Function(_, params, variadic) = function_ty.unqualified() else {
                    unreachable!();
                };
                for arg in args {
                    let arg_ty = self.value_expr_type(arg, frame, objects)?;
                    if !self.type_is_complete(&arg_ty) {
                        return Err(Diagnostic::error(
                            format!(
                                "function arguments must have complete object type, but this argument has type {arg_ty}"
                            ),
                            arg.span(),
                        ));
                    }
                }
                if !params.is_empty() {
                    let fixed = if params.as_slice() == [CType::Void] {
                        0
                    } else {
                        params.len()
                    };
                    if (!variadic && args.len() != fixed) || (*variadic && args.len() < fixed) {
                        return Err(Diagnostic::error(
                            format!(
                                "function call expected {}{} argument(s), got {}",
                                fixed,
                                if *variadic { "+" } else { "" },
                                args.len()
                            ),
                            *span,
                        ));
                    }
                    let parameter_spans = self.call_parameter_spans(callee, frame);
                    for (index, (arg, param)) in
                        args.iter().zip(params.iter()).take(fixed).enumerate()
                    {
                        self.validate_implicit_conversion(arg, param, frame, objects, arg.span())
                            .map_err(|diagnostic| {
                                if let Some(span) = parameter_spans.get(index).copied() {
                                    diagnostic.with_related_span(
                                        "destination",
                                        "the parameter is declared here",
                                        span,
                                    )
                                } else {
                                    diagnostic
                                }
                            })?;
                    }
                }
            }
            Expr::Member { base, .. } => {
                self.validate_expr_constraints(base, frame, objects)?;
            }
        }
        let _ = self.expr_type(expr, frame, objects)?;
        Ok(())
    }
}
