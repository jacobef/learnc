use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn call_parameter_spans(&self, callee: &Expr, frame: &Frame) -> Vec<Span> {
        let Expr::Variable(name, span) = callee else {
            return Vec::new();
        };
        if let Some(declaration) = frame.function_decls.get(name) {
            return declaration.params.iter().map(|param| param.span).collect();
        }
        if let Some(function) = self.lookup_function(name, span.file) {
            return function.params.iter().map(|param| param.span).collect();
        }
        self.lookup_function_declaration(name, span.file)
            .map(|declaration| declaration.params.iter().map(|param| param.span).collect())
            .unwrap_or_default()
    }

    pub(super) fn initialize_globals(
        &mut self,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let globals = self.program.global_definitions.clone();
        let mut allocated = Vec::with_capacity(globals.len());
        for decl in &globals {
            if decl.vla_bounds.iter().any(|bound| bound.is_some()) {
                return Err(Diagnostic::error(
                    "file-scope declarations cannot have variable length array type",
                    decl.span,
                ));
            }
            if decl.storage_class == Some(StorageClass::Extern) && decl.init.is_none() {
                continue;
            }
            if matches!(decl.ty.unqualified(), CType::Void | CType::Function(..)) {
                return Err(Diagnostic::error(
                    format!("an object cannot have type {}", decl.ty),
                    decl.span,
                ));
            }
            if !self.type_is_complete(&decl.ty)
                && !matches!(decl.ty.unqualified(), CType::Array(_, 0) if decl.init.is_some())
            {
                return Err(Diagnostic::error(
                    format!("object definition has incomplete type {}", decl.ty),
                    decl.span,
                ));
            }
            let object = self.allocate_object(
                objects,
                decl.ty.clone(),
                StorageDuration::Static,
                decl.span,
                false,
                false,
            )?;
            self.ensure_object_alignment(object, &decl.ty, decl.alignment);
            if decl.storage_class == Some(StorageClass::Static) {
                self.internal_global_bindings
                    .entry(decl.span.file)
                    .or_default()
                    .insert(decl.name.clone(), object);
            } else {
                self.global_bindings.insert(decl.name.clone(), object);
            }
            allocated.push((object, decl.clone()));
        }
        // Keep the cloned declarations at stable addresses for the entire pass. Expression
        // caches use AST addresses as keys, so moving each declaration through the same loop
        // variable would make unrelated global initializers share cache entries.
        for (object, decl) in &allocated {
            let mut frame = Frame {
                id: 0,
                bindings: HashMap::default(),
                object_decls: HashMap::default(),
                function_decls: HashMap::default(),
            };
            if let Some(initializer) = decl.init.as_ref() {
                self.validate_initializer_constraints(&decl.ty, initializer, &frame, objects)?;
                self.validate_static_initializer(initializer, &frame, objects)?;
            }
            self.initialize_declared_object(
                *object,
                &decl.ty,
                decl.init.as_ref(),
                &mut frame,
                objects,
                true,
                decl.span,
            )?;
        }
        // These keys belong to the cloned global AST above, which is dropped on return. Do not
        // let its addresses collide with persistent function-body expressions during execution.
        self.variable_cache.clear();
        self.sequence_point();
        Ok(())
    }

    pub(super) fn initialize_host_stdio(
        &mut self,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.initialize_standard_stream_binding("__codex_stdin", CaptureStream::Stdin, objects)?;
        self.initialize_standard_stream_binding("__codex_stdout", CaptureStream::Stdout, objects)?;
        self.initialize_standard_stream_binding("__codex_stderr", CaptureStream::Stderr, objects)?;
        Ok(())
    }

    pub(super) fn initialize_host_math(
        &mut self,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if self
            .lookup_global_binding("__codex_signgam", FileId(0))
            .is_some()
        {
            return Ok(());
        }
        let Some(decl) = self.declared_globals.get("__codex_signgam").cloned() else {
            return Ok(());
        };
        let object = self.allocate_object(
            objects,
            CType::Int,
            StorageDuration::Static,
            decl.span,
            false,
            false,
        )?;
        let zero =
            self.stored_value_from_typed_value(TypedValue::int(0), &CType::Int, decl.span)?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.value = zero;
        }
        self.global_bindings
            .insert("__codex_signgam".to_owned(), object);
        self.signgam_binding = Some(object);
        Ok(())
    }

    pub(super) fn initialize_host_errno(
        &mut self,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if self.errno_binding.is_some() {
            return Ok(());
        }
        let span = Span::new(FileId(0), 0, 0);
        let object = self.allocate_object(
            objects,
            CType::Int,
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        let zero = self.stored_value_from_typed_value(
            TypedValue::int(unsafe { *host_errno_ptr() } as i128),
            &CType::Int,
            span,
        )?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.indeterminate_reason = None;
            state.value = zero;
        }
        self.errno_binding = Some(object);
        Ok(())
    }

    pub(super) fn sync_host_errno_from_host(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(object) = self.errno_binding else {
            return Ok(());
        };
        self.store_value(
            objects,
            object,
            TypedValue::int(unsafe { *host_errno_ptr() } as i128),
            span,
        )
    }

    pub(super) fn sync_host_errno_to_host(
        &mut self,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let Some(object) = self.errno_binding else {
            return Ok(());
        };
        let Some(state) = self.lookup_object(objects, object) else {
            return Ok(());
        };
        let value = match &state.value {
            StoredValue::Scalar(value) => value.to_int().unwrap_or(0),
            _ => 0,
        };
        unsafe {
            *host_errno_ptr() = value as c_int;
        }
        Ok(())
    }

    pub(super) fn sync_host_signgam(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(object) = self.signgam_binding else {
            return Ok(());
        };
        self.store_value(
            objects,
            object,
            TypedValue::int(unsafe { signgam } as i128),
            span,
        )
    }

    fn initialize_standard_stream_binding(
        &mut self,
        name: &str,
        capture: CaptureStream,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if self.host_capture_streams.contains_key(&capture) {
            return Ok(());
        }
        let decl = self.declared_globals.get(name).cloned();
        let span = decl
            .as_ref()
            .map_or_else(|| Span::new(FileId(0), 0, 0), |decl| decl.span);
        let stream_object = self.allocate_host_stream_object(
            objects,
            StorageDuration::Static,
            span,
            StreamBacking::Capture(capture),
            match capture {
                CaptureStream::Stdin => HostStreamMode {
                    kind: HostStreamModeKind::Input,
                    binary: false,
                },
                CaptureStream::Stdout | CaptureStream::Stderr => HostStreamMode {
                    kind: HostStreamModeKind::Output,
                    binary: false,
                },
            },
            false,
        )?;
        self.host_capture_streams.insert(capture, stream_object);

        let Some(decl) = decl else {
            return Ok(());
        };
        let binding_object = self.allocate_object(
            objects,
            decl.ty.clone(),
            StorageDuration::Static,
            decl.span,
            false,
            false,
        )?;
        let pointer = PointerValue::at_object(stream_object);
        let stored = StoredValue::Scalar(TypedValue::pointer(decl.ty.clone(), pointer));
        if let Some(state) = self.lookup_object_mut(objects, binding_object) {
            state.initialized = true;
            state.value = stored;
        }
        self.global_bindings.insert(name.to_owned(), binding_object);
        self.host_stdio_bindings
            .insert(name.to_owned(), binding_object);
        Ok(())
    }

    pub(super) fn allocate_host_stream_object(
        &mut self,
        objects: &mut ObjectFrames,
        storage_duration: StorageDuration,
        span: Span,
        backing: StreamBacking,
        mode: HostStreamMode,
        append: bool,
    ) -> Result<ObjectId, Diagnostic> {
        let (captured, input_closed) = match backing {
            StreamBacking::Capture(CaptureStream::Stdin) => {
                let mut bytes = Vec::new();
                let mut closed = false;
                for ch in self.run_options.stdin.chars() {
                    if matches!(ch, '\u{4}' | '\u{2404}') {
                        closed = true;
                        break;
                    }
                    let mut encoded = [0u8; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut encoded).as_bytes());
                }
                (bytes, closed)
            }
            _ => (Vec::new(), true),
        };
        let object = self.allocate_object(
            objects,
            CType::array_of(CType::UnsignedChar, 1),
            storage_duration,
            span,
            false,
            false,
        )?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.address_taken = true;
            state.value = StoredValue::Array(vec![StoredValue::Scalar(TypedValue::integer(
                CType::UnsignedChar,
                0,
            ))]);
        }
        self.host_streams.insert(
            object,
            HostStream {
                backing,
                open: true,
                captured,
                position: 0,
                append,
                pushback: Vec::new(),
                orientation: 0,
                eof: false,
                error: false,
                mode,
                last_operation: None,
                last_input_hit_eof: false,
                input_closed,
                operation_performed: false,
                buffer: None,
            },
        );
        Ok(object)
    }

    pub(super) fn parse_standard_fopen_mode(&self, bytes: &[u8]) -> Option<ParsedFopenMode> {
        let text = std::str::from_utf8(bytes).ok()?;
        let (kind, binary, opening, exclusive) = match text {
            "r" => (HostStreamModeKind::Input, false, FopenOpening::Read, false),
            "rb" => (HostStreamModeKind::Input, true, FopenOpening::Read, false),
            "w" => (
                HostStreamModeKind::Output,
                false,
                FopenOpening::Write,
                false,
            ),
            "wb" => (HostStreamModeKind::Output, true, FopenOpening::Write, false),
            "wx" => (HostStreamModeKind::Output, false, FopenOpening::Write, true),
            "wbx" | "wxb" => (HostStreamModeKind::Output, true, FopenOpening::Write, true),
            "a" => (
                HostStreamModeKind::Output,
                false,
                FopenOpening::Append,
                false,
            ),
            "ab" => (
                HostStreamModeKind::Output,
                true,
                FopenOpening::Append,
                false,
            ),
            "r+" => (HostStreamModeKind::Update, false, FopenOpening::Read, false),
            "rb+" | "r+b" => (HostStreamModeKind::Update, true, FopenOpening::Read, false),
            "w+" => (
                HostStreamModeKind::Update,
                false,
                FopenOpening::Write,
                false,
            ),
            "wb+" | "w+b" => (HostStreamModeKind::Update, true, FopenOpening::Write, false),
            "w+x" | "wx+" => (HostStreamModeKind::Update, false, FopenOpening::Write, true),
            "w+bx" | "w+xb" | "wb+x" | "wbx+" | "wx+b" | "wxb+" => {
                (HostStreamModeKind::Update, true, FopenOpening::Write, true)
            }
            "a+" => (
                HostStreamModeKind::Update,
                false,
                FopenOpening::Append,
                false,
            ),
            "ab+" | "a+b" => (
                HostStreamModeKind::Update,
                true,
                FopenOpening::Append,
                false,
            ),
            _ => return None,
        };
        Some(ParsedFopenMode {
            kind,
            binary,
            opening,
            exclusive,
        })
    }

    pub(super) fn stream_buffer_use_diag(&self, span: Span) -> Diagnostic {
        Diagnostic::ub(
            "attempt to use the contents of an array supplied to setbuf or setvbuf",
            span,
            Some("7.19.5.5-6"),
        )
    }

    pub(super) fn replace_stream_buffer_config(
        &mut self,
        stream_object: ObjectId,
        new_config: Option<StreamBufferConfig>,
    ) {
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            match (stream.buffer.is_some(), new_config.is_some()) {
                (false, true) => self.configured_stream_buffers += 1,
                (true, false) => self.configured_stream_buffers -= 1,
                _ => {}
            }
            stream.buffer = new_config;
        }
    }

    pub(super) fn stream_buffer_region_overlaps(
        &self,
        object: ObjectId,
        start: usize,
        size: usize,
        objects: &ObjectFrames,
    ) -> bool {
        if size == 0 || self.configured_stream_buffers == 0 {
            return false;
        }
        let Some(end) = start.checked_add(size) else {
            return true;
        };
        self.host_streams.values().any(|stream| {
            let Some(config) = stream.buffer.as_ref() else {
                return false;
            };
            if config.size == 0 {
                return false;
            }
            let Some(pointer) = config.pointer.as_ref() else {
                return false;
            };
            if pointer.object != Some(object) {
                return false;
            }
            let Some(buffer_start) =
                self.pointer_byte_offset(pointer, &CType::UnsignedChar, objects)
            else {
                return false;
            };
            let Some(buffer_end) = buffer_start.checked_add(config.size) else {
                return true;
            };
            start < buffer_end && buffer_start < end
        })
    }

    pub(super) fn stream_buffer_lifetime_has_ended(&self, stream: &HostStream) -> bool {
        stream
            .buffer
            .as_ref()
            .and_then(|config| config.pointer.as_ref())
            .and_then(|pointer| pointer.object)
            .and_then(|object| self.retired_objects.get(&object))
            .is_some_and(|object| !object.alive)
    }

    pub(super) fn note_stream_buffer_lifetime_end(&mut self, retired: &[ObjectId]) {
        if retired.is_empty() || self.pending_stream_buffer_lifetime_ub.is_some() {
            return;
        }
        let supplied_at = self.host_streams.values().find_map(|stream| {
            if !stream.open {
                return None;
            }
            let config = stream.buffer.as_ref()?;
            let object = config.pointer.as_ref()?.object?;
            retired.contains(&object).then_some(config.supplied_at)
        });
        if let Some(span) = supplied_at {
            self.pending_stream_buffer_lifetime_ub = Some(
                Diagnostic::ub(
                    "a supplied buffer's lifetime has ended while its stream is still open",
                    span,
                    Some("7.19.5.5-6"),
                )
                .with_note("the supplied array must remain alive until the stream is closed"),
            );
        }
    }

    pub(super) fn take_pending_stream_buffer_lifetime_ub(&mut self) -> Result<(), Diagnostic> {
        match self.pending_stream_buffer_lifetime_ub.take() {
            Some(diagnostic) => Err(diagnostic),
            None => Ok(()),
        }
    }

    pub(super) fn clear_stream_buffer_config(&mut self, stream_object: ObjectId) {
        self.replace_stream_buffer_config(stream_object, None);
    }

    pub(super) fn mark_stream_input(&mut self, stream_object: ObjectId, hit_eof: bool) {
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.last_operation = Some(StreamLastOperation::Input);
            stream.last_input_hit_eof = hit_eof;
        }
    }

    pub(super) fn mark_stream_output(&mut self, stream_object: ObjectId) {
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.last_operation = Some(StreamLastOperation::Output);
            stream.last_input_hit_eof = false;
        }
    }

    pub(super) fn clear_stream_last_operation(&mut self, stream_object: ObjectId) {
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.last_operation = None;
            stream.last_input_hit_eof = false;
        }
    }

    fn check_stream_update_transition(
        &self,
        stream_object: ObjectId,
        next_operation: StreamLastOperation,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let Some(stream) = self.host_streams.get(&stream_object) else {
            return Err(self.stream_pointer_diag(function_name, span, standard));
        };
        if stream.mode.kind != HostStreamModeKind::Update {
            return Ok(());
        }
        match (stream.last_operation, next_operation) {
            (Some(StreamLastOperation::Output), StreamLastOperation::Input) => Err(Diagnostic::ub(
                "input on an update stream requires an intervening fflush or file-positioning call after output",
                span,
                Some("7.19.5.3"),
            )),
            (Some(StreamLastOperation::Input), StreamLastOperation::Output)
                if !stream.last_input_hit_eof =>
            {
                Err(Diagnostic::ub(
                    "output on an update stream requires an intervening file-positioning call after input that did not encounter end-of-file",
                    span,
                    Some("7.19.5.3"),
                ))
            }
            _ => Ok(()),
        }
    }

    pub(super) fn check_stdio_preconditions(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let (stream_object, next_operation, standard, checked_name) = match function_name {
            "fgetc" | "getc" => (
                self.stream_object_from_value(&evaluated[0], args[0].span(), "fgetc", "7.19.7")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.19.7",
                "fgetc",
            ),
            "fgets" => (
                self.stream_object_from_value(&evaluated[2], args[2].span(), "fgets", "7.19.7.2")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.19.7.2",
                "fgets",
            ),
            "fscanf" => (
                self.stream_object_from_value(&evaluated[0], args[0].span(), "fscanf", "7.19.6.2")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.19.6.2",
                "fscanf",
            ),
            "vfscanf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "vfscanf",
                    "7.19.6.8",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.19.6.8",
                "vfscanf",
            ),
            "fread" => (
                self.stream_object_from_value(&evaluated[3], args[3].span(), "fread", "7.19.8.1")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.19.8.1",
                "fread",
            ),
            "getchar" | "scanf" | "vscanf" | "gets" => (
                self.standard_stream_object(CaptureStream::Stdin, span)?,
                StreamLastOperation::Input,
                "7.19",
                "stdin",
            ),
            "fputc" | "putc" => (
                self.stream_object_from_value(&evaluated[1], args[1].span(), "fputc", "7.19.7")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.19.7",
                "fputc",
            ),
            "fputs" => (
                self.stream_object_from_value(&evaluated[1], args[1].span(), "fputs", "7.19.7.4")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.19.7.4",
                "fputs",
            ),
            "fprintf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "fprintf",
                    "7.19.6.1",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.19.6.1",
                "fprintf",
            ),
            "vfprintf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "vfprintf",
                    "7.19.6.8",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.19.6.8",
                "vfprintf",
            ),
            "fwrite" => (
                self.stream_object_from_value(&evaluated[3], args[3].span(), "fwrite", "7.19.8.2")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.19.8.2",
                "fwrite",
            ),
            "printf" | "vprintf" | "puts" | "putchar" | "perror" => (
                self.standard_stream_object(
                    if function_name == "perror" {
                        CaptureStream::Stderr
                    } else {
                        CaptureStream::Stdout
                    },
                    span,
                )?,
                StreamLastOperation::Output,
                "7.19",
                "stdout",
            ),
            "fgetwc" | "getwc" => (
                self.stream_object_from_value(&evaluated[0], args[0].span(), "fgetwc", "7.24.3")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.24.3",
                "fgetwc",
            ),
            "fgetws" => (
                self.stream_object_from_value(&evaluated[2], args[2].span(), "fgetws", "7.24.3.2")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.24.3.2",
                "fgetws",
            ),
            "fwscanf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "fwscanf",
                    "7.24.2.2",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.24.2.2",
                "fwscanf",
            ),
            "vfwscanf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "vfwscanf",
                    "7.24.2.6",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Input,
                "7.24.2.6",
                "vfwscanf",
            ),
            "getwchar" | "wscanf" | "vwscanf" => (
                self.standard_stream_object(CaptureStream::Stdin, span)?,
                StreamLastOperation::Input,
                "7.24",
                "stdin",
            ),
            "fputwc" | "putwc" => (
                self.stream_object_from_value(&evaluated[1], args[1].span(), "fputwc", "7.24.3")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.24.3",
                "fputwc",
            ),
            "fputws" => (
                self.stream_object_from_value(&evaluated[1], args[1].span(), "fputws", "7.24.3.8")?
                    .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.24.3.8",
                "fputws",
            ),
            "fwprintf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "fwprintf",
                    "7.24.2.1",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.24.2.1",
                "fwprintf",
            ),
            "vfwprintf" => (
                self.stream_object_from_value(
                    &evaluated[0],
                    args[0].span(),
                    "vfwprintf",
                    "7.24.2.5",
                )?
                .expect("stream validated by specific checker"),
                StreamLastOperation::Output,
                "7.24.2.5",
                "vfwprintf",
            ),
            "wprintf" | "vwprintf" | "putwchar" => (
                self.standard_stream_object(CaptureStream::Stdout, span)?,
                StreamLastOperation::Output,
                "7.24",
                "stdout",
            ),
            _ => return Ok(()),
        };
        self.check_stream_update_transition(
            stream_object,
            next_operation,
            span,
            checked_name,
            standard,
        )?;
        self.host_streams
            .get_mut(&stream_object)
            .expect("validated host stream exists")
            .operation_performed = true;
        Ok(())
    }

    pub(super) fn capture_stream_contents(
        &mut self,
        capture: CaptureStream,
    ) -> Result<String, Diagnostic> {
        let Some(object) = self.host_capture_streams.get(&capture).copied() else {
            return Ok(String::new());
        };
        let Some(stream) = self.host_streams.get(&object) else {
            return Ok(String::new());
        };
        Ok(String::from_utf8_lossy(&stream.captured).into_owned())
    }

    pub(super) fn initialize_declared_object(
        &mut self,
        object: ObjectId,
        ty: &CType,
        init: Option<&Initializer>,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        static_zero: bool,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if let Some(initializer) = init {
            self.begin_full_expression();
            let (actual_ty, stored) =
                self.build_initialized_stored_value(ty, initializer, frame, objects)?;
            self.end_full_expression();
            let initialized = self.stored_value_is_determinate(&stored);
            let actual_size = self.type_size_of(&actual_ty).ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "cannot allocate an object of incomplete or oversized type {}",
                        actual_ty
                    ),
                    span,
                )
            })?;
            let storage_duration = self
                .lookup_object(objects, object)
                .expect("declared object exists while it is initialized")
                .storage_duration;
            self.ensure_object_storage_limit(
                objects,
                storage_duration,
                actual_size,
                Some(object),
                span,
            )?;
            let had_incomplete_type = self
                .lookup_object(objects, object)
                .is_some_and(|state| self.type_size_of(&state.ty).is_none());
            if had_incomplete_type {
                self.object_base_addresses.remove(&object);
                self.assign_object_base_address(object, &actual_ty);
            }
            let previous_size = self
                .lookup_object(objects, object)
                .filter(|state| state.alive && state.storage_duration != StorageDuration::Dynamic)
                .map_or(0, |state| state.byte_size);
            {
                let object_state = self.lookup_object_mut(objects, object).unwrap();
                object_state.ty = actual_ty.clone();
                object_state.initialized = initialized;
                object_state.indeterminate_reason = None;
                object_state.byte_size = actual_size;
                object_state.value = stored;
                object_state.modification_count = object_state.modification_count.saturating_add(1);
            }
            if storage_duration != StorageDuration::Dynamic {
                self.replace_live_non_dynamic_bytes(previous_size, actual_size);
            }
            self.object_type_registry.insert(object, actual_ty);
            return Ok(());
        }
        if matches!(ty.unqualified(), CType::Array(_, 0)) {
            return Err(Diagnostic::error(
                "incomplete array type requires an initializer",
                span,
            ));
        }
        if static_zero {
            let stored = if self
                .type_size_of(ty)
                .is_some_and(|size| size >= COMPACT_OBJECT_REPRESENTATION_THRESHOLD)
            {
                StoredValue::ObjectRepresentation(vec![
                    ByteCell::Known(0);
                    self.type_size_of(ty).unwrap()
                ])
            } else {
                self.zero_stored_value(ty)
            };
            let object_state = self.lookup_object_mut(objects, object).unwrap();
            object_state.initialized = true;
            object_state.indeterminate_reason = None;
            object_state.value = stored;
            object_state.modification_count = object_state.modification_count.saturating_add(1);
        } else {
            let stored = self.indeterminate_stored_value(ty);
            let object_state = self.lookup_object_mut(objects, object).unwrap();
            object_state.initialized = false;
            object_state.indeterminate_reason = None;
            object_state.value = stored;
            object_state.modification_count = object_state.modification_count.saturating_add(1);
            object_state.raw_indeterminate_bytes = None;
            object_state.pointer_slots.clear();
        }
        if let Some(object_state) = self.lookup_object(objects, object) {
            self.object_type_registry
                .insert(object, object_state.ty.clone());
        }
        Ok(())
    }

    pub(super) fn validate_static_initializer(
        &self,
        initializer: &Initializer,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match initializer {
            Initializer::Expr(expr) => self.validate_static_initializer_expr(expr, frame, objects),
            Initializer::List { items, .. } => {
                for item in items {
                    self.validate_static_initializer(&item.initializer, frame, objects)?;
                }
                Ok(())
            }
        }
    }

    fn non_constant_initializer_diag(&self, span: Span) -> Diagnostic {
        Diagnostic::error(
            "initializer for static storage duration object is not a compile-time constant",
            span,
        )
    }

    fn validate_static_initializer_expr(
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
            | Expr::OffsetOf { .. } => Ok(()),
            Expr::Variable(name, span) => {
                if self
                    .program
                    .enum_constants
                    .contains_key(&(span.file, name.clone()))
                    || self.lookup_function(name, span.file).is_some()
                    || self
                        .lookup_global_declaration(name, span.file)
                        .is_some_and(|declaration| {
                            matches!(declaration.ty.unqualified(), CType::Array(_, _))
                        })
                {
                    Ok(())
                } else {
                    Err(self.non_constant_initializer_diag(*span))
                }
            }
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::AddressOf => self.validate_static_address_designator(expr, frame, objects),
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::LogicalNot | UnaryOp::BitNot => {
                    self.validate_static_initializer_expr(expr, frame, objects)
                }
                UnaryOp::Dereference | UnaryOp::PreIncrement | UnaryOp::PreDecrement => {
                    Err(self.non_constant_initializer_diag(*span))
                }
            },
            Expr::Call {
                callee, args, span, ..
            } if matches!(callee.as_ref(), Expr::Variable(name, _) if matches!(name.as_str(), "__codex_cmplxf" | "__codex_cmplx" | "__codex_cmplxl"))
                && args.len() == 2 =>
            {
                for argument in args {
                    self.validate_static_initializer_expr(argument, frame, objects)?;
                }
                Ok(())
            }
            Expr::Postfix { span, .. }
            | Expr::Subscript { span, .. }
            | Expr::Assign { span, .. }
            | Expr::CompoundAssign { span, .. }
            | Expr::Call { span, .. }
            | Expr::VaArg { span, .. } => Err(self.non_constant_initializer_diag(*span)),
            Expr::Binary { op, lhs, rhs, span } => {
                if *op == BinaryOp::Comma {
                    return Err(self.non_constant_initializer_diag(*span));
                }
                if matches!(op, BinaryOp::Add | BinaryOp::Sub)
                    && self
                        .validate_static_address_value_expr(expr, frame, objects)
                        .is_ok()
                {
                    return Ok(());
                }
                self.validate_static_initializer_expr(lhs, frame, objects)?;
                if matches!(op, BinaryOp::LogicalAnd | BinaryOp::LogicalOr)
                    && self
                        .eval_typed_integer_constant_expr(lhs, frame, objects)
                        .is_ok_and(|value| {
                            (*op == BinaryOp::LogicalAnd && value.to_int().ok() == Some(0))
                                || (*op == BinaryOp::LogicalOr
                                    && value.to_int().is_ok_and(|value| value != 0))
                        })
                {
                    return Ok(());
                }
                self.validate_static_initializer_expr(rhs, frame, objects)
            }
            Expr::SizeofType {
                vla_bounds, span, ..
            } => {
                for bound in vla_bounds {
                    if let Some(bound) = bound {
                        self.validate_static_initializer_expr(bound, frame, objects)?;
                    } else {
                        return Err(self.non_constant_initializer_diag(*span));
                    }
                }
                Ok(())
            }
            Expr::SizeofExpr { expr, span } => {
                if matches!(
                    self.expr_type(expr, frame, objects)?.unqualified(),
                    CType::Array(..)
                ) && self.expr_refers_to_vla(expr, frame, objects)
                {
                    Err(self.non_constant_initializer_diag(*span))
                } else {
                    Ok(())
                }
            }
            Expr::Cast {
                vla_bounds,
                expr,
                span,
                ..
            } => {
                for bound in vla_bounds {
                    if let Some(bound) = bound {
                        self.validate_static_initializer_expr(bound, frame, objects)?;
                    } else {
                        return Err(self.non_constant_initializer_diag(*span));
                    }
                }
                self.validate_static_initializer_expr(expr, frame, objects)
            }
            Expr::CompoundLiteral {
                vla_bounds,
                initializer,
                ..
            } => {
                for bound in vla_bounds.iter().flatten() {
                    self.validate_static_initializer_expr(bound, frame, objects)?;
                }
                self.validate_static_initializer(initializer, frame, objects)
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => self.validate_static_initializer_expr(
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
            ),
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.validate_static_initializer_expr(condition, frame, objects)?;
                if let Ok(value) = self.eval_typed_integer_constant_expr(condition, frame, objects)
                {
                    if value.to_int()? != 0 {
                        self.validate_static_initializer_expr(then_expr, frame, objects)
                    } else {
                        self.validate_static_initializer_expr(else_expr, frame, objects)
                    }
                } else {
                    self.validate_static_initializer_expr(then_expr, frame, objects)?;
                    self.validate_static_initializer_expr(else_expr, frame, objects)
                }
            }
            Expr::Member { base, .. } => {
                self.validate_static_initializer_expr(base, frame, objects)
            }
        }
    }

    fn validate_static_address_designator(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match expr {
            Expr::Variable(name, span) => {
                if self.lookup_global_binding(name, span.file).is_some()
                    || self.lookup_global_declaration(name, span.file).is_some()
                    || self.lookup_function(name, span.file).is_some()
                {
                    Ok(())
                } else {
                    Err(self.non_constant_initializer_diag(*span))
                }
            }
            Expr::Unary {
                op: UnaryOp::Dereference,
                expr,
                ..
            } => self.validate_static_address_value_expr(expr, frame, objects),
            Expr::Member { base, .. } => {
                self.validate_static_address_designator(base, frame, objects)
            }
            Expr::Subscript { base, index, .. } => {
                self.validate_static_address_value_expr(base, frame, objects)?;
                self.validate_static_initializer_expr(index, frame, objects)
            }
            Expr::Binary {
                op: BinaryOp::Add | BinaryOp::Sub,
                lhs,
                rhs,
                ..
            } => {
                self.validate_static_address_designator(lhs, frame, objects)?;
                self.validate_static_initializer_expr(rhs, frame, objects)
            }
            Expr::Cast { expr, .. } => {
                self.validate_static_address_designator(expr, frame, objects)
            }
            _ => self.validate_static_initializer_expr(expr, frame, objects),
        }
    }

    fn validate_static_address_value_expr(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match expr {
            Expr::Variable(name, span) => {
                if self.lookup_global_binding(name, span.file).is_some()
                    || self.lookup_global_declaration(name, span.file).is_some()
                    || self.lookup_function(name, span.file).is_some()
                {
                    Ok(())
                } else {
                    Err(self.non_constant_initializer_diag(*span))
                }
            }
            Expr::StringLiteral(..) | Expr::WideStringLiteral(..) => Ok(()),
            Expr::Member { base, .. } => {
                self.validate_static_address_designator(base, frame, objects)
            }
            Expr::Unary {
                op: UnaryOp::AddressOf,
                expr,
                ..
            } => self.validate_static_address_designator(expr, frame, objects),
            Expr::Unary {
                op: UnaryOp::Dereference,
                expr,
                ..
            } => self.validate_static_address_value_expr(expr, frame, objects),
            Expr::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
                ..
            } => {
                if self
                    .validate_static_address_value_expr(lhs, frame, objects)
                    .is_ok()
                    && self
                        .validate_static_initializer_expr(rhs, frame, objects)
                        .is_ok()
                {
                    return Ok(());
                }
                if self
                    .validate_static_initializer_expr(lhs, frame, objects)
                    .is_ok()
                    && self
                        .validate_static_address_value_expr(rhs, frame, objects)
                        .is_ok()
                {
                    return Ok(());
                }
                Err(self.non_constant_initializer_diag(expr.span()))
            }
            Expr::Binary {
                op: BinaryOp::Sub,
                lhs,
                rhs,
                ..
            } => {
                self.validate_static_address_value_expr(lhs, frame, objects)?;
                self.validate_static_initializer_expr(rhs, frame, objects)
            }
            Expr::Cast { expr, .. } => {
                self.validate_static_address_value_expr(expr, frame, objects)
            }
            _ => self.validate_static_initializer_expr(expr, frame, objects),
        }
    }

    pub(super) fn build_initialized_stored_value(
        &mut self,
        ty: &CType,
        initializer: &Initializer,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(CType, StoredValue), Diagnostic> {
        let initializer = self.unwrap_braced_string_initializer(ty, initializer);
        if let CType::Array(inner, _) = ty.unqualified()
            && Self::has_incomplete_array_dimension(inner)
        {
            return Err(Diagnostic::error(
                format!("array element has incomplete type {inner}"),
                initializer.span(),
            ));
        }
        if let CType::Array(inner, len) = ty.unqualified() {
            if let Initializer::Expr(Expr::StringLiteral(text, init_span)) = initializer
                && inner.is_character()
            {
                return self.string_literal_array_initializer(inner, *len, text, *init_span);
            }
            if let Initializer::Expr(Expr::WideStringLiteral(text, init_span)) = initializer
                && *inner.unqualified() == self.wchar_type()
            {
                return self.wide_string_literal_array_initializer(*len, text, *init_span);
            }
            if let Initializer::Expr(Expr::Utf16StringLiteral(text, init_span)) = initializer
                && *inner.unqualified() == CType::UnsignedShort
            {
                return self.unicode_string_literal_array_initializer(*len, text, true, *init_span);
            }
            if let Initializer::Expr(Expr::Utf32StringLiteral(text, init_span)) = initializer
                && *inner.unqualified() == CType::UnsignedInt
            {
                return self
                    .unicode_string_literal_array_initializer(*len, text, false, *init_span);
            }
        }

        let actual_ty =
            self.complete_array_initializer_type(ty, initializer, frame, objects, true)?;
        let mut stored = match actual_ty.unqualified() {
            CType::Array(_, 0) => StoredValue::Array(Vec::new()),
            _ => self.zero_stored_value(&actual_ty),
        };
        self.apply_initializer(&mut stored, &actual_ty, initializer, frame, objects, true)?;
        self.normalize_initialized_bit_fields(&mut stored, &actual_ty, initializer.span())?;
        self.refresh_all_union_bytes(&mut stored, &actual_ty, initializer.span())?;
        let final_ty = match (actual_ty.unqualified(), &stored) {
            (CType::Array(inner, 0), StoredValue::Array(values)) => CType::qualified(
                CType::array_of((**inner).clone(), values.len()),
                actual_ty.top_level_qualifiers(),
            ),
            _ => actual_ty,
        };
        if Self::has_incomplete_array_dimension(&final_ty) {
            return Err(Diagnostic::error(
                "incomplete array type requires a non-empty initializer",
                initializer.span(),
            ));
        }
        Ok((final_ty, stored))
    }

    pub(super) fn complete_compound_literal_type(
        &self,
        ty: &CType,
        initializer: &Initializer,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        self.complete_array_initializer_type(ty, initializer, frame, objects, false)
    }

    pub(super) fn complete_array_initializer_type(
        &self,
        ty: &CType,
        initializer: &Initializer,
        frame: &Frame,
        objects: &ObjectFrames,
        defer_outer_list_bound: bool,
    ) -> Result<CType, Diagnostic> {
        let initializer = self.unwrap_braced_string_initializer(ty, initializer);
        let CType::Array(declared_inner, declared_len) = ty.unqualified() else {
            return Ok(ty.clone());
        };

        let source_ty = if let Initializer::Expr(expr) = initializer {
            Some(self.expr_type(expr, frame, objects)?)
        } else {
            None
        };
        let source_array = source_ty
            .as_ref()
            .and_then(|source| match source.unqualified() {
                CType::Array(inner, len) => Some((&**inner, *len)),
                _ => None,
            });
        let len = if *declared_len != 0 {
            *declared_len
        } else if let Some((_, source_len)) = source_array {
            if source_len == 0 {
                return Err(Diagnostic::error(
                    "incomplete array initializer has incomplete type",
                    initializer.span(),
                ));
            }
            source_len
        } else if defer_outer_list_bound && matches!(initializer, Initializer::List { .. }) {
            0
        } else {
            let len = self.incomplete_array_initializer_length(declared_inner, initializer)?;
            if len == 0 {
                return Err(Diagnostic::error(
                    "incomplete array type requires a non-empty initializer",
                    initializer.span(),
                ));
            }
            len
        };

        let completed_inner = if Self::has_incomplete_array_dimension(declared_inner) {
            if let Some((source_inner, _)) = source_array {
                self.complete_array_shape_from_source(
                    declared_inner,
                    source_inner,
                    initializer.span(),
                )?
            } else {
                let element_initializer = self
                    .first_array_element_initializer(initializer)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "cannot infer a nested array bound from this initializer",
                            initializer.span(),
                        )
                    })?;
                self.complete_array_initializer_type(
                    declared_inner,
                    &element_initializer,
                    frame,
                    objects,
                    false,
                )?
            }
        } else {
            (**declared_inner).clone()
        };

        Ok(CType::qualified(
            CType::array_of(completed_inner, len),
            ty.top_level_qualifiers(),
        ))
    }

    fn complete_array_shape_from_source(
        &self,
        declared: &CType,
        source: &CType,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let CType::Array(declared_inner, declared_len) = declared.unqualified() else {
            return Ok(declared.clone());
        };
        let CType::Array(source_inner, source_len) = source.unqualified() else {
            return Err(Diagnostic::error(
                "cannot infer nested array bounds from an initializer of a different shape",
                span,
            ));
        };
        if *source_len == 0 {
            return Err(Diagnostic::error(
                "incomplete array initializer has incomplete type",
                span,
            ));
        }
        let completed_inner = if Self::has_incomplete_array_dimension(declared_inner) {
            self.complete_array_shape_from_source(declared_inner, source_inner, span)?
        } else {
            (**declared_inner).clone()
        };
        Ok(CType::qualified(
            CType::array_of(
                completed_inner,
                if *declared_len == 0 {
                    *source_len
                } else {
                    *declared_len
                },
            ),
            declared.top_level_qualifiers(),
        ))
    }

    pub(super) fn has_incomplete_array_dimension(ty: &CType) -> bool {
        match ty.unqualified() {
            CType::Array(inner, len) => *len == 0 || Self::has_incomplete_array_dimension(inner),
            _ => false,
        }
    }

    fn first_array_element_initializer(&self, initializer: &Initializer) -> Option<Initializer> {
        let Initializer::List { items, span } = initializer else {
            return None;
        };
        let first = items.first()?;
        if first.designators.is_empty() {
            return Some(first.initializer.clone());
        }
        let (Designator::Index(_, _), remaining) = first.designators.split_first()? else {
            return None;
        };
        if remaining.is_empty() {
            return Some(first.initializer.clone());
        }
        Some(Initializer::List {
            items: vec![crate::ast::InitializerItem {
                designators: remaining.to_vec(),
                initializer: first.initializer.clone(),
                span: first.span,
            }],
            span: *span,
        })
    }

    fn incomplete_array_initializer_length(
        &self,
        inner: &CType,
        initializer: &Initializer,
    ) -> Result<usize, Diagnostic> {
        match initializer {
            Initializer::Expr(Expr::StringLiteral(text, _)) if inner.is_character() => {
                Ok(text.narrow_len() + 1)
            }
            Initializer::Expr(Expr::WideStringLiteral(text, _))
                if *inner.unqualified() == self.wchar_type() =>
            {
                Ok(text.utf32_units().len() + 1)
            }
            Initializer::Expr(Expr::Utf16StringLiteral(text, _))
                if *inner.unqualified() == CType::UnsignedShort =>
            {
                Ok(text.utf16_units().len() + 1)
            }
            Initializer::Expr(Expr::Utf32StringLiteral(text, _))
                if *inner.unqualified() == CType::UnsignedInt =>
            {
                Ok(text.utf32_units().len() + 1)
            }
            Initializer::List { items, .. } => {
                let mut next_index = 0usize;
                let mut max_len = 0usize;
                for item in items {
                    let target_index = item
                        .designators
                        .iter()
                        .find_map(|designator| match designator {
                            Designator::Index(index, _) => Some(*index),
                            Designator::Member(_, _) => None,
                        })
                        .unwrap_or(next_index);
                    max_len = max_len.max(target_index.saturating_add(1));
                    next_index = target_index.saturating_add(1);
                }
                Ok(max_len)
            }
            _ => Err(Diagnostic::error(
                "incomplete array type requires a non-empty initializer",
                initializer.span(),
            )),
        }
    }

    pub(super) fn string_literal_array_initializer(
        &self,
        inner: &CType,
        declared_len: usize,
        text: &StringLiteralValue,
        span: Span,
    ) -> Result<(CType, StoredValue), Diagnostic> {
        let mut bytes = text.narrow_bytes();
        bytes.push(0);
        let actual_len = if declared_len == 0 {
            bytes.len()
        } else {
            declared_len
        };
        if bytes.len().saturating_sub(1) > actual_len {
            return Err(Diagnostic::error(
                "string literal is too long for the destination array",
                span,
            ));
        }
        let values = (0..actual_len)
            .map(|idx| {
                let byte = bytes.get(idx).copied().unwrap_or(0);
                StoredValue::Scalar(TypedValue::integer(
                    inner.unqualified().clone(),
                    byte as i128,
                ))
            })
            .collect::<Vec<_>>();
        Ok((
            CType::array_of(inner.unqualified().clone(), actual_len),
            StoredValue::Array(values),
        ))
    }

    pub(super) fn wide_literal_units(
        &self,
        text: &StringLiteralValue,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        // Numeric escapes in an L literal are constrained by the unsigned
        // type corresponding to wchar_t, not by wchar_t itself.  Our wchar_t
        // is a 32-bit int, so values in the upper half of u32 receive the
        // implementation-defined corresponding signed representation.
        Ok(text
            .utf32_units()
            .into_iter()
            .map(|unit| (unit as i32) as libc::wchar_t)
            .collect())
    }

    pub(super) fn wide_string_literal_array_initializer(
        &self,
        declared_len: usize,
        text: &StringLiteralValue,
        span: Span,
    ) -> Result<(CType, StoredValue), Diagnostic> {
        let mut units = self
            .wide_literal_units(text)
            .map_err(|_| Diagnostic::error("invalid wide string literal", span))?;
        units.push(0);
        let actual_len = if declared_len == 0 {
            units.len()
        } else {
            declared_len
        };
        if units.len().saturating_sub(1) > actual_len {
            return Err(Diagnostic::error(
                "wide string literal is too long for the destination array",
                span,
            ));
        }
        let values = (0..actual_len)
            .map(|idx| {
                let unit = units.get(idx).copied().unwrap_or(0);
                StoredValue::Scalar(TypedValue::integer(self.wchar_type(), unit as i128))
            })
            .collect::<Vec<_>>();
        Ok((
            CType::array_of(self.wchar_type(), actual_len),
            StoredValue::Array(values),
        ))
    }

    pub(super) fn unicode_string_literal_array_initializer(
        &self,
        declared_len: usize,
        text: &StringLiteralValue,
        utf16: bool,
        span: Span,
    ) -> Result<(CType, StoredValue), Diagnostic> {
        let element_ty = if utf16 {
            CType::UnsignedShort
        } else {
            CType::UnsignedInt
        };
        let mut units: Vec<u32> = if utf16 {
            text.utf16_units().into_iter().map(u32::from).collect()
        } else {
            text.utf32_units()
        };
        units.push(0);
        let actual_len = if declared_len == 0 {
            units.len()
        } else {
            declared_len
        };
        if units.len().saturating_sub(1) > actual_len {
            return Err(Diagnostic::error(
                "Unicode string literal is too long for the destination array",
                span,
            ));
        }
        let values = (0..actual_len)
            .map(|idx| {
                StoredValue::Scalar(TypedValue::integer(
                    element_ty.clone(),
                    units.get(idx).copied().unwrap_or(0) as i128,
                ))
            })
            .collect();
        Ok((
            CType::array_of(element_ty, actual_len),
            StoredValue::Array(values),
        ))
    }

    fn apply_initializer(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        initializer: &Initializer,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        allow_array_growth: bool,
    ) -> Result<(), Diagnostic> {
        let initializer = self.unwrap_braced_string_initializer(ty, initializer);
        match initializer {
            Initializer::Expr(expr) => {
                if let (CType::Array(inner, len), Expr::StringLiteral(text, init_span)) =
                    (ty.unqualified(), expr)
                    && inner.is_character()
                {
                    let (_, value) =
                        self.string_literal_array_initializer(inner, *len, text, *init_span)?;
                    *stored = value;
                    return Ok(());
                }
                if let (CType::Array(inner, len), Expr::WideStringLiteral(text, init_span)) =
                    (ty.unqualified(), expr)
                    && *inner.unqualified() == self.wchar_type()
                {
                    let (_, value) =
                        self.wide_string_literal_array_initializer(*len, text, *init_span)?;
                    *stored = value;
                    return Ok(());
                }
                if let CType::Array(inner, len) = ty.unqualified() {
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
                        let (_, value) =
                            self.unicode_string_literal_array_initializer(*len, text, utf16, span)?;
                        *stored = value;
                        return Ok(());
                    }
                }
                let sequencing = self
                    .initializer_sequencing
                    .is_some()
                    .then(|| self.sequencing_snapshot());
                let value = self.eval_rvalue(expr, frame, objects)?;
                let value =
                    self.convert_value_in_context(expr, value, ty, expr.span(), frame, objects)?;
                *stored = self.stored_value_from_typed_value(value, ty, expr.span())?;
                if let Some(sequencing) = sequencing {
                    let footprint = self.finish_sequenced_operand(sequencing);
                    self.accumulate_initializer_footprint(footprint);
                }
                Ok(())
            }
            Initializer::List { items, .. } => {
                let owns_sequence = self.initializer_sequencing.is_none();
                if owns_sequence {
                    self.initializer_sequencing = Some(InitializerSequencing::default());
                }
                let result = (|| -> Result<(), Diagnostic> {
                    match ty.unqualified() {
                        CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                            let used = self.consume_initializer_sequence(
                                stored,
                                ty,
                                items,
                                frame,
                                objects,
                                allow_array_growth,
                            )?;
                            if used != items.len() {
                                return Err(Diagnostic::error(
                                    "too many initializer elements",
                                    items[used].span,
                                ));
                            }
                            Ok(())
                        }
                        _ => {
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
                            self.apply_initializer(
                                stored,
                                ty,
                                &items[0].initializer,
                                frame,
                                objects,
                                false,
                            )
                        }
                    }
                })();
                if owns_sequence {
                    let sequence = self
                        .initializer_sequencing
                        .take()
                        .expect("initializer sequencing context exists");
                    if result.is_ok() {
                        self.merge_sequenced_footprint(sequence.footprint);
                    }
                }
                result
            }
        }
    }

    pub(super) fn unwrap_braced_string_initializer<'b>(
        &self,
        ty: &CType,
        initializer: &'b Initializer,
    ) -> &'b Initializer {
        let CType::Array(inner, _) = ty.unqualified() else {
            return initializer;
        };
        let Initializer::List { items, .. } = initializer else {
            return initializer;
        };
        if items.len() != 1 || !items[0].designators.is_empty() {
            return initializer;
        }
        let compatible = match &items[0].initializer {
            Initializer::Expr(Expr::StringLiteral(..)) => inner.is_character(),
            Initializer::Expr(Expr::WideStringLiteral(..)) => {
                *inner.unqualified() == self.wchar_type()
            }
            Initializer::Expr(Expr::Utf16StringLiteral(..)) => {
                *inner.unqualified() == CType::UnsignedShort
            }
            Initializer::Expr(Expr::Utf32StringLiteral(..)) => {
                *inner.unqualified() == CType::UnsignedInt
            }
            _ => false,
        };
        if compatible {
            &items[0].initializer
        } else {
            initializer
        }
    }

    fn consume_initializer_items_into_type(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        items: &[crate::ast::InitializerItem],
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        allow_array_growth: bool,
    ) -> Result<usize, Diagnostic> {
        let Some(first) = items.first() else {
            return Ok(0);
        };
        if first.designators.is_empty() && matches!(first.initializer, Initializer::Expr(_)) {
            let Initializer::Expr(expr) = &first.initializer else {
                unreachable!("checked above");
            };
            let expr_ty = self.expr_type(expr, frame, objects)?;
            let initializes_whole_object = match ty.unqualified() {
                CType::Array(_, _) => matches!(expr_ty.unqualified(), CType::Array(_, _)),
                CType::Struct(_, _) | CType::Union(_, _) => {
                    expr_ty.unqualified() == ty.unqualified()
                }
                _ => false,
            };
            if initializes_whole_object {
                self.apply_initializer(stored, ty, &first.initializer, frame, objects, false)?;
                return Ok(1);
            }
        }
        if let CType::Array(inner, _) = ty.unqualified() {
            if first.designators.is_empty()
                && matches!(
                    first.initializer,
                    Initializer::Expr(Expr::StringLiteral(..))
                )
                && inner.is_character()
            {
                self.apply_initializer(
                    stored,
                    ty,
                    &first.initializer,
                    frame,
                    objects,
                    allow_array_growth,
                )?;
                return Ok(1);
            }
            if first.designators.is_empty()
                && ((matches!(
                    first.initializer,
                    Initializer::Expr(Expr::Utf16StringLiteral(..))
                ) && *inner.unqualified() == CType::UnsignedShort)
                    || (matches!(
                        first.initializer,
                        Initializer::Expr(Expr::Utf32StringLiteral(..))
                    ) && *inner.unqualified() == CType::UnsignedInt))
            {
                self.apply_initializer(
                    stored,
                    ty,
                    &first.initializer,
                    frame,
                    objects,
                    allow_array_growth,
                )?;
                return Ok(1);
            }
            if first.designators.is_empty()
                && matches!(
                    first.initializer,
                    Initializer::Expr(Expr::WideStringLiteral(..))
                )
                && *inner.unqualified() == self.wchar_type()
            {
                self.apply_initializer(
                    stored,
                    ty,
                    &first.initializer,
                    frame,
                    objects,
                    allow_array_growth,
                )?;
                return Ok(1);
            }
        }
        match ty.unqualified() {
            CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                if first.designators.is_empty()
                    && matches!(first.initializer, Initializer::List { .. })
                {
                    self.apply_initializer(stored, ty, &first.initializer, frame, objects, false)?;
                    Ok(1)
                } else {
                    self.consume_initializer_sequence(
                        stored,
                        ty,
                        items,
                        frame,
                        objects,
                        allow_array_growth,
                    )
                }
            }
            _ => {
                if !first.designators.is_empty() {
                    return Err(Diagnostic::error(
                        "designators require an aggregate or union initializer",
                        first.span,
                    ));
                }
                self.apply_initializer(stored, ty, &first.initializer, frame, objects, false)?;
                Ok(1)
            }
        }
    }

    fn consume_initializer_sequence(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        items: &[crate::ast::InitializerItem],
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        allow_array_growth: bool,
    ) -> Result<usize, Diagnostic> {
        match ty.unqualified() {
            CType::Array(inner, _) => {
                let StoredValue::Array(values) = stored else {
                    return Err(Diagnostic::error(
                        "array object does not have array storage",
                        items[0].span,
                    ));
                };
                let mut item_index = 0usize;
                let mut current = 0usize;
                while item_index < items.len() {
                    let item = &items[item_index];
                    if !item.designators.is_empty() {
                        let selectors = self.initializer_selectors(ty, &item.designators)?;
                        let (target_index, rest) = match selectors.split_first() {
                            Some((InitSelector::Index(index), rest)) => (*index, rest),
                            _ => {
                                return Err(Diagnostic::error(
                                    "array initializer designator must begin with [index]",
                                    item.span,
                                ));
                            }
                        };
                        if target_index >= values.len() {
                            if allow_array_growth {
                                while values.len() <= target_index {
                                    values.push(self.zero_stored_value(inner));
                                }
                            } else {
                                return Err(Diagnostic::error(
                                    "array designator is outside the bounds of the array",
                                    item.span,
                                ));
                            }
                        }
                        self.apply_initializer_selectors(
                            &mut values[target_index],
                            inner,
                            rest,
                            &item.initializer,
                            frame,
                            objects,
                            false,
                        )?;
                        item_index += 1;
                        item_index += self.consume_after_designated_subobject(
                            &mut values[target_index],
                            inner,
                            rest,
                            &items[item_index..],
                            frame,
                            objects,
                        )?;
                        current = target_index + 1;
                    } else {
                        if current >= values.len() {
                            if allow_array_growth {
                                values.push(self.zero_stored_value(inner));
                            } else {
                                break;
                            }
                        }
                        let used = self.consume_initializer_items_into_type(
                            &mut values[current],
                            inner,
                            &items[item_index..],
                            frame,
                            objects,
                            false,
                        )?;
                        item_index += used;
                        current += 1;
                    }
                }
                Ok(item_index)
            }
            CType::Struct(_, _) => {
                let positions = self.initializable_record_member_positions(ty);
                let StoredValue::Record(values) = stored else {
                    return Err(Diagnostic::error(
                        "structure object does not have structure storage",
                        items[0].span,
                    ));
                };
                let mut item_index = 0usize;
                let mut current = 0usize;
                while item_index < items.len() {
                    let item = &items[item_index];
                    if !item.designators.is_empty() {
                        let selectors = self.initializer_selectors(ty, &item.designators)?;
                        let (member_name, rest) = match selectors.split_first() {
                            Some((InitSelector::Member(name), rest)) => (name.clone(), rest),
                            _ => {
                                return Err(Diagnostic::error(
                                    "structure initializer designator must begin with .member",
                                    item.span,
                                ));
                            }
                        };
                        let top_pos = positions
                            .iter()
                            .position(|member| member.storage_name == member_name)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "invalid structure initializer designator",
                                    item.span,
                                )
                            })?;
                        let member = positions[top_pos].clone();
                        let slot = self.record_slot_mut(values, &member.storage_name, item.span)?;
                        self.apply_initializer_selectors(
                            slot,
                            &member.ty,
                            rest,
                            &item.initializer,
                            frame,
                            objects,
                            false,
                        )?;
                        item_index += 1;
                        item_index += self.consume_after_designated_subobject(
                            slot,
                            &member.ty,
                            rest,
                            &items[item_index..],
                            frame,
                            objects,
                        )?;
                        current = top_pos + 1;
                    } else {
                        if current >= positions.len() {
                            break;
                        }
                        let member = positions[current].clone();
                        let slot = self.record_slot_mut(values, &member.storage_name, item.span)?;
                        let used = self.consume_initializer_items_into_type(
                            slot,
                            &member.ty,
                            &items[item_index..],
                            frame,
                            objects,
                            false,
                        )?;
                        item_index += used;
                        current += 1;
                    }
                }
                Ok(item_index)
            }
            CType::Union(_, _) => {
                let StoredValue::Union {
                    active_member,
                    members,
                    ..
                } = stored
                else {
                    return Err(Diagnostic::error(
                        "union object does not have union storage",
                        items[0].span,
                    ));
                };
                if items[0].designators.is_empty() {
                    let item = &items[0];
                    let member = self.first_initializable_union_member(ty).ok_or_else(|| {
                        Diagnostic::error("union has no initializable members", item.span)
                    })?;
                    let slot = self.record_slot_mut(members, &member.storage_name, item.span)?;
                    let used = self.consume_initializer_items_into_type(
                        slot, &member.ty, items, frame, objects, false,
                    )?;
                    *active_member = Some(member.storage_name.as_str().into());
                    return Ok(used);
                }

                let mut item_index = 0;
                while item_index < items.len() && !items[item_index].designators.is_empty() {
                    let item = &items[item_index];
                    let selectors = self.initializer_selectors(ty, &item.designators)?;
                    let (member_name, rest) = match selectors.split_first() {
                        Some((InitSelector::Member(name), rest)) => (name.clone(), rest),
                        _ => {
                            return Err(Diagnostic::error(
                                "union initializer designator must begin with .member",
                                item.span,
                            ));
                        }
                    };
                    let member = self
                        .direct_member_by_storage_name(ty, &member_name)
                        .cloned()
                        .ok_or_else(|| {
                            Diagnostic::error("invalid union initializer designator", item.span)
                        })?;
                    let slot = self.record_slot_mut(members, &member.storage_name, item.span)?;
                    self.apply_initializer_selectors(
                        slot,
                        &member.ty,
                        rest,
                        &item.initializer,
                        frame,
                        objects,
                        false,
                    )?;
                    *active_member = Some(member.storage_name.as_str().into());
                    item_index += 1;
                    let slot = self.record_slot_mut(members, &member.storage_name, item.span)?;
                    item_index += self.consume_after_designated_subobject(
                        slot,
                        &member.ty,
                        rest,
                        &items[item_index..],
                        frame,
                        objects,
                    )?;
                }
                Ok(item_index)
            }
            _ => Err(Diagnostic::error(
                "initializer sequence requires an aggregate or union type",
                items[0].span,
            )),
        }
    }

    fn consume_after_designated_subobject(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        selectors: &[InitSelector],
        items: &[crate::ast::InitializerItem],
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        let Some((first, rest)) = selectors.split_first() else {
            return Ok(0);
        };
        // A later designation is relative to the enclosing brace pair. Never
        // consume it while finishing a nested subaggregate through brace elision.
        let count = items
            .iter()
            .position(|item| !item.designators.is_empty())
            .unwrap_or(items.len());
        let items = &items[..count];
        if items.is_empty() {
            return Ok(0);
        }
        match (ty.unqualified(), stored, first) {
            (CType::Array(inner, _), StoredValue::Array(values), InitSelector::Index(index)) => {
                let mut used = self.consume_after_designated_subobject(
                    &mut values[*index],
                    inner,
                    rest,
                    items,
                    frame,
                    objects,
                )?;
                for slot in values.iter_mut().skip(index + 1) {
                    if used == items.len() {
                        break;
                    }
                    used += self.consume_initializer_items_into_type(
                        slot,
                        inner,
                        &items[used..],
                        frame,
                        objects,
                        false,
                    )?;
                }
                Ok(used)
            }
            (CType::Struct(..), StoredValue::Record(values), InitSelector::Member(name)) => {
                let members = self.initializable_record_member_positions(ty);
                let index = members
                    .iter()
                    .position(|member| member.storage_name == *name)
                    .expect("validated initializer member");
                let member = &members[index];
                let slot = self.record_slot_mut(values, name, items[0].span)?;
                let mut used = self.consume_after_designated_subobject(
                    slot, &member.ty, rest, items, frame, objects,
                )?;
                for member in members.iter().skip(index + 1) {
                    if used == items.len() {
                        break;
                    }
                    let slot =
                        self.record_slot_mut(values, &member.storage_name, items[used].span)?;
                    used += self.consume_initializer_items_into_type(
                        slot,
                        &member.ty,
                        &items[used..],
                        frame,
                        objects,
                        false,
                    )?;
                }
                Ok(used)
            }
            (CType::Union(..), StoredValue::Union { members, .. }, InitSelector::Member(name)) => {
                let member = self
                    .direct_member_by_storage_name(ty, name)
                    .cloned()
                    .expect("validated initializer member");
                let slot = self.record_slot_mut(members, name, items[0].span)?;
                self.consume_after_designated_subobject(
                    slot, &member.ty, rest, items, frame, objects,
                )
            }
            _ => unreachable!("validated initializer selector"),
        }
    }

    fn apply_initializer_selectors(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        selectors: &[InitSelector],
        initializer: &Initializer,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        allow_array_growth: bool,
    ) -> Result<(), Diagnostic> {
        let Some((first, rest)) = selectors.split_first() else {
            return self.apply_initializer(
                stored,
                ty,
                initializer,
                frame,
                objects,
                allow_array_growth,
            );
        };
        match first {
            InitSelector::Index(index) => match (stored, ty.unqualified()) {
                (StoredValue::Array(values), CType::Array(inner, _)) => {
                    if *index >= values.len() {
                        if allow_array_growth {
                            while values.len() <= *index {
                                values.push(self.zero_stored_value(inner));
                            }
                        } else {
                            return Err(Diagnostic::error(
                                "array designator is outside the bounds of the array",
                                initializer.span(),
                            ));
                        }
                    }
                    self.apply_initializer_selectors(
                        &mut values[*index],
                        inner,
                        rest,
                        initializer,
                        frame,
                        objects,
                        false,
                    )
                }
                _ => Err(Diagnostic::error(
                    "array designator requires an array type",
                    initializer.span(),
                )),
            },
            InitSelector::Member(name) => match (stored, ty.unqualified()) {
                (StoredValue::Record(values), CType::Struct(_, _)) => {
                    let member = self
                        .direct_member_by_storage_name(ty, name)
                        .cloned()
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "invalid structure initializer designator",
                                initializer.span(),
                            )
                        })?;
                    let slot =
                        self.record_slot_mut(values, &member.storage_name, initializer.span())?;
                    self.apply_initializer_selectors(
                        slot,
                        &member.ty,
                        rest,
                        initializer,
                        frame,
                        objects,
                        false,
                    )
                }
                (
                    StoredValue::Union {
                        active_member,
                        members,
                        ..
                    },
                    CType::Union(_, _),
                ) => {
                    let member = self
                        .direct_member_by_storage_name(ty, name)
                        .cloned()
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "invalid union initializer designator",
                                initializer.span(),
                            )
                        })?;
                    let slot =
                        self.record_slot_mut(members, &member.storage_name, initializer.span())?;
                    self.apply_initializer_selectors(
                        slot,
                        &member.ty,
                        rest,
                        initializer,
                        frame,
                        objects,
                        false,
                    )?;
                    *active_member = Some(member.storage_name.as_str().into());
                    Ok(())
                }
                _ => Err(Diagnostic::error(
                    "member designator requires a structure or union type",
                    initializer.span(),
                )),
            },
        }
    }

    pub(super) fn initializer_selectors(
        &self,
        ty: &CType,
        designators: &[Designator],
    ) -> Result<Vec<InitSelector>, Diagnostic> {
        if designators.is_empty() {
            return Ok(Vec::new());
        }
        match &designators[0] {
            Designator::Index(index, designator_span) => match ty.unqualified() {
                CType::Array(inner, _) => {
                    let mut result = vec![InitSelector::Index(*index)];
                    result.extend(self.initializer_selectors(inner, &designators[1..])?);
                    Ok(result)
                }
                _ => Err(Diagnostic::error(
                    "array designator requires an array type",
                    *designator_span,
                )),
            },
            Designator::Member(name, designator_span) => {
                let ResolvedMemberAccess {
                    path,
                    ty: member_ty,
                    ..
                } = self.resolve_member_access(ty, name, *designator_span)?;
                if matches!(member_ty.unqualified(), CType::Array(_, 0)) {
                    return Err(Diagnostic::error(
                        "flexible array members cannot be initialized",
                        *designator_span,
                    ));
                }
                let mut result = path
                    .iter()
                    .cloned()
                    .map(InitSelector::Member)
                    .collect::<Vec<_>>();
                result.extend(self.initializer_selectors(&member_ty, &designators[1..])?);
                Ok(result)
            }
        }
    }

    pub(super) fn initializable_record_member_positions(&self, ty: &CType) -> Vec<RecordMember> {
        self.record_type(ty)
            .map(|record| {
                record
                    .members
                    .iter()
                    .filter(|member| {
                        !matches!(member.ty.unqualified(), CType::Array(_, 0))
                            && (member.name.is_some()
                                || (member.bit_width.is_none()
                                    && matches!(
                                        member.ty.unqualified(),
                                        CType::Struct(_, _) | CType::Union(_, _)
                                    )))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn initializer_selected_record_member(
        &self,
        target: &CType,
        selectors: &[InitSelector],
    ) -> Option<RecordMember> {
        let mut current = target.clone();
        let mut selected = None;
        for selector in selectors {
            match selector {
                InitSelector::Index(_) => {
                    let CType::Array(inner, _) = current.unqualified() else {
                        return selected;
                    };
                    current = (**inner).clone();
                }
                InitSelector::Member(name) => {
                    let member = self
                        .record_type(&current)?
                        .members
                        .iter()
                        .find(|member| member.storage_name == *name)?
                        .clone();
                    current = member.ty.clone();
                    selected = Some(member);
                }
            }
        }
        selected
    }

    pub(super) fn annotate_member_initializer_error(
        diagnostic: Diagnostic,
        member: &RecordMember,
    ) -> Diagnostic {
        let Some(name) = member.name.as_deref() else {
            return diagnostic;
        };
        let diagnostic =
            diagnostic.with_message_prefix(format!("while initializing member {name}"));
        if let Some(span) = member.declaration_span {
            diagnostic.with_related_span(
                "destination",
                format!("member {name} is declared here"),
                span,
            )
        } else {
            diagnostic
        }
    }

    pub(super) fn first_initializable_union_member(&self, ty: &CType) -> Option<RecordMember> {
        self.initializable_record_member_positions(ty)
            .into_iter()
            .next()
    }

    fn record_slot_mut<'b>(
        &self,
        values: &'b mut [(StoredMemberName, StoredValue)],
        name: &str,
        span: Span,
    ) -> Result<&'b mut StoredValue, Diagnostic> {
        values
            .iter_mut()
            .find(|(candidate, _)| candidate.as_ref() == name)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                Diagnostic::error("initializer target member not found in storage", span)
            })
    }

    pub(super) fn record_slot<'b>(
        &self,
        values: &'b [(StoredMemberName, StoredValue)],
        name: &str,
        span: Span,
    ) -> Result<&'b StoredValue, Diagnostic> {
        values
            .iter()
            .find(|(candidate, _)| candidate.as_ref() == name)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                Diagnostic::error("initializer target member not found in storage", span)
            })
    }

    pub(super) fn overlay_bytes(&self, dst: &mut [ByteCell], offset: usize, src: &[ByteCell]) {
        for (index, byte) in src.iter().enumerate() {
            if let Some(slot) = dst.get_mut(offset + index) {
                *slot = *byte;
            }
        }
    }

    fn normalize_initialized_bit_fields(
        &self,
        stored: &mut StoredValue,
        ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (stored, ty.unqualified()) {
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                for value in values {
                    self.normalize_initialized_bit_fields(value, inner, span)?;
                }
            }
            (StoredValue::Record(values), CType::Struct(_, _))
            | (
                StoredValue::Union {
                    members: values, ..
                },
                CType::Union(_, _),
            ) => {
                let Some(record) = self.record_type(ty) else {
                    return Ok(());
                };
                for member in &record.members {
                    let slot = self.record_slot_mut(values, &member.storage_name, span)?;
                    if let Some(width) = member.bit_width {
                        if width != 0
                            && let StoredValue::Scalar(value) = slot
                            && !value.indeterminate
                        {
                            *slot = StoredValue::Scalar(TypedValue::integer(
                                member.ty.clone(),
                                self.normalize_bit_field_value(value.to_int()?, &member.ty, width),
                            ));
                        }
                    } else {
                        self.normalize_initialized_bit_fields(slot, &member.ty, span)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn refresh_all_union_bytes(
        &mut self,
        stored: &mut StoredValue,
        ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (stored, ty.unqualified()) {
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                for value in values {
                    self.refresh_all_union_bytes(value, inner, span)?;
                }
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let record = self
                    .record_type(ty)
                    .cloned()
                    .ok_or_else(|| Diagnostic::error("incomplete structure type", span))?;
                for member in &record.members {
                    let slot = self.record_slot_mut(values, &member.storage_name, span)?;
                    self.refresh_all_union_bytes(slot, &member.ty, span)?;
                }
            }
            (
                StoredValue::Union {
                    active_member,
                    members,
                    bytes,
                },
                CType::Union(_, _),
            ) => {
                let record = self
                    .record_type(ty)
                    .cloned()
                    .ok_or_else(|| Diagnostic::error("incomplete union type", span))?;
                for member in &record.members {
                    let slot = self.record_slot_mut(members, &member.storage_name, span)?;
                    self.refresh_all_union_bytes(slot, &member.ty, span)?;
                }
                if let Some(active_name) = active_member.clone() {
                    let member = record
                        .members
                        .iter()
                        .find(|member| member.storage_name.as_str() == active_name.as_ref())
                        .ok_or_else(|| {
                            Diagnostic::error("active union member was not found in storage", span)
                        })?
                        .clone();
                    let slot = self.record_slot(members, &active_name, span)?.clone();
                    let member_bytes = self.serialize_member_bytes(&member, &slot, span)?;
                    self.overlay_bytes(bytes, member.offset, &member_bytes);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn serialize_member_bytes(
        &mut self,
        member: &RecordMember,
        stored: &StoredValue,
        span: Span,
    ) -> Result<Vec<ByteCell>, Diagnostic> {
        let size = if member.bit_width.is_some() {
            member.bit_storage_size
        } else if matches!(member.ty.unqualified(), CType::Array(_, 0)) {
            0
        } else {
            self.type_size_of(&member.ty).ok_or_else(|| {
                Diagnostic::error(
                    format!("{} does not have an object representation", member.ty),
                    span,
                )
            })?
        };
        let mut bytes = vec![ByteCell::Known(0); size];
        self.serialize_member_bytes_into(member, stored, span, &mut bytes)?;
        if member.bit_width == Some(0) {
            bytes.clear();
        }
        Ok(bytes)
    }

    pub(super) fn serialize_stored_value(
        &mut self,
        ty: &CType,
        stored: &StoredValue,
        span: Span,
    ) -> Result<Vec<ByteCell>, Diagnostic> {
        let size = self.type_size_of(ty).ok_or_else(|| {
            Diagnostic::error(
                format!("{} does not have an object representation", ty),
                span,
            )
        })?;
        let mut bytes = vec![ByteCell::Known(0); size];
        self.serialize_stored_value_into(ty, stored, span, &mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn serialize_scalar_bytes(
        &mut self,
        ty: &CType,
        value: &TypedValue,
        span: Span,
    ) -> Result<Vec<ByteCell>, Diagnostic> {
        let size = self.type_size_of(ty).ok_or_else(|| {
            Diagnostic::error(
                format!("{} does not have an object representation", ty),
                span,
            )
        })?;
        let mut bytes = vec![ByteCell::Known(0); size];
        self.serialize_scalar_bytes_into(ty, value, span, &mut bytes)?;
        Ok(bytes)
    }

    fn serialize_member_bytes_into(
        &mut self,
        member: &RecordMember,
        stored: &StoredValue,
        span: Span,
        dst: &mut [ByteCell],
    ) -> Result<(), Diagnostic> {
        if let Some(width) = member.bit_width {
            if width == 0 || dst.is_empty() {
                return Ok(());
            }
            match stored {
                StoredValue::Scalar(value) if !value.indeterminate => {
                    let normalized =
                        self.normalize_bit_field_value(value.to_int()?, &member.ty, width) as u128;
                    let encoded = normalized << member.bit_offset;
                    let field_mask = self.integer_mask(width as u32) << member.bit_offset;
                    for (index, slot) in dst.iter_mut().enumerate() {
                        let mask = ((field_mask >> (index * 8)) & 0xff) as u8;
                        let bits = ((encoded >> (index * 8)) & 0xff) as u8;
                        *slot = match *slot {
                            ByteCell::Known(existing) => {
                                ByteCell::Known((existing & !mask) | (bits & mask))
                            }
                            ByteCell::Indeterminate if mask == 0xff => ByteCell::Known(bits),
                            ByteCell::Indeterminate => ByteCell::Indeterminate,
                        };
                    }
                }
                _ => {
                    let field_mask = self.integer_mask(width as u32) << member.bit_offset;
                    for (index, slot) in dst.iter_mut().enumerate() {
                        if ((field_mask >> (index * 8)) & 0xff) != 0 {
                            *slot = ByteCell::Indeterminate;
                        }
                    }
                }
            }
            return Ok(());
        }
        self.serialize_stored_value_into(&member.ty, stored, span, dst)
    }

    fn serialize_stored_value_into(
        &mut self,
        ty: &CType,
        stored: &StoredValue,
        span: Span,
        dst: &mut [ByteCell],
    ) -> Result<(), Diagnostic> {
        match (stored, ty.unqualified()) {
            (StoredValue::Indeterminate, _) => {
                dst.fill(ByteCell::Indeterminate);
                Ok(())
            }
            (StoredValue::Scalar(value), _) => {
                self.serialize_scalar_bytes_into(ty, value, span, dst)
            }
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                let stride = self.type_size_of(inner).ok_or_else(|| {
                    Diagnostic::error("array element type does not have size", span)
                })?;
                for (index, value) in values.iter().enumerate() {
                    let start = index * stride;
                    self.serialize_stored_value_into(
                        inner,
                        value,
                        span,
                        &mut dst[start..start + stride],
                    )?;
                }
                Ok(())
            }
            (StoredValue::ObjectRepresentation(bytes), _) => {
                if bytes.len() != dst.len() {
                    return Err(Diagnostic::error(
                        "stored object representation has the wrong size for its type",
                        span,
                    ));
                }
                dst.copy_from_slice(bytes);
                Ok(())
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let record = self
                    .record_type(ty)
                    .cloned()
                    .ok_or_else(|| Diagnostic::error("incomplete structure type", span))?;
                for member in &record.members {
                    let slot = self.record_slot(values, &member.storage_name, span)?;
                    let member_size = if member.bit_width.is_some() {
                        member.bit_storage_size
                    } else if matches!(member.ty.unqualified(), CType::Array(_, 0)) {
                        0
                    } else {
                        self.type_size_of(&member.ty).ok_or_else(|| {
                            Diagnostic::error(
                                format!("{} does not have an object representation", member.ty),
                                span,
                            )
                        })?
                    };
                    self.serialize_member_bytes_into(
                        member,
                        slot,
                        span,
                        &mut dst[member.offset..member.offset + member_size],
                    )?;
                }
                Ok(())
            }
            (StoredValue::Union { bytes, .. }, CType::Union(_, _)) => {
                dst.copy_from_slice(bytes);
                Ok(())
            }
            _ => Err(Diagnostic::error(
                format!("{} does not match its stored representation", ty),
                span,
            )),
        }
    }

    fn serialize_scalar_bytes_into(
        &mut self,
        ty: &CType,
        value: &TypedValue,
        span: Span,
        dst: &mut [ByteCell],
    ) -> Result<(), Diagnostic> {
        if let ValueData::ObjectRepresentation(bytes) = &value.data {
            if bytes.len() != dst.len() {
                return Err(Diagnostic::error(
                    "stored object representation has the wrong size for its type",
                    span,
                ));
            }
            dst.copy_from_slice(bytes);
            return Ok(());
        }
        if value.indeterminate {
            dst.fill(ByteCell::Indeterminate);
            return Ok(());
        }
        match ty.unqualified() {
            CType::Bool => {
                let int = value.to_int()?;
                if !(0..=1).contains(&int) {
                    return Err(Diagnostic::ub(
                        "attempt to store an invalid _Bool representation",
                        span,
                        Some("6.2.6.1p5-6"),
                    ));
                }
                dst[0] = ByteCell::Known(int as u8);
            }
            _ if ty.is_integer() => {
                let bits = (dst.len() * 8) as u32;
                let mask = self.integer_mask(bits);
                let normalized =
                    ty.normalize_integer_value(value.to_int()?).unwrap() as u128 & mask;
                for (index, slot) in dst.iter_mut().enumerate() {
                    *slot = ByteCell::Known(((normalized >> (index * 8)) & 0xff) as u8);
                }
            }
            CType::Float => {
                for (slot, byte) in dst
                    .iter_mut()
                    .zip((value.to_float()? as f32).to_bits().to_le_bytes())
                {
                    *slot = ByteCell::Known(byte);
                }
            }
            CType::Double | CType::LongDouble => {
                for (slot, byte) in dst
                    .iter_mut()
                    .zip(value.to_float()?.to_bits().to_le_bytes())
                {
                    *slot = ByteCell::Known(byte);
                }
            }
            CType::Complex(component_ty) => {
                let complex = value.to_complex()?;
                let component_size = self.type_size_of(component_ty).ok_or_else(|| {
                    Diagnostic::error(
                        "complex object representation requires a real floating component type",
                        span,
                    )
                })?;
                let (real_bytes, imag_bytes) = dst.split_at_mut(component_size);
                self.serialize_complex_component_bytes_into(
                    component_ty,
                    complex.real,
                    span,
                    real_bytes,
                )?;
                self.serialize_complex_component_bytes_into(
                    component_ty,
                    complex.imag,
                    span,
                    imag_bytes,
                )?;
            }
            CType::VaList => {
                for (slot, byte) in dst.iter_mut().zip(
                    u64::try_from(value.to_int()?)
                        .map_err(|_| {
                            Diagnostic::ub(
                                "va_list contains an invalid object representation",
                                span,
                                Some("7.15.1.1"),
                            )
                        })?
                        .to_le_bytes(),
                ) {
                    *slot = ByteCell::Known(byte);
                }
            }
            CType::Pointer(_) => {
                if matches!(
                    &value.data,
                    ValueData::Pointer(pointer)
                        if pointer.object.is_some_and(|object| {
                            self.retired_objects
                                .get(&object)
                                .is_some_and(|state| !state.alive)
                        })
                ) {
                    dst.fill(ByteCell::Indeterminate);
                    return Ok(());
                }
                let address = self.encode_pointer_address(value, ty, span)?;
                for (slot, byte) in dst.iter_mut().zip(address.to_le_bytes()) {
                    *slot = ByteCell::Known(byte);
                }
            }
            _ => {
                return Err(Diagnostic::error(
                    format!("{} does not have a supported scalar representation", ty),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn serialize_complex_component_bytes_into(
        &self,
        component_ty: &CType,
        component: f64,
        span: Span,
        dst: &mut [ByteCell],
    ) -> Result<(), Diagnostic> {
        match component_ty.unqualified() {
            CType::Float => {
                for (slot, byte) in dst
                    .iter_mut()
                    .zip((component as f32).to_bits().to_le_bytes())
                {
                    *slot = ByteCell::Known(byte);
                }
                Ok(())
            }
            CType::Double | CType::LongDouble => {
                for (slot, byte) in dst.iter_mut().zip(component.to_bits().to_le_bytes()) {
                    *slot = ByteCell::Known(byte);
                }
                Ok(())
            }
            _ => Err(Diagnostic::error(
                "complex object representation requires a real floating component type",
                span,
            )),
        }
    }

    fn encode_pointer_address(
        &mut self,
        value: &TypedValue,
        ty: &CType,
        span: Span,
    ) -> Result<u64, Diagnostic> {
        let CType::Pointer(inner) = ty.unqualified() else {
            return Err(Diagnostic::error(
                "pointer encoding requires a pointer type",
                span,
            ));
        };
        match &value.data {
            ValueData::Pointer(pointer) => {
                if pointer.is_null() {
                    return Ok(0);
                }
                if let Some(address) = Self::opaque_integer_pointer_address(pointer) {
                    return Ok(address);
                }
                if matches!(inner.unqualified(), CType::Function(..)) {
                    return Err(Diagnostic::ub(
                        "read of a function pointer with an invalid object representation",
                        span,
                        Some("6.2.6.1p5-6"),
                    ));
                }
                if let Some(address) = self.encoded_object_pointers.get(pointer).copied() {
                    let candidate = EncodedPointer::Object {
                        pointer: pointer.as_ref().clone(),
                        pointee_ty: Some((**inner).clone()),
                    };
                    let candidates = self.decoded_pointers.entry(address).or_default();
                    if !candidates.contains(&candidate) {
                        candidates.push(candidate);
                    }
                    return Ok(address);
                }
                let Some(object_id) = pointer.object else {
                    return Ok(0);
                };
                let base = self
                    .object_base_addresses
                    .get(&object_id)
                    .copied()
                    .ok_or_else(|| {
                        Diagnostic::error("object does not have an assigned address", span)
                    })?;
                let address = if let Some(inner_offset) =
                    self.pointer_byte_offset_from_root_type(pointer, inner)
                {
                    base.checked_add(inner_offset as u64).ok_or_else(|| {
                        Diagnostic::ub("pointer address overflow", span, Some("6.2.6.1p5-6"))
                    })?
                } else {
                    let fallback = self.next_encoded_pointer;
                    self.next_encoded_pointer += 0x1000;
                    fallback
                };
                self.encoded_object_pointers
                    .insert(pointer.as_ref().clone(), address);
                self.decoded_pointers
                    .entry(address)
                    .or_default()
                    .push(EncodedPointer::Object {
                        pointer: pointer.as_ref().clone(),
                        pointee_ty: Some((**inner).clone()),
                    });
                Ok(address)
            }
            ValueData::Function(name) => {
                if !matches!(inner.unqualified(), CType::Function(..)) {
                    return Err(Diagnostic::ub(
                        "read of an object pointer with an invalid object representation",
                        span,
                        Some("6.2.6.1p5-6"),
                    ));
                }
                if let Some(address) = self.encoded_function_pointers.get(name.as_ref()) {
                    return Ok(*address);
                }
                let address = self.next_encoded_pointer;
                self.next_encoded_pointer += 0x1000;
                self.encoded_function_pointers
                    .insert(name.to_string(), address);
                self.decoded_pointers
                    .entry(address)
                    .or_default()
                    .push(EncodedPointer::Function(name.to_string()));
                Ok(address)
            }
            _ => Err(Diagnostic::error(
                format!("expected pointer value, got {}", value.ty),
                span,
            )),
        }
    }

    pub(super) fn pointer_numeric_address(
        &self,
        value: &TypedValue,
        ty: &CType,
        span: Span,
    ) -> Result<u64, Diagnostic> {
        let CType::Pointer(inner) = ty.unqualified() else {
            return Err(Diagnostic::error(
                "pointer encoding requires a pointer type",
                span,
            ));
        };
        match &value.data {
            ValueData::Pointer(pointer) => {
                if pointer.is_null() {
                    return Ok(0);
                }
                if let Some(address) = Self::opaque_integer_pointer_address(pointer) {
                    return Ok(address);
                }
                if matches!(inner.unqualified(), CType::Function(..)) {
                    return Err(Diagnostic::ub(
                        "read of a function pointer with an invalid object representation",
                        span,
                        Some("6.2.6.1p5-6"),
                    ));
                }
                if let Some(address) = self.encoded_object_pointers.get(pointer) {
                    return Ok(*address);
                }
                let Some(object_id) = pointer.object else {
                    return Ok(0);
                };
                let base = self
                    .object_base_addresses
                    .get(&object_id)
                    .copied()
                    .ok_or_else(|| {
                        Diagnostic::error("object does not have an assigned address", span)
                    })?;
                if let Some(inner_offset) = self.pointer_byte_offset_from_root_type(pointer, inner)
                {
                    base.checked_add(inner_offset as u64).ok_or_else(|| {
                        Diagnostic::ub("pointer address overflow", span, Some("6.2.6.1p5-6"))
                    })
                } else {
                    Err(Diagnostic::error(
                        "pointer address is not materializable for integer conversion",
                        span,
                    ))
                }
            }
            ValueData::Function(name) => self
                .encoded_function_pointers
                .get(name.as_ref())
                .copied()
                .ok_or_else(|| {
                    Diagnostic::error("function pointer does not have an assigned address", span)
                }),
            _ => Err(Diagnostic::error(
                format!("expected pointer value, got {}", value.ty),
                span,
            )),
        }
    }

    fn decode_pointer_bytes(
        &self,
        ty: &CType,
        bytes: &[ByteCell],
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        let raw = self.known_bytes(bytes, span)?.to_vec();
        let address = u64::from_le_bytes(raw.try_into().unwrap());
        if address == 0 {
            return Ok(StoredValue::Scalar(TypedValue::pointer(
                ty.clone(),
                Self::null_pointer(),
            )));
        }
        let Some(encoded) = self.decoded_pointers.get(&address) else {
            if self
                .opaque_integer_pointer_addresses
                .borrow()
                .contains(&address)
                && let Ok(byte_offset) = usize::try_from(address)
            {
                return Ok(StoredValue::Scalar(TypedValue::pointer(
                    ty.clone(),
                    PointerValue {
                        object: None,
                        base_offset: 1,
                        offset: 0,
                        member_path: Rc::new(Vec::new()),
                        designated_root_ty: None,
                        byte_offset_override: Some(byte_offset),
                        arithmetic_domain_start: None,
                        object_representation_domain: None,
                    },
                )));
            }
            return Ok(StoredValue::Scalar(TypedValue::object_representation(
                ty.clone(),
                bytes.to_vec(),
            )));
        };
        let CType::Pointer(inner) = ty.unqualified() else {
            unreachable!("pointer bytes require a pointer type");
        };
        let data = if matches!(inner.unqualified(), CType::Function(..)) {
            encoded.iter().find_map(|candidate| match candidate {
                EncodedPointer::Function(name) => Some(ValueData::Function(name.clone().into())),
                EncodedPointer::Object { .. } => None,
            })
        } else {
            self.best_decoded_object_pointer(encoded, inner)
                .map(|pointer| ValueData::Pointer(Rc::new(pointer)))
        };
        let Some(data) = data else {
            return Ok(StoredValue::Scalar(TypedValue::object_representation(
                ty.clone(),
                bytes.to_vec(),
            )));
        };
        Ok(StoredValue::Scalar(TypedValue::from_data(ty.clone(), data)))
    }

    pub(super) fn best_decoded_object_pointer(
        &self,
        candidates: &[EncodedPointer],
        target_inner: &CType,
    ) -> Option<PointerValue> {
        candidates
            .iter()
            .filter_map(|candidate| {
                let EncodedPointer::Object {
                    pointer,
                    pointee_ty,
                } = candidate
                else {
                    return None;
                };
                let type_score = match pointee_ty {
                    Some(source_inner)
                        if self.cross_unit_tagged_type_compatible(
                            source_inner.unqualified(),
                            target_inner.unqualified(),
                        ) =>
                    {
                        2
                    }
                    Some(source_inner)
                        if matches!(source_inner.unqualified(), CType::Void)
                            && !matches!(target_inner.unqualified(), CType::Function(..)) =>
                    {
                        1
                    }
                    Some(_) => return None,
                    None => 0,
                };
                let union_round_trip = pointer
                    .designated_root_ty
                    .as_ref()
                    .filter(|root| matches!(root.unqualified(), CType::Union(_, _)))
                    .and_then(|root| self.storage_path_type(root, &pointer.member_path))
                    .is_some_and(|member_ty| {
                        self.cross_unit_tagged_type_compatible(
                            member_ty.unqualified(),
                            target_inner.unqualified(),
                        )
                    });
                Some((type_score + usize::from(union_round_trip), pointer))
            })
            .max_by_key(|(score, _)| *score)
            .map(|(_, pointer)| pointer.clone())
    }

    pub(super) fn known_bytes(
        &self,
        bytes: &[ByteCell],
        span: Span,
    ) -> Result<Vec<u8>, Diagnostic> {
        bytes.iter()
            .map(|byte| match byte {
                ByteCell::Known(byte) => Ok(*byte),
                ByteCell::Indeterminate => Err(Diagnostic::ub(
                    "read of an indeterminate value with a potentially invalid object representation",
                    span,
                    Some("6.2.6.1p5-6"),
                )),
            })
            .collect()
    }

    pub(super) fn deserialize_stored_value(
        &self,
        ty: &CType,
        bytes: &[ByteCell],
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        let size = self.type_size_of(ty).ok_or_else(|| {
            Diagnostic::error(
                format!("{} does not have an object representation", ty),
                span,
            )
        })?;
        if bytes.len() < size {
            return Err(Diagnostic::error(
                "byte slice is too small for the destination type",
                span,
            ));
        }
        if bytes
            .iter()
            .all(|byte| matches!(byte, ByteCell::Indeterminate))
        {
            return Ok(match ty.unqualified() {
                CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                    self.indeterminate_stored_value(ty)
                }
                _ => StoredValue::Indeterminate,
            });
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let stride = self.type_size_of(inner).ok_or_else(|| {
                    Diagnostic::error("array element type does not have size", span)
                })?;
                let mut values = Vec::with_capacity(*len);
                for index in 0..*len {
                    let start = index * stride;
                    values.push(self.deserialize_stored_value(
                        inner,
                        &bytes[start..start + stride],
                        span,
                    )?);
                }
                Ok(StoredValue::Array(values))
            }
            CType::Struct(_, _) => {
                let record = self
                    .record_type(ty)
                    .cloned()
                    .ok_or_else(|| Diagnostic::error("incomplete structure type", span))?;
                let mut values = Vec::with_capacity(record.members.len());
                for member in &record.members {
                    let member_size = if member.bit_width.is_some() {
                        member.bit_storage_size
                    } else if matches!(member.ty.unqualified(), CType::Array(_, 0)) {
                        0
                    } else {
                        self.type_size_of(&member.ty).ok_or_else(|| {
                            Diagnostic::error(
                                format!("{} does not have an object representation", member.ty),
                                span,
                            )
                        })?
                    };
                    let end = member.offset + member_size;
                    let member_value = if matches!(member.ty.unqualified(), CType::Array(_, 0)) {
                        StoredValue::Array(Vec::new())
                    } else if member.bit_width.is_some() {
                        self.deserialize_bit_field(member, &bytes[member.offset..end], span)?
                    } else {
                        self.deserialize_stored_value(&member.ty, &bytes[member.offset..end], span)?
                    };
                    values.push((member.storage_name.as_str().into(), member_value));
                }
                Ok(StoredValue::Record(values))
            }
            CType::Union(_, _) => {
                let record = self
                    .record_type(ty)
                    .cloned()
                    .ok_or_else(|| Diagnostic::error("incomplete union type", span))?;
                let members = record
                    .members
                    .iter()
                    .map(|member| {
                        (
                            member.storage_name.as_str().into(),
                            self.indeterminate_stored_value(&member.ty),
                        )
                    })
                    .collect();
                Ok(StoredValue::Union {
                    active_member: None,
                    members,
                    bytes: bytes[..size].to_vec(),
                })
            }
            _ => self.deserialize_scalar_value(ty, &bytes[..size], span),
        }
    }

    pub(super) fn deserialize_bit_field(
        &self,
        member: &RecordMember,
        bytes: &[ByteCell],
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        let Some(width) = member.bit_width else {
            return self.deserialize_stored_value(&member.ty, bytes, span);
        };
        if width == 0 {
            return Ok(StoredValue::Scalar(TypedValue::integer(
                member.ty.clone(),
                0,
            )));
        }
        if bytes
            .iter()
            .any(|byte| matches!(byte, ByteCell::Indeterminate))
        {
            return Ok(StoredValue::Indeterminate);
        }
        let mut raw = 0u128;
        for (index, byte) in bytes.iter().enumerate() {
            let ByteCell::Known(byte) = byte else {
                unreachable!();
            };
            raw |= (*byte as u128) << (index * 8);
        }
        let mask = self.integer_mask(width as u32);
        let field_bits = (raw >> member.bit_offset) & mask;
        let value = if matches!(member.ty.unqualified(), CType::Bool) {
            if field_bits > 1 {
                return Err(Diagnostic::ub(
                    "read of an indeterminate _Bool with a potentially invalid object representation",
                    span,
                    Some("6.2.6.1p5-6"),
                ));
            }
            field_bits as i128
        } else if member.ty.is_unsigned_integer() {
            field_bits as i128
        } else {
            let sign_bit = 1u128 << (width - 1);
            if field_bits & sign_bit != 0 {
                (field_bits as i128) - ((mask + 1) as i128)
            } else {
                field_bits as i128
            }
        };
        Ok(StoredValue::Scalar(TypedValue::integer(
            member.ty.clone(),
            value,
        )))
    }

    fn deserialize_scalar_value(
        &self,
        ty: &CType,
        bytes: &[ByteCell],
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        if bytes
            .iter()
            .any(|byte| matches!(byte, ByteCell::Indeterminate))
        {
            return Ok(StoredValue::Scalar(TypedValue::object_representation(
                ty.clone(),
                bytes.to_vec(),
            )));
        }
        match ty.unqualified() {
            CType::Bool => {
                let byte = match bytes[0] {
                    ByteCell::Known(byte) => byte,
                    ByteCell::Indeterminate => unreachable!(),
                };
                if byte > 1 {
                    return Ok(StoredValue::Scalar(TypedValue::object_representation(
                        ty.clone(),
                        bytes.to_vec(),
                    )));
                }
                Ok(StoredValue::Scalar(TypedValue::integer(
                    ty.clone(),
                    byte as i128,
                )))
            }
            _ if ty.is_integer() => {
                let mut raw = 0u128;
                for (index, byte) in bytes.iter().enumerate() {
                    let ByteCell::Known(byte) = byte else {
                        unreachable!();
                    };
                    raw |= (*byte as u128) << (index * 8);
                }
                Ok(StoredValue::Scalar(TypedValue::integer(
                    ty.clone(),
                    ty.normalize_integer_value(raw as i128).unwrap(),
                )))
            }
            CType::Float => {
                let raw = self.known_bytes(bytes, span)?.try_into().unwrap();
                Ok(StoredValue::Scalar(TypedValue::floating(
                    ty.clone(),
                    f32::from_bits(u32::from_le_bytes(raw)) as f64,
                )))
            }
            CType::Double => {
                let raw = self.known_bytes(bytes, span)?.try_into().unwrap();
                Ok(StoredValue::Scalar(TypedValue::floating(
                    ty.clone(),
                    f64::from_bits(u64::from_le_bytes(raw)),
                )))
            }
            CType::LongDouble => {
                let raw: [u8; 8] = self.known_bytes(&bytes[..8], span)?.try_into().unwrap();
                Ok(StoredValue::Scalar(TypedValue::floating(
                    ty.clone(),
                    f64::from_bits(u64::from_le_bytes(raw)),
                )))
            }
            CType::Complex(component_ty) => {
                let component_size = self.type_size_of(component_ty).ok_or_else(|| {
                    Diagnostic::error("complex component type is incomplete", span)
                })?;
                let decode_component = |component_bytes: &[ByteCell]| -> Result<f64, Diagnostic> {
                    match component_ty.unqualified() {
                        CType::Float => {
                            let raw = self.known_bytes(component_bytes, span)?.try_into().unwrap();
                            Ok(f32::from_bits(u32::from_le_bytes(raw)) as f64)
                        }
                        CType::Double => {
                            let raw = self.known_bytes(component_bytes, span)?.try_into().unwrap();
                            Ok(f64::from_bits(u64::from_le_bytes(raw)))
                        }
                        CType::LongDouble => {
                            let raw: [u8; 8] = self
                                .known_bytes(&component_bytes[..8], span)?
                                .try_into()
                                .unwrap();
                            Ok(f64::from_bits(u64::from_le_bytes(raw)))
                        }
                        _ => Err(Diagnostic::error(
                            "complex object representation requires a real floating component type",
                            span,
                        )),
                    }
                };
                let real = decode_component(&bytes[..component_size])?;
                let imag = decode_component(&bytes[component_size..component_size * 2])?;
                Ok(StoredValue::Scalar(TypedValue::complex(
                    ty.clone(),
                    real,
                    imag,
                )))
            }
            CType::VaList => {
                let raw = self.known_bytes(bytes, span)?.try_into().unwrap();
                Ok(StoredValue::Scalar(
                    self.va_list_value(u64::from_le_bytes(raw)),
                ))
            }
            CType::Pointer(_) => self.decode_pointer_bytes(ty, bytes, span),
            _ => Err(Diagnostic::error(
                format!("{} does not have a supported scalar representation", ty),
                span,
            )),
        }
    }

    pub(super) fn extract_stored_subobject(
        &self,
        stored: &StoredValue,
        ty: &CType,
        path: &[String],
        span: Span,
    ) -> Result<(StoredValue, CType), Diagnostic> {
        if path.is_empty() {
            return Ok((stored.clone(), ty.clone()));
        }
        let member_name = &path[0];
        match (stored, ty.unqualified()) {
            (StoredValue::Array(_), CType::Array(inner, _)) => {
                self.extract_stored_subobject(stored, inner, path, span)
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.record_slot(values, member_name, span)?;
                self.extract_stored_subobject(slot, &member_ty, &path[1..], span)
            }
            (StoredValue::Union { .. }, CType::Union(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.extract_union_member(stored, member, span)?;
                self.extract_stored_subobject(&slot, &member_ty, &path[1..], span)
            }
            _ => Err(Diagnostic::ub(
                "pointer or lvalue does not actually designate a structure or union object of the required type",
                span,
                Some("6.5.2.3"),
            )),
        }
    }

    pub(super) fn extract_union_member(
        &self,
        stored: &StoredValue,
        member: &RecordMember,
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        let StoredValue::Union { bytes, .. } = stored else {
            return Err(Diagnostic::ub(
                "pointer or lvalue does not actually designate a structure or union object of the required type",
                span,
                Some("6.5.2.3"),
            ));
        };
        let member_size = if member.bit_width.is_some() {
            member.bit_storage_size
        } else {
            self.type_size_of(&member.ty).unwrap()
        };
        let end = member.offset + member_size;
        if member.bit_width.is_some() {
            self.deserialize_bit_field(member, &bytes[member.offset..end], span)
        } else {
            self.deserialize_stored_value(&member.ty, &bytes[member.offset..end], span)
        }
    }

    pub(super) fn apply_owned_lvalue_offset(
        &self,
        stored: StoredValue,
        ty: &CType,
        offset: isize,
        span: Span,
    ) -> Result<(StoredValue, CType), Diagnostic> {
        match ty.unqualified() {
            CType::Array(inner, _) => {
                let StoredValue::Array(values) = stored else {
                    return Err(Diagnostic::error(
                        "array object does not have array storage",
                        span,
                    ));
                };
                let index = usize::try_from(offset).map_err(|_| {
                    Diagnostic::ub(
                        "array subscript is outside the bounds of the object",
                        span,
                        Some("6.5.6"),
                    )
                })?;
                let value = values.get(index).cloned().ok_or_else(|| {
                    Diagnostic::ub(
                        "array subscript is outside the bounds of the object",
                        span,
                        Some("6.5.6"),
                    )
                })?;
                Ok((value, (**inner).clone()))
            }
            _ if offset == 0 => Ok((stored, ty.clone())),
            _ => Err(Diagnostic::ub(
                "pointer arithmetic produced an invalid scalar object offset",
                span,
                Some("6.5.6"),
            )),
        }
    }

    pub(super) fn normalize_lvalue_base_offset(&self, lvalue: &mut LValue, objects: &ObjectFrames) {
        if lvalue.offset == 0 || lvalue.byte_offset_override.is_some() {
            return;
        }
        let root_is_array = self
            .lookup_object(objects, lvalue.object)
            .is_some_and(|object| matches!(object.ty.unqualified(), CType::Array(_, _)));
        if root_is_array
            && (!lvalue.member_path.is_empty()
                || matches!(lvalue.ty.unqualified(), CType::Array(_, _)))
        {
            lvalue.base_offset += lvalue.offset;
            lvalue.offset = 0;
        }
    }

    pub(super) fn rebase_array_lvalue(
        &self,
        lvalue: &mut LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let default_root = self
            .lookup_object(objects, lvalue.object)
            .map(|object| &object.ty)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "array lvalue refers to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        if matches!(lvalue.ty.unqualified(), CType::Array(_, 0)) {
            return Ok(());
        }
        let current_root = lvalue.designated_root_ty.as_deref().unwrap_or(default_root);
        if self.compatible_object_layout_types(current_root, &lvalue.ty) {
            // A typed array lvalue reconstructed over raw allocated storage already
            // has the right root type, but it still needs its own arithmetic domain.
            // Otherwise decaying one selected row would inherit the allocation's full
            // byte extent and allow element pointers to walk into adjacent rows.
            if lvalue.arithmetic_domain_start.is_none()
                && let Some(start) = lvalue.byte_offset_override
            {
                lvalue.arithmetic_domain_start = Some(start);
            }
            return Ok(());
        }
        let (_, start, _) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "array lvalue does not designate a valid subarray",
                    span,
                    Some("6.5.2.1"),
                )
            })?;
        lvalue.base_offset = 0;
        lvalue.offset = 0;
        Rc::make_mut(&mut lvalue.member_path).clear();
        lvalue.designated_root_ty = Some(Rc::new(lvalue.ty.clone()));
        lvalue.byte_offset_override = Some(start);
        lvalue.arithmetic_domain_start = Some(start);
        Ok(())
    }
}
