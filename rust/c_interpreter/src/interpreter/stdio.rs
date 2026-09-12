use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn check_puts_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("puts", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "puts")?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "puts requires a const char * argument",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("puts", value, args[0].span(), objects)?;
        let pointer = value.as_pointer(args[0].span())?;
        let _ = self.read_c_string_bytes(pointer, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_remove_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("remove", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "remove")?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "remove requires a const char * filename",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("remove", value, args[0].span(), objects)?;
        let _ =
            self.read_c_string_bytes(value.as_pointer(args[0].span())?, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_rename_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("rename", args, evaluated, 2, span)?;
        self.check_remove_call(&args[..1], &evaluated[..1], span, objects)?;
        self.check_remove_call(&args[1..], &evaluated[1..], span, objects)
    }

    pub(super) fn check_tmpfile_call(
        &self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !args.is_empty() || !evaluated.is_empty() {
            return Err(Diagnostic::error("tmpfile requires no arguments", span));
        }
        Ok(())
    }

    pub(super) fn check_tmpnam_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("tmpnam", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "tmpnam")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &value.ty)
            && !value.as_pointer(args[0].span())?.is_null()
        {
            return Err(Diagnostic::error(
                "tmpnam requires a char * argument or a null pointer",
                args[0].span(),
            ));
        }
        let pointer = value.as_pointer(args[0].span())?;
        if !pointer.is_null() {
            let _ = self.ensure_library_array_destination(
                &pointer,
                HOST_L_TMPNAM,
                1,
                args[0].span(),
                objects,
            )?;
        }
        Ok(())
    }

    pub(super) fn check_stream_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), function_name)?;
        let Some(stream) =
            self.stream_object_from_value(value, args[0].span(), function_name, standard)?
        else {
            return Err(self.stream_pointer_diag(function_name, args[0].span(), standard));
        };
        self.require_open_stream(stream, args[0].span(), function_name, standard)?;
        if !matches!(function_name, "setbuf" | "setvbuf") {
            self.host_streams
                .get_mut(&stream)
                .expect("validated host stream exists")
                .operation_performed = true;
        }
        Ok(())
    }

    pub(super) fn check_fclose_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.check_stream_call("fclose", args, evaluated, span, "7.19.5.1")
    }

    pub(super) fn check_fflush_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fflush", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "fflush")?;
        if value.as_pointer(args[0].span())?.is_null() {
            let affected = self
                .host_streams
                .iter()
                .filter_map(|(object, stream)| {
                    (stream.open
                        && matches!(
                            stream.mode.kind,
                            HostStreamModeKind::Output | HostStreamModeKind::Update
                        ))
                    .then_some(*object)
                })
                .collect::<Vec<_>>();
            for stream in affected {
                self.require_open_stream(stream, args[0].span(), "fflush", "7.19.5.2")?;
                self.host_streams
                    .get_mut(&stream)
                    .expect("validated host stream exists")
                    .operation_performed = true;
            }
            return Ok(());
        }
        let Some(stream) =
            self.stream_object_from_value(value, args[0].span(), "fflush", "7.19.5.2")?
        else {
            return Ok(());
        };
        self.require_open_stream(stream, args[0].span(), "fflush", "7.19.5.2")?;
        if let Some(stream_state) = self.host_streams.get(&stream)
            && (stream_state.mode.kind == HostStreamModeKind::Input
                || (stream_state.mode.kind == HostStreamModeKind::Update
                    && stream_state.last_operation == Some(StreamLastOperation::Input)))
        {
            return Err(Diagnostic::ub(
                "fflush is undefined for an input stream or for an update stream whose most recent operation was input",
                args[0].span(),
                Some("7.19.5.2"),
            ));
        }
        self.host_streams
            .get_mut(&stream)
            .expect("validated host stream exists")
            .operation_performed = true;
        Ok(())
    }

    pub(super) fn check_fopen_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fopen", args, evaluated, 2, span)?;
        self.check_remove_call(&args[..1], &evaluated[..1], span, objects)?;
        self.check_remove_call(&args[1..], &evaluated[1..], span, objects)?;
        let mode = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        if self.parse_standard_fopen_mode(&mode).is_none() {
            return Err(Diagnostic::ub(
                "fopen mode string must exactly match one of the standard mode sequences",
                args[1].span(),
                Some("7.19.5.3"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_freopen_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("freopen", args, evaluated, 3, span)?;
        self.check_remove_call(&args[..1], &evaluated[..1], span, objects)?;
        self.check_remove_call(&args[1..2], &evaluated[1..2], span, objects)?;
        let mode = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        if self.parse_standard_fopen_mode(&mode).is_none() {
            return Err(Diagnostic::ub(
                "freopen mode string must exactly match one of the standard mode sequences",
                args[1].span(),
                Some("7.19.5.4"),
            ));
        }
        self.check_stream_call("freopen", &args[2..], &evaluated[2..], span, "7.19.5.4")
    }

    pub(super) fn check_setbuf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("setbuf", args, evaluated, 2, span)?;
        self.check_stream_call("setbuf", &args[..1], &evaluated[..1], span, "7.19.5.5")?;
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "setbuf", "7.19.5.5")?
            .expect("checked above");
        self.check_stream_buffer_configuration_timing(
            stream_object,
            args[0].span(),
            "setbuf",
            "7.19.5.5",
        )?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "setbuf")?;
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        if !pointer.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[1].ty)
        {
            return Err(Diagnostic::error(
                "setbuf buffer argument must be a char * or null pointer",
                args[1].span(),
            ));
        }
        if !pointer.is_null() {
            self.reject_nonvolatile_library_pointer_access(
                "setbuf",
                &evaluated[1],
                args[1].span(),
                objects,
            )?;
            let (object_id, start, _) = self.byte_region_from_pointer_allow_reserved(
                &pointer,
                HOST_BUFSIZ,
                args[1].span(),
                objects,
            )?;
            self.ensure_library_writable_region(
                object_id,
                start,
                HOST_BUFSIZ,
                args[1].span(),
                objects,
            )?;
        }
        Ok(())
    }

    fn check_stream_buffer_configuration_timing(
        &self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let stream = self
            .host_streams
            .get(&stream_object)
            .ok_or_else(|| self.stream_pointer_diag(function_name, span, standard))?;
        if stream.operation_performed || stream.buffer.is_some() {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} must be called after the stream is opened and before any other operation is performed on it"
                ),
                span,
                Some(standard),
            )
            .with_note("a previous unsuccessful call to setvbuf is the only permitted exception"));
        }
        Ok(())
    }

    pub(super) fn check_setvbuf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("setvbuf", args, evaluated, 4, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_missing_return_value(&evaluated[2], args[2].span())?;
        self.reject_missing_return_value(&evaluated[3], args[3].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "setvbuf")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "setvbuf")?;
        self.reject_indeterminate_library_value(&evaluated[2], args[2].span(), "setvbuf")?;
        self.reject_indeterminate_library_value(&evaluated[3], args[3].span(), "setvbuf")?;
        if evaluated[2].ty != CType::Int {
            return Err(Diagnostic::error(
                "setvbuf mode argument must have type int",
                args[2].span(),
            ));
        }
        if evaluated[3].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "setvbuf size argument must have type unsigned long",
                args[3].span(),
            ));
        }
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        if !pointer.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[1].ty)
        {
            return Err(Diagnostic::error(
                "setvbuf buffer argument must be a char * or null pointer",
                args[1].span(),
            ));
        }
        self.check_stream_call("setvbuf", &args[..1], &evaluated[..1], span, "7.19.5.6")?;
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "setvbuf", "7.19.5.6")?
            .expect("checked above");
        self.check_stream_buffer_configuration_timing(
            stream_object,
            args[0].span(),
            "setvbuf",
            "7.19.5.6",
        )?;
        let mode = evaluated[2].to_int()? as c_int;
        if !matches!(mode, libc::_IOFBF | libc::_IOLBF | libc::_IONBF) {
            return Ok(());
        }
        let size =
            self.checked_usize_from_unsigned_long(&evaluated[3], args[3].span(), "setvbuf size")?;
        if !pointer.is_null() && mode != libc::_IONBF {
            let validation_size = size.max(1);
            let (object_id, start, _) = self.byte_region_from_pointer_allow_reserved(
                &pointer,
                validation_size,
                args[1].span(),
                objects,
            )?;
            self.ensure_writable_region(
                object_id,
                start,
                validation_size,
                args[1].span(),
                objects,
            )?;
            if size != 0 {
                self.ensure_library_writable_region(
                    object_id,
                    start,
                    size,
                    args[1].span(),
                    objects,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn check_fputc_like_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} character argument must have type int"),
                args[0].span(),
            ));
        }
        self.check_stream_call(function_name, &args[1..], &evaluated[1..], span, standard)
    }

    pub(super) fn check_fputs_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fputs", args, evaluated, 2, span)?;
        self.check_puts_call(&args[..1], &evaluated[..1], span, objects)?;
        self.check_stream_call("fputs", &args[1..], &evaluated[1..], span, "7.19.7.4")
    }

    pub(super) fn check_fgetc_like_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.check_stream_call(function_name, args, evaluated, span, standard)
    }

    pub(super) fn check_fgets_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fgets", args, evaluated, 3, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "fgets")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "fgets")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "fgets requires a char * destination",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            "fgets",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "fgets count argument must have type int",
                args[1].span(),
            ));
        }
        let count = evaluated[1].to_int()?;
        if count <= 0 {
            return Err(Diagnostic::ub(
                "fgets count must be positive",
                args[1].span(),
                Some("7.21.7.2"),
            ));
        }
        let count = usize::try_from(count).map_err(|_| {
            Diagnostic::error(
                "fgets destination size is out of supported range",
                args[1].span(),
            )
        })?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&destination, count, 1, args[0].span(), objects)?;
        self.check_stream_call("fgets", &args[2..], &evaluated[2..], span, "7.19.7.2")
    }

    pub(super) fn check_gets_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("gets", args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "gets")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "gets requires a char * argument",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            "gets",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&destination, 1, 1, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_fread_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fread", args, evaluated, 4, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_missing_return_value(&evaluated[2], args[2].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "fread")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "fread")?;
        self.reject_indeterminate_library_value(&evaluated[2], args[2].span(), "fread")?;
        if !self.pointer_assignment_compatible(&self.void_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "fread requires a void * destination",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            "fread",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        if evaluated[1].ty != CType::UnsignedLong || evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "fread size and count arguments must have type unsigned long",
                args[1].span().merge(args[2].span()),
            ));
        }
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "fread element size",
        )?;
        let count = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "fread element count",
        )?;
        let total = size.checked_mul(count).ok_or_else(|| {
            Diagnostic::error("fread total byte count is out of supported range", span)
        })?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&destination, total, 1, args[0].span(), objects)?;
        self.check_stream_call("fread", &args[3..], &evaluated[3..], span, "7.19.8.1")
    }

    pub(super) fn check_fwrite_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fwrite", args, evaluated, 4, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_missing_return_value(&evaluated[2], args[2].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "fwrite")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "fwrite")?;
        self.reject_indeterminate_library_value(&evaluated[2], args[2].span(), "fwrite")?;
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "fwrite requires a const void * source",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            "fwrite",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        if evaluated[1].ty != CType::UnsignedLong || evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "fwrite size and count arguments must have type unsigned long",
                args[1].span().merge(args[2].span()),
            ));
        }
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "fwrite element size",
        )?;
        let count = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "fwrite element count",
        )?;
        let total = size.checked_mul(count).ok_or_else(|| {
            Diagnostic::error("fwrite total byte count is out of supported range", span)
        })?;
        let source = evaluated[0].as_pointer(args[0].span())?;
        let _ = self.library_array_region(&source, total, 1, args[0].span(), objects)?;
        self.check_stream_call("fwrite", &args[3..], &evaluated[3..], span, "7.19.8.2")
    }

    pub(super) fn check_fseek_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fseek", args, evaluated, 3, span)?;
        self.check_stream_call("fseek", &args[..1], &evaluated[..1], span, "7.19.9.2")?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_missing_return_value(&evaluated[2], args[2].span())?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "fseek")?;
        self.reject_indeterminate_library_value(&evaluated[2], args[2].span(), "fseek")?;
        if evaluated[1].ty != CType::Long || evaluated[2].ty != CType::Int {
            return Err(Diagnostic::error(
                "fseek requires (FILE *, long, int)",
                span,
            ));
        }
        let origin = evaluated[2].to_int()? as c_int;
        if !matches!(origin, libc::SEEK_SET | libc::SEEK_CUR | libc::SEEK_END) {
            return Err(Diagnostic::ub(
                "fseek origin must be SEEK_SET, SEEK_CUR, or SEEK_END",
                args[2].span(),
                Some("7.21.9.2"),
            ));
        }
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fseek", "7.21.9.2")?
            .expect("checked stream pointer");
        let stream = self
            .host_streams
            .get(&stream_object)
            .expect("checked stream object");
        let offset = evaluated[1].to_int()?;
        if !stream.mode.binary
            && offset != 0
            && (origin != libc::SEEK_SET
                || !self.ftell_provenance.contains(&(stream.backing, offset)))
        {
            return Err(Diagnostic::ub(
                "fseek on a text stream requires a zero offset or an offset returned by ftell for the same file with SEEK_SET",
                args[1].span().merge(args[2].span()),
                Some("7.21.9.2"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_fsetpos_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fsetpos", args, evaluated, 2, span)?;
        self.check_stream_call("fsetpos", &args[..1], &evaluated[..1], span, "7.19.9.4")?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "fsetpos")?;
        let expected = CType::pointer_to(CType::qualified(
            CType::Long,
            crate::types::TypeQualifiers {
                is_atomic: false,
                is_const: true,
                is_restrict: false,
                is_volatile: false,
            },
        ));
        if !self.pointer_assignment_compatible(&expected, &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "fsetpos requires a const fpos_t * position pointer",
                args[1].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            "fsetpos",
            &evaluated[1],
            args[1].span(),
            objects,
        )?;
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 8, args[1].span(), objects)?;
        let version = self
            .lookup_object(objects, object_id)
            .expect("validated fpos_t object")
            .modification_count;
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fsetpos", "7.21.9.3")?
            .expect("checked stream pointer");
        let backing = self
            .host_streams
            .get(&stream_object)
            .expect("checked stream object")
            .backing;
        if self.fpos_provenance.get(&(object_id, start)).copied() != Some((version, backing)) {
            return Err(Diagnostic::ub(
                "fsetpos requires an unmodified fpos_t value produced by fgetpos for a stream associated with the same file",
                args[1].span(),
                Some("7.21.9.3"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_fgetpos_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fgetpos", args, evaluated, 2, span)?;
        self.check_stream_call("fgetpos", &args[..1], &evaluated[..1], span, "7.19.9.1")?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "fgetpos")?;
        if !self.pointer_assignment_compatible(&CType::pointer_to(CType::Long), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "fgetpos requires an fpos_t * output pointer",
                args[1].span(),
            ));
        }
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 8, args[1].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, 8, args[1].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_ungetc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.check_fputc_like_call("ungetc", args, evaluated, span, "7.19.7.10")
    }

    pub(super) fn check_ftell_like_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.check_stream_call(function_name, args, evaluated, span, standard)
    }

    pub(super) fn check_clearerr_like_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.check_stream_call(function_name, args, evaluated, span, standard)
    }

    pub(super) fn check_perror_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("perror", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "perror")?;
        let pointer = value.as_pointer(args[0].span())?;
        if pointer.is_null() {
            return Ok(());
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "perror requires a const char * argument or a null pointer",
                args[0].span(),
            ));
        }
        let _ = self.read_c_string_bytes(pointer, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_putchar_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("putchar", args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "putchar")?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                "putchar character argument must have type int",
                args[0].span(),
            ));
        }
        Ok(())
    }

    pub(super) fn check_calloc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("calloc", args, evaluated, 2, span)?;
        let mut sizes = [0usize; 2];
        for (index, (expr, value)) in args.iter().zip(evaluated).enumerate() {
            self.check_library_argument_value(value, expr.span(), "calloc")?;
            if value.ty != CType::UnsignedLong {
                return Err(Diagnostic::error(
                    "calloc arguments must have type unsigned long",
                    expr.span(),
                ));
            }
            sizes[index] =
                self.checked_usize_from_unsigned_long(value, expr.span(), "calloc size")?;
        }
        self.pending_heap_call = Some(PendingHeapCall::Calloc(sizes[0].checked_mul(sizes[1])));
        Ok(())
    }

    pub(super) fn check_realloc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("realloc", args, evaluated, 2, span)?;
        let ptr_value = &evaluated[0];
        let size_value = &evaluated[1];
        self.reject_missing_return_value(ptr_value, args[0].span())?;
        self.reject_missing_return_value(size_value, args[1].span())?;
        self.reject_indeterminate_library_value(ptr_value, args[0].span(), "realloc")?;
        self.reject_indeterminate_library_value(size_value, args[1].span(), "realloc")?;
        if !self.pointer_assignment_compatible(&self.void_ptr_type(), &ptr_value.ty) {
            return Err(Diagnostic::error(
                "realloc requires a void * argument",
                args[0].span(),
            ));
        }
        if size_value.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "realloc size argument must have type unsigned long",
                args[1].span(),
            ));
        }
        let pointer = ptr_value.as_pointer(args[0].span())?;
        let object = self.validate_heap_allocation_pointer(&pointer, args[0].span(), objects)?;
        let size =
            self.checked_usize_from_unsigned_long(size_value, args[1].span(), "realloc size")?;
        self.pending_heap_call = Some(PendingHeapCall::Realloc { object, size });
        Ok(())
    }

    pub(super) fn check_memset_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("memset", args, evaluated, 3, span)?;
        let dest = &evaluated[0];
        let ch = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(ch, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "memset")?;
        self.reject_indeterminate_library_value(ch, args[1].span(), "memset")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "memset")?;
        if !self.pointer_assignment_compatible(&self.void_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "memset requires a void * destination",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("memset", dest, args[0].span(), objects)?;
        if ch.ty != CType::Int {
            return Err(Diagnostic::error(
                "memset fill argument must have type int",
                args[1].span(),
            ));
        }
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "memset requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "memset byte count")?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let _ = self.ensure_library_untyped_byte_destination(
            &dest_pointer,
            size,
            args[0].span(),
            objects,
        )?;
        Ok(())
    }

    pub(super) fn check_memchr_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("memchr", args, evaluated, 3, span)?;
        let value = &evaluated[0];
        let ch = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(value, args[0].span())?;
        self.reject_missing_return_value(ch, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(value, args[0].span(), "memchr")?;
        self.reject_indeterminate_library_value(ch, args[1].span(), "memchr")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "memchr")?;
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "memchr requires a const void * argument",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("memchr", value, args[0].span(), objects)?;
        if ch.ty != CType::Int {
            return Err(Diagnostic::error(
                "memchr search byte must have type int",
                args[1].span(),
            ));
        }
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "memchr requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "memchr byte count")?;
        let pointer = value.as_pointer(args[0].span())?;
        let _ = self.library_untyped_byte_region(&pointer, size, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_memcmp_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("memcmp", args, evaluated, 3, span)?;
        let lhs = &evaluated[0];
        let rhs = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(lhs, args[0].span())?;
        self.reject_missing_return_value(rhs, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(lhs, args[0].span(), "memcmp")?;
        self.reject_indeterminate_library_value(rhs, args[1].span(), "memcmp")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "memcmp")?;
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &lhs.ty) {
            return Err(Diagnostic::error(
                "memcmp requires a const void * first argument",
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("memcmp", lhs, args[0].span(), objects)?;
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &rhs.ty) {
            return Err(Diagnostic::error(
                "memcmp requires a const void * second argument",
                args[1].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access("memcmp", rhs, args[1].span(), objects)?;
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "memcmp requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "memcmp byte count")?;
        let lhs_pointer = lhs.as_pointer(args[0].span())?;
        let rhs_pointer = rhs.as_pointer(args[1].span())?;
        let _ = self.library_untyped_byte_region(&lhs_pointer, size, args[0].span(), objects)?;
        let _ = self.library_untyped_byte_region(&rhs_pointer, size, args[1].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_strlen_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strlen", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "strlen")?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "strlen requires a const char * argument",
                args[0].span(),
            ));
        }
        let pointer = value.as_pointer(args[0].span())?;
        let _ = self.read_c_string_bytes(pointer, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_binary_c_string_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), function_name)?;
            if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
                return Err(Diagnostic::error(
                    format!("{function_name} requires const char * arguments"),
                    expr.span(),
                ));
            }
            let pointer = value.as_pointer(expr.span())?;
            let _ = self.read_c_string_bytes(pointer, expr.span(), objects)?;
        }
        Ok(())
    }

    pub(super) fn check_c_string_character_search_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        let value = &evaluated[0];
        let character = &evaluated[1];
        self.reject_missing_return_value(value, args[0].span())?;
        self.reject_missing_return_value(character, args[1].span())?;
        self.reject_indeterminate_library_value(value, args[0].span(), function_name)?;
        self.reject_indeterminate_library_value(character, args[1].span(), function_name)?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a const char * argument"),
                args[0].span(),
            ));
        }
        if character.ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} search character must have type int"),
                args[1].span(),
            ));
        }
        let pointer = value.as_pointer(args[0].span())?;
        let _ = self.read_c_string_bytes(pointer, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_strncmp_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strncmp", args, evaluated, 3, span)?;
        let count = &evaluated[2];
        self.check_library_argument_value(count, args[2].span(), "strncmp")?;
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "strncmp requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "strncmp byte count")?;
        for (expr, value) in args[..2].iter().zip(&evaluated[..2]) {
            self.check_library_argument_value(value, expr.span(), "strncmp")?;
            if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
                return Err(Diagnostic::error(
                    "strncmp requires const char * arguments",
                    expr.span(),
                ));
            }
            let pointer = value.as_pointer(expr.span())?;
            let _ =
                self.byte_region_from_pointer_bounded_string(&pointer, size, expr.span(), objects)?;
        }
        Ok(())
    }

    pub(super) fn check_strcpy_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strcpy", args, evaluated, 2, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "strcpy")?;
        self.reject_indeterminate_library_value(src, args[1].span(), "strcpy")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "strcpy requires a char * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                "strcpy requires a const char * source",
                args[1].span(),
            ));
        }
        let src_pointer = src.as_pointer(args[1].span())?;
        let src_bytes = self.read_c_string_bytes(src_pointer.clone(), args[1].span(), objects)?;
        let copy_size = src_bytes.len() + 1;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, copy_size, args[0].span(), objects)?;
        let (src_object_id, src_start, _) =
            self.byte_region_from_pointer(&src_pointer, copy_size, args[1].span(), objects)?;
        if self.ranges_overlap(
            dest_object_id,
            dest_start,
            src_object_id,
            src_start,
            copy_size,
        ) {
            return Err(Diagnostic::ub(
                "strcpy source and destination regions overlap",
                span,
                Some("7.21.2.3"),
            ));
        }
        self.ensure_library_writable_region(
            dest_object_id,
            dest_start,
            copy_size,
            args[0].span(),
            objects,
        )?;
        Ok(())
    }

    pub(super) fn check_strcat_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strcat", args, evaluated, 2, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "strcat")?;
        self.reject_indeterminate_library_value(src, args[1].span(), "strcat")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "strcat requires a char * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                "strcat requires a const char * source",
                args[1].span(),
            ));
        }
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        let dest_bytes = self.read_c_string_bytes(dest_pointer.clone(), args[0].span(), objects)?;
        let src_bytes = self.read_c_string_bytes(src_pointer.clone(), args[1].span(), objects)?;
        let total = dest_bytes.len() + src_bytes.len() + 1;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
        let (src_object_id, src_start, _) = self.byte_region_from_pointer(
            &src_pointer,
            src_bytes.len() + 1,
            args[1].span(),
            objects,
        )?;
        if self.ranges_overlap(
            dest_object_id,
            dest_start + dest_bytes.len(),
            src_object_id,
            src_start,
            src_bytes.len() + 1,
        ) {
            return Err(Diagnostic::ub(
                "strcat source and destination regions overlap",
                span,
                Some("7.21.3.1"),
            ));
        }
        self.ensure_library_writable_region(
            dest_object_id,
            dest_start,
            total,
            args[0].span(),
            objects,
        )?;
        Ok(())
    }

    pub(super) fn check_strncpy_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strncpy", args, evaluated, 3, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "strncpy")?;
        self.reject_indeterminate_library_value(src, args[1].span(), "strncpy")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "strncpy")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "strncpy requires a char * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                "strncpy requires a const char * source",
                args[1].span(),
            ));
        }
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "strncpy requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "strncpy byte count")?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        let (dest_object_id, dest_start, _) =
            self.ensure_library_array_destination(&dest_pointer, size, 1, args[0].span(), objects)?;
        let _ = self.library_array_region(&src_pointer, size, 1, args[1].span(), objects)?;
        let (src_object_id, src_start, _) = self.byte_region_from_pointer_bounded_string(
            &src_pointer,
            size,
            args[1].span(),
            objects,
        )?;
        if self.ranges_overlap(dest_object_id, dest_start, src_object_id, src_start, size) {
            return Err(Diagnostic::ub(
                "strncpy source and destination regions overlap",
                span,
                Some("7.21.2.4"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_strncat_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strncat", args, evaluated, 3, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "strncat")?;
        self.reject_indeterminate_library_value(src, args[1].span(), "strncat")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "strncat")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "strncat requires a char * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                "strncat requires a const char * source",
                args[1].span(),
            ));
        }
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "strncat requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "strncat byte count")?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        let _ = self.library_array_region(&src_pointer, size, 1, args[1].span(), objects)?;
        let dest_bytes = self.read_c_string_bytes(dest_pointer.clone(), args[0].span(), objects)?;
        let (src_object_id, src_start, _) = self.byte_region_from_pointer_bounded_string(
            &src_pointer,
            size,
            args[1].span(),
            objects,
        )?;
        let src_buffer =
            self.read_bounded_c_string_source(src_pointer, size, args[1].span(), objects)?;
        let nul_pos = src_buffer.iter().position(|&byte| byte == 0);
        let copy_len = nul_pos.unwrap_or(size);
        let src_access_size = nul_pos.map_or(size, |pos| pos + 1);
        let total = dest_bytes.len() + copy_len + 1;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
        if self.ranges_overlap_with_sizes(
            dest_object_id,
            dest_start + dest_bytes.len(),
            copy_len + 1,
            src_object_id,
            src_start,
            src_access_size,
        ) {
            return Err(Diagnostic::ub(
                "strncat source and destination regions overlap",
                span,
                Some("7.21.3.2"),
            ));
        }
        self.ensure_library_writable_region(
            dest_object_id,
            dest_start,
            total,
            args[0].span(),
            objects,
        )?;
        Ok(())
    }

    pub(super) fn check_strxfrm_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strxfrm", args, evaluated, 3, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), "strxfrm")?;
        self.reject_indeterminate_library_value(src, args[1].span(), "strxfrm")?;
        self.reject_indeterminate_library_value(count, args[2].span(), "strxfrm")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                "strxfrm requires a char * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                "strxfrm requires a const char * source",
                args[1].span(),
            ));
        }
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "strxfrm requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let size =
            self.checked_usize_from_unsigned_long(count, args[2].span(), "strxfrm byte count")?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        // C11 7.24.4.5 permits a null destination for sizing, but still
        // requires a valid, terminated source string.
        if size == 0 && dest_pointer.is_null() {
            self.read_c_string_bytes(src_pointer, args[1].span(), objects)?;
            return Ok(());
        }
        let (dest_object_id, dest_start, _) =
            self.ensure_library_array_destination(&dest_pointer, size, 1, args[0].span(), objects)?;
        let src_bytes = self.read_c_string_bytes(src_pointer.clone(), args[1].span(), objects)?;
        let (src_object_id, src_start, _) = self.byte_region_from_pointer(
            &src_pointer,
            src_bytes.len() + 1,
            args[1].span(),
            objects,
        )?;
        if self.ranges_overlap_with_sizes(
            dest_object_id,
            dest_start,
            size,
            src_object_id,
            src_start,
            src_bytes.len() + 1,
        ) {
            return Err(Diagnostic::ub(
                "strxfrm source and destination regions overlap",
                span,
                Some("7.21.4.5"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_strerror_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strerror", args, evaluated, 1, span)?;
        let errnum = &evaluated[0];
        self.check_library_argument_value(errnum, args[0].span(), "strerror")?;
        if errnum.ty != CType::Int {
            return Err(Diagnostic::error(
                "strerror requires an int argument",
                args[0].span(),
            ));
        }
        Ok(())
    }

    pub(super) fn check_strdup_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strdup", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "strdup")?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "strdup requires a const char * argument",
                args[0].span(),
            ));
        }
        let pointer = value.as_pointer(args[0].span())?;
        let _ = self.read_c_string_bytes(pointer, args[0].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_strtok_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strtok", args, evaluated, 2, span)?;
        let value = &evaluated[0];
        let delim = &evaluated[1];
        self.reject_missing_return_value(value, args[0].span())?;
        self.reject_missing_return_value(delim, args[1].span())?;
        self.reject_indeterminate_library_value(value, args[0].span(), "strtok")?;
        self.reject_indeterminate_library_value(delim, args[1].span(), "strtok")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                "strtok requires a char * first argument",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &delim.ty) {
            return Err(Diagnostic::error(
                "strtok requires a const char * delimiter",
                args[1].span(),
            ));
        }
        let delim_pointer = delim.as_pointer(args[1].span())?;
        let delim_bytes =
            self.read_c_string_bytes(delim_pointer.clone(), args[1].span(), objects)?;
        let source_pointer = value.as_pointer(args[0].span())?;
        if source_pointer.is_null() {
            if let Some(state) = self.strtok_state.clone() {
                let source_bytes =
                    self.read_c_string_bytes(state.clone(), args[0].span(), objects)?;
                let _ = self.ensure_library_array_destination(
                    &state,
                    source_bytes.len() + 1,
                    1,
                    args[0].span(),
                    objects,
                )?;
                self.check_strtok_restrict_overlap(
                    &state,
                    &source_bytes,
                    &delim_pointer,
                    &delim_bytes,
                    span,
                    args,
                    objects,
                )?;
            } else if !self.strtok_started {
                return Err(Diagnostic::ub(
                    "strtok first argument is null without a preceding call that supplied a string",
                    args[0].span(),
                    Some("7.24.5.8"),
                ));
            }
        } else {
            let source_bytes =
                self.read_c_string_bytes(source_pointer.clone(), args[0].span(), objects)?;
            let _ = self.ensure_library_array_destination(
                &source_pointer,
                source_bytes.len() + 1,
                1,
                args[0].span(),
                objects,
            )?;
            self.check_strtok_restrict_overlap(
                &source_pointer,
                &source_bytes,
                &delim_pointer,
                &delim_bytes,
                span,
                args,
                objects,
            )?;
        }
        Ok(())
    }

    fn check_strtok_restrict_overlap(
        &self,
        source_pointer: &PointerValue,
        source_bytes: &[u8],
        delim_pointer: &PointerValue,
        delim_bytes: &[u8],
        span: Span,
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let token_start = source_bytes
            .iter()
            .position(|byte| !delim_bytes.contains(byte))
            .unwrap_or(source_bytes.len());
        if token_start == source_bytes.len() {
            return Ok(());
        }
        let token_len = source_bytes[token_start..]
            .iter()
            .position(|byte| delim_bytes.contains(byte))
            .unwrap_or(source_bytes.len() - token_start);
        let delimiter_offset = token_start + token_len;
        if delimiter_offset >= source_bytes.len() {
            return Ok(());
        }
        let (source_object, source_start, _) =
            self.byte_region_from_pointer(source_pointer, 0, args[0].span(), objects)?;
        let (delim_object, delim_start, _) = self.byte_region_from_pointer(
            delim_pointer,
            delim_bytes.len() + 1,
            args[1].span(),
            objects,
        )?;
        if self.ranges_overlap_with_sizes(
            source_object,
            source_start + delimiter_offset,
            1,
            delim_object,
            delim_start,
            delim_bytes.len() + 1,
        ) {
            return Err(Diagnostic::ub(
                "strtok reads the delimiter string and writes through the source string, and those restrict-qualified accesses overlap",
                span,
                Some("6.7.3.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn void_pointer_value(&self, pointer: PointerValue) -> TypedValue {
        TypedValue::pointer(self.void_ptr_type(), pointer)
    }

    pub(super) fn pointer_value_with_type(&self, ty: CType, pointer: PointerValue) -> TypedValue {
        TypedValue::pointer(ty, pointer)
    }

    pub(super) fn host_function_return_type(
        &self,
        function_name: &str,
        file: FileId,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let decl = self
            .lookup_function_declaration(function_name, file)
            .ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "missing declaration for host library function {}",
                        function_name
                    ),
                    span,
                )
            })?;
        match self.function_declaration_type(decl).unqualified() {
            CType::Function(return_ty, _, _) => Ok((**return_ty).clone()),
            _ => Err(Diagnostic::error(
                format!("declaration for {} is not a function", function_name),
                span,
            )),
        }
    }

    pub(super) fn host_function_parameter_type(
        &self,
        function_name: &str,
        file: FileId,
        index: usize,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let decl = self
            .lookup_function_declaration(function_name, file)
            .ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "missing declaration for host library function {}",
                        function_name
                    ),
                    span,
                )
            })?;
        match self.function_declaration_type(decl).unqualified() {
            CType::Function(_, params, _) => params.get(index).cloned().ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "declaration for host library function {} is missing parameter {}",
                        function_name, index
                    ),
                    span,
                )
            }),
            _ => Err(Diagnostic::error(
                format!("declaration for {} is not a function", function_name),
                span,
            )),
        }
    }

    pub(super) fn allocate_dynamic_raw_object(
        &mut self,
        objects: &mut ObjectFrames,
        size: usize,
        span: Span,
        zeroed: bool,
    ) -> Result<ObjectId, Diagnostic> {
        self.next_encoded_pointer =
            align_address(self.next_encoded_pointer, HOST_LONG_DOUBLE_ALIGN as u64);
        let object_ty = CType::array_of(CType::UnsignedChar, size);
        let object = self.allocate_object(
            objects,
            object_ty.clone(),
            StorageDuration::Dynamic,
            span,
            false,
            false,
        )?;
        let value = if zeroed {
            StoredValue::ObjectRepresentation(vec![ByteCell::Known(0); size])
        } else {
            StoredValue::Indeterminate
        };
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.byte_size = size;
            state.value = value;
            state.initialized = zeroed;
            state.address_taken = true;
            state.raw_indeterminate_bytes =
                Some((state.modification_count, usize::from(!zeroed) * size));
        }
        self.dynamic_allocations.insert(object);
        self.dynamic_bytes_allocated = self
            .dynamic_bytes_allocated
            .checked_add(size)
            .expect("validated dynamic allocation total");
        Ok(object)
    }

    pub(super) fn retire_dynamic_object(
        &mut self,
        object_id: ObjectId,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let removed = objects.remove(object_id);
        if let Some(mut removed) = removed {
            self.dynamic_bytes_allocated = self
                .dynamic_bytes_allocated
                .checked_sub(removed.byte_size)
                .expect("retired allocation was included in the dynamic byte total");
            self.note_stream_buffer_lifetime_end(&[object_id]);
            removed.alive = false;
            removed.value = StoredValue::Indeterminate;
            self.retired_objects.insert(object_id, removed);
            self.forget_retired_restrict_sources(&[object_id]);
            return Ok(());
        }
        Err(Diagnostic::error(
            "dynamic allocation object was not present in storage",
            Span::new(FileId(0), 0, 0),
        ))
    }

    fn byte_ranges_overlap(
        lhs_start: usize,
        lhs_size: usize,
        rhs_start: usize,
        rhs_size: usize,
    ) -> bool {
        lhs_size != 0
            && rhs_size != 0
            && lhs_start < rhs_start.saturating_add(rhs_size)
            && rhs_start < lhs_start.saturating_add(lhs_size)
    }

    pub(super) fn replace_pointer_slots_for_write(
        object: &mut ObjectState,
        start: usize,
        size: usize,
        replacement: Option<StoredPointerValue>,
    ) {
        let write_end = start.saturating_add(size);
        let first_possible_start = start.saturating_sub(HOST_POINTER_SIZE.saturating_sub(1));
        let overlapping = object
            .pointer_slots
            .range((first_possible_start, 0)..(write_end, 0))
            .filter_map(|(&slot, _)| {
                Self::byte_ranges_overlap(start, size, slot.0, slot.1).then_some(slot)
            })
            .collect::<Vec<_>>();
        for slot in overlapping {
            object.pointer_slots.remove(&slot);
        }
        if let Some(replacement) = replacement {
            object.pointer_slots.insert((start, size), replacement);
        }
    }

    pub(super) fn collect_stored_pointer_slots(
        &self,
        stored: &StoredValue,
        ty: &CType,
        start: usize,
        slots: &mut Vec<((usize, usize), StoredPointerValue)>,
    ) {
        match (stored, ty.unqualified()) {
            (StoredValue::Scalar(value), CType::Pointer(_)) => {
                let pointer = match &value.data {
                    ValueData::Pointer(pointer) => {
                        Some(StoredPointerValue::Object(pointer.as_ref().clone()))
                    }
                    ValueData::Function(name) => {
                        Some(StoredPointerValue::Function(name.to_string()))
                    }
                    _ => None,
                };
                if let (Some(pointer), Some(size)) = (pointer, self.type_size_of(ty)) {
                    slots.push(((start, size), pointer));
                }
            }
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return;
                };
                for (index, value) in values.iter().enumerate() {
                    self.collect_stored_pointer_slots(
                        value,
                        inner,
                        start.saturating_add(index.saturating_mul(stride)),
                        slots,
                    );
                }
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let Some(record) = self.record_type(ty) else {
                    return;
                };
                for member in &record.members {
                    if member.bit_width.is_some() {
                        continue;
                    }
                    if let Some((_, value)) = values
                        .iter()
                        .find(|(name, _)| name.as_ref() == member.storage_name)
                    {
                        self.collect_stored_pointer_slots(
                            value,
                            &member.ty,
                            start.saturating_add(member.offset),
                            slots,
                        );
                    }
                }
            }
            (
                StoredValue::Union {
                    active_member: Some(active_member),
                    members,
                    ..
                },
                CType::Union(_, _),
            ) => {
                let Some(record) = self.record_type(ty) else {
                    return;
                };
                let Some(member) = record
                    .members
                    .iter()
                    .find(|member| member.storage_name.as_str() == active_member.as_ref())
                else {
                    return;
                };
                if member.bit_width.is_some() {
                    return;
                }
                if let Some((_, value)) = members
                    .iter()
                    .find(|(name, _)| name.as_ref() == active_member.as_ref())
                {
                    self.collect_stored_pointer_slots(
                        value,
                        &member.ty,
                        start.saturating_add(member.offset),
                        slots,
                    );
                }
            }
            _ => {}
        }
    }

    pub(super) fn copied_pointer_slots(
        source: &ObjectState,
        source_start: usize,
        destination_start: usize,
        size: usize,
    ) -> Vec<((usize, usize), StoredPointerValue)> {
        let source_end = source_start.saturating_add(size);
        source
            .pointer_slots
            .range((source_start, 0)..(source_end, 0))
            .filter_map(|(&(slot_start, slot_size), value)| {
                let slot_end = slot_start.checked_add(slot_size)?;
                if slot_start < source_start || slot_end > source_end {
                    return None;
                }
                let destination_slot = destination_start.checked_add(slot_start - source_start)?;
                Some(((destination_slot, slot_size), value.clone()))
            })
            .collect()
    }

    fn overlay_raw_object_representation(
        object: &mut ObjectState,
        start: usize,
        byte_count: usize,
        bytes: impl IntoIterator<Item = ByteCell>,
    ) -> bool {
        let StoredValue::ObjectRepresentation(all_bytes) = &mut object.value else {
            return false;
        };
        let current_version = object.modification_count;
        let mut indeterminate_bytes = object
            .raw_indeterminate_bytes
            .filter(|(version, _)| *version == current_version)
            .map(|(_, count)| count)
            .unwrap_or_else(|| {
                all_bytes
                    .iter()
                    .filter(|byte| matches!(byte, ByteCell::Indeterminate))
                    .count()
            });
        for (index, source) in bytes.into_iter().enumerate() {
            if let Some(destination) = all_bytes.get_mut(start + index) {
                match (*destination, source) {
                    (ByteCell::Indeterminate, ByteCell::Known(_)) => {
                        indeterminate_bytes = indeterminate_bytes.saturating_sub(1);
                    }
                    (ByteCell::Known(_), ByteCell::Indeterminate) => {
                        indeterminate_bytes = indeterminate_bytes.saturating_add(1);
                    }
                    _ => {}
                }
                *destination = source;
            }
        }
        object.initialized = indeterminate_bytes == 0;
        object.indeterminate_reason = None;
        object.modification_count = object.modification_count.saturating_add(1);
        object.raw_indeterminate_bytes = Some((object.modification_count, indeterminate_bytes));
        Self::replace_pointer_slots_for_write(object, start, byte_count, None);
        true
    }

    fn overlay_serialized_object(
        &mut self,
        object_id: ObjectId,
        start: usize,
        bytes: &[ByteCell],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let (object_ty, object_value) = self
            .lookup_object(objects, object_id)
            .map(|object| (object.ty.clone(), object.value.clone()))
            .ok_or_else(|| Self::ended_object_write_diagnostic(span))?;
        let mut all_bytes = self.serialize_stored_value(&object_ty, &object_value, span)?;
        self.overlay_bytes(&mut all_bytes, start, bytes);
        let indeterminate_bytes = all_bytes
            .iter()
            .filter(|byte| matches!(byte, ByteCell::Indeterminate))
            .count();
        let object = self
            .lookup_object_mut(objects, object_id)
            .ok_or_else(|| Self::ended_object_write_diagnostic(span))?;
        object.value = StoredValue::ObjectRepresentation(all_bytes);
        object.initialized = indeterminate_bytes == 0;
        object.indeterminate_reason = None;
        object.modification_count = object.modification_count.saturating_add(1);
        object.raw_indeterminate_bytes = Some((object.modification_count, indeterminate_bytes));
        Self::replace_pointer_slots_for_write(object, start, bytes.len(), None);
        Ok(())
    }

    fn ended_object_write_diagnostic(span: Span) -> Diagnostic {
        Diagnostic::ub(
            "write through a pointer to an object whose lifetime has ended",
            span,
            Some("6.2.4"),
        )
    }

    pub(super) fn overlay_known_bytes_into_object(
        &mut self,
        object_id: ObjectId,
        start: usize,
        bytes: &[u8],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let object = self
            .lookup_object_mut(objects, object_id)
            .ok_or_else(|| Self::ended_object_write_diagnostic(span))?;
        if Self::overlay_raw_object_representation(
            object,
            start,
            bytes.len(),
            bytes.iter().copied().map(ByteCell::Known),
        ) {
            return Ok(());
        }
        let overlay = bytes
            .iter()
            .copied()
            .map(ByteCell::Known)
            .collect::<Vec<_>>();
        self.overlay_serialized_object(object_id, start, &overlay, span, objects)
    }

    pub(super) fn overlay_byte_cells_into_object(
        &mut self,
        object_id: ObjectId,
        start: usize,
        bytes: &[ByteCell],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let object = self
            .lookup_object_mut(objects, object_id)
            .ok_or_else(|| Self::ended_object_write_diagnostic(span))?;
        if Self::overlay_raw_object_representation(
            object,
            start,
            bytes.len(),
            bytes.iter().copied(),
        ) {
            return Ok(());
        }
        self.overlay_serialized_object(object_id, start, bytes, span, objects)
    }

    pub(super) fn read_multibyte_source_buffer(
        &mut self,
        pointer: &PointerValue,
        limit: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<u8>, Diagnostic> {
        if limit == 0 {
            let _ = self.library_array_region(pointer, 0, 1, span, objects)?;
            return Ok(vec![0]);
        }
        let mut bytes = self.read_bounded_c_string_source(pointer.clone(), limit, span, objects)?;
        if bytes.last().copied() != Some(0) {
            bytes.push(0);
        }
        Ok(bytes)
    }

    pub(super) fn wide_char_byte_width(&self) -> usize {
        std::mem::size_of::<libc::wchar_t>()
    }

    pub(super) fn wchar_scalar_to_host(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &str,
    ) -> Result<libc::wchar_t, Diagnostic> {
        if value.ty != self.wchar_type() {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wchar_t argument"),
                span,
            ));
        }
        libc::wchar_t::try_from(value.to_int()?).map_err(|_| {
            Diagnostic::ub(
                format!("{function_name} argument cannot be represented as wchar_t"),
                span,
                Some("7.20.8"),
            )
        })
    }

    pub(super) fn wide_units_to_byte_cells(
        &mut self,
        units: &[libc::wchar_t],
        span: Span,
    ) -> Result<Vec<ByteCell>, Diagnostic> {
        let mut bytes = Vec::with_capacity(units.len() * self.wide_char_byte_width());
        for &unit in units {
            let typed = TypedValue::integer(self.wchar_type(), unit as i128);
            bytes.extend(self.serialize_scalar_bytes(&self.wchar_type(), &typed, span)?);
        }
        Ok(bytes)
    }

    pub(super) fn read_wide_string_units(
        &mut self,
        pointer: PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        let start = self
            .pointer_byte_offset(&pointer, &self.wchar_type(), objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "wide string pointer does not designate a valid wchar_t region",
                    span,
                    Some("7.1.4"),
                )
            })?;
        let (_, _, all_bytes) = self.pointer_object_bytes(&pointer, span, objects)?;
        let stride = self.wide_char_byte_width();
        let mut units = Vec::new();
        let mut offset = start;
        while offset + stride <= all_bytes.len() {
            let stored = self.deserialize_stored_value(
                &self.wchar_type(),
                &all_bytes[offset..offset + stride],
                span,
            )?;
            let value = self.typed_value_from_stored(&self.wchar_type(), &stored);
            self.reject_indeterminate_library_value(&value, span, "wide string")?;
            let unit = libc::wchar_t::try_from(value.to_int()?).map_err(|_| {
                Diagnostic::ub(
                    "wide string contains an invalid wchar_t representation",
                    span,
                    Some("7.1.4"),
                )
            })?;
            if unit == 0 {
                let access_size = (units.len() + 1).checked_mul(stride).ok_or_else(|| {
                    Diagnostic::ub("wide string access is out of range", span, Some("7.1.4"))
                })?;
                let _ = self.library_array_region(&pointer, access_size, stride, span, objects)?;
                return Ok(units);
            }
            units.push(unit);
            offset += stride;
        }
        Err(Diagnostic::ub(
            "wide string is not terminated within the bounds of its object",
            span,
            Some("7.1.4"),
        ))
    }

    pub(super) fn read_bounded_wide_source(
        &mut self,
        pointer: PointerValue,
        limit: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        if limit == 0 {
            let _ =
                self.library_array_region(&pointer, 0, self.wide_char_byte_width(), span, objects)?;
            return Ok(Vec::new());
        }
        let start = self
            .pointer_byte_offset(&pointer, &self.wchar_type(), objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "wide string pointer does not designate a valid wchar_t region",
                    span,
                    Some("7.1.4"),
                )
            })?;
        let (_, _, all_bytes) = self.pointer_object_bytes(&pointer, span, objects)?;
        let stride = self.wide_char_byte_width();
        let available_units = all_bytes.len().saturating_sub(start) / stride;
        let mut units = Vec::with_capacity(limit.min(available_units));
        let mut offset = start;
        for _ in 0..limit.min(available_units) {
            let stored = self.deserialize_stored_value(
                &self.wchar_type(),
                &all_bytes[offset..offset + stride],
                span,
            )?;
            let value = self.typed_value_from_stored(&self.wchar_type(), &stored);
            self.reject_indeterminate_library_value(&value, span, "wide string")?;
            let unit = libc::wchar_t::try_from(value.to_int()?).map_err(|_| {
                Diagnostic::ub(
                    "wide string contains an invalid wchar_t representation",
                    span,
                    Some("7.1.4"),
                )
            })?;
            units.push(unit);
            offset += stride;
            if unit == 0 {
                let access_size = units.len().checked_mul(stride).ok_or_else(|| {
                    Diagnostic::ub("wide string access is out of range", span, Some("7.1.4"))
                })?;
                let _ = self.library_array_region(&pointer, access_size, stride, span, objects)?;
                return Ok(units);
            }
        }
        if available_units < limit {
            return Err(Diagnostic::ub(
                format!(
                    "requested wide-character access of {} element(s) exceeds the object bounds",
                    limit
                ),
                span,
                Some("7.1.4"),
            ));
        }
        let access_size = limit.checked_mul(stride).ok_or_else(|| {
            Diagnostic::ub("wide string access is out of range", span, Some("7.1.4"))
        })?;
        let _ = self.library_array_region(&pointer, access_size, stride, span, objects)?;
        Ok(units)
    }

    pub(super) fn read_exact_wide_units(
        &mut self,
        pointer: PointerValue,
        count: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        if count == 0 {
            let _ =
                self.library_array_region(&pointer, 0, self.wide_char_byte_width(), span, objects)?;
            return Ok(Vec::new());
        }
        let stride = self.wide_char_byte_width();
        let total = count.checked_mul(stride).ok_or_else(|| {
            Diagnostic::ub(
                "requested wide-character access is out of supported range",
                span,
                Some("7.1.4"),
            )
        })?;
        let _ = self.library_array_region(&pointer, total, stride, span, objects)?;
        let (_, start, all_bytes) = self.pointer_object_bytes(&pointer, span, objects)?;
        let end = start.checked_add(total).ok_or_else(|| {
            Diagnostic::ub(
                "requested wide-character access is out of supported range",
                span,
                Some("7.1.4"),
            )
        })?;
        if end > all_bytes.len() {
            return Err(Diagnostic::ub(
                format!(
                    "requested wide-character access of {} element(s) exceeds the object bounds",
                    count
                ),
                span,
                Some("7.1.4"),
            ));
        }
        let mut units = Vec::with_capacity(count);
        let mut offset = start;
        for _ in 0..count {
            let stored = self.deserialize_stored_value(
                &self.wchar_type(),
                &all_bytes[offset..offset + stride],
                span,
            )?;
            let value = self.typed_value_from_stored(&self.wchar_type(), &stored);
            self.reject_indeterminate_library_value(&value, span, "wide string")?;
            let unit = libc::wchar_t::try_from(value.to_int()?).map_err(|_| {
                Diagnostic::ub(
                    "wide string contains an invalid wchar_t representation",
                    span,
                    Some("7.1.4"),
                )
            })?;
            units.push(unit);
            offset += stride;
        }
        Ok(units)
    }

    pub(super) fn write_wide_units_to_pointer(
        &mut self,
        pointer: &PointerValue,
        units: &[libc::wchar_t],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let bytes = self.wide_units_to_byte_cells(units, span)?;
        let size = bytes.len();
        if size == 0 {
            return Ok(());
        }
        let (object_id, start, _) = self.byte_region_from_pointer(pointer, size, span, objects)?;
        self.ensure_library_writable_region(object_id, start, size, span, objects)?;
        self.overlay_byte_cells_into_object(object_id, start, &bytes, span, objects)
    }

    fn host_mbstate_size(&self) -> usize {
        std::mem::size_of::<HostMbState>()
    }

    pub(super) fn check_mbstate_for_current_locale(
        &mut self,
        function_name: &str,
        pointer: &PointerValue,
        writable: bool,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if pointer.is_null() {
            if self
                .internal_mbstate_generations
                .get(function_name)
                .is_some_and(|&generation| generation != self.locale_generation)
            {
                return Err(Diagnostic::ub(
                    format!(
                        "{function_name} uses an internal conversion state retained across an LC_CTYPE locale change"
                    ),
                    span,
                    Some("7.29.6"),
                ));
            }
            self.internal_mbstate_generations
                .insert(function_name.to_owned(), self.locale_generation);
            return Ok(());
        }

        let size = self.host_mbstate_size();
        let (object_id, start, _) =
            self.library_array_region(pointer, size, size, span, objects)?;
        if writable {
            self.ensure_library_writable_region(object_id, start, size, span, objects)?;
        }
        let version = self
            .lookup_object(objects, object_id)
            .expect("validated mbstate_t object")
            .modification_count;
        match self.mbstate_provenance.get(&(object_id, start)).copied() {
            Some((saved_version, generation))
                if saved_version == version && generation == self.locale_generation => {}
            Some((saved_version, _)) if saved_version == version => {
                return Err(Diagnostic::ub(
                    format!(
                        "{function_name} uses a conversion state retained across an LC_CTYPE locale change"
                    ),
                    span,
                    Some("7.29.6"),
                ));
            }
            _ => {
                let bytes = self.read_pointer_bytes(pointer, size, span, objects)?;
                if bytes.iter().any(|&byte| byte != 0) {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} uses an mbstate_t value that is neither initial nor produced by a tracked conversion"
                        ),
                        span,
                        Some("7.29.6"),
                    ));
                }
            }
        }
        self.mbstate_provenance
            .insert((object_id, start), (version, self.locale_generation));
        Ok(())
    }

    pub(super) fn read_mbstate_value(
        &mut self,
        pointer: &PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<HostMbState, Diagnostic> {
        let bytes = self.read_pointer_bytes(pointer, self.host_mbstate_size(), span, objects)?;
        let mut state = HostMbState { opaque: [0; 128] };
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (&mut state as *mut HostMbState).cast::<u8>(),
                bytes.len(),
            );
        }
        Ok(state)
    }

    pub(super) fn write_mbstate_value(
        &mut self,
        pointer: &PointerValue,
        state: &HostMbState,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (state as *const HostMbState).cast::<u8>(),
                self.host_mbstate_size(),
            )
        };
        let (object_id, start, _) =
            self.byte_region_from_pointer(pointer, self.host_mbstate_size(), span, objects)?;
        self.ensure_writable_object(object_id, span, objects)?;
        self.overlay_known_bytes_into_object(object_id, start, bytes, span, objects)?;
        let version = self
            .lookup_object(objects, object_id)
            .expect("mbstate_t output object exists")
            .modification_count;
        self.mbstate_provenance
            .insert((object_id, start), (version, self.locale_generation));
        Ok(())
    }

    pub(super) fn comparator_function_symbol(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<String, Diagnostic> {
        let (return_ty, params, is_variadic) = self.call_target_signature(&value.ty, span)?;
        if is_variadic
            || *return_ty != CType::Int
            || params.len() != 2
            || params[0] != self.const_void_ptr_type()
            || params[1] != self.const_void_ptr_type()
        {
            return Err(Diagnostic::error(
                format!(
                    "{function_name} requires a comparison function of type int (*)(const void *, const void *)"
                ),
                span,
            ));
        }
        match &value.data {
            ValueData::Function(name) => Ok(name.to_string()),
            ValueData::Pointer(pointer) if pointer.is_null() => Err(Diagnostic::ub(
                format!("{function_name} comparison function pointer is null"),
                span,
                Some("7.20.5"),
            )),
            _ => Err(Diagnostic::error(
                format!("{function_name} requires a callable comparison function"),
                span,
            )),
        }
    }

    fn store_printf_n_count(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        value: &TypedValue,
        span: Span,
        count: usize,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let pointee = self.printf_n_pointee_type(conversion.length, span)?;
        let pointer = value.as_pointer(span)?;
        let stored = self.ensure_integer_ub_range(count as i128, &pointee, span, standard)?;
        let lvalue = self.pointer_lvalue(function_name, &pointer, &pointee, span, standard)?;
        self.record_formatted_io_access(
            &pointer,
            &pointee,
            1,
            FormattedIoAccessRole::PercentN,
            span,
            objects,
        )?;
        self.store_lvalue(objects, &lvalue, TypedValue::integer(pointee, stored), span)
    }

    fn normalized_printf_width(&self, width: Option<i32>, flags: &str) -> (usize, bool) {
        let left_adjust = flags.contains('-') || width.is_some_and(|value| value < 0);
        let width = width
            .map(|value| value.unsigned_abs() as usize)
            .unwrap_or(0);
        (width, left_adjust)
    }

    fn apply_printf_field_width_bytes(
        &self,
        bytes: Vec<u8>,
        width: Option<i32>,
        flags: &str,
        pad_byte: u8,
    ) -> Vec<u8> {
        let (field_width, left_adjust) = self.normalized_printf_width(width, flags);
        if bytes.len() >= field_width {
            return bytes;
        }
        let mut padding = vec![pad_byte; field_width - bytes.len()];
        if left_adjust {
            let mut out = bytes;
            out.append(&mut padding);
            out
        } else {
            let mut out = padding;
            out.extend(bytes);
            out
        }
    }

    fn wchar_to_multibyte_bytes(
        &self,
        wc: libc::wchar_t,
        span: Span,
        standard: &'static str,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut state = HostMbState { opaque: [0; 128] };
        let mut bytes = vec![0u8; host_mb_cur_max().max(1)];
        let written = unsafe { wcrtomb(bytes.as_mut_ptr().cast::<c_char>(), wc, &mut state) };
        if written == usize::MAX {
            return Err(Diagnostic::ub(
                "wide character cannot be converted to a multibyte sequence",
                span,
                Some(standard),
            ));
        }
        bytes.truncate(written);
        Ok(bytes)
    }

    fn wide_units_to_multibyte_bytes(
        &self,
        units: &[libc::wchar_t],
        precision: Option<usize>,
        span: Span,
        standard: &'static str,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut out = Vec::new();
        for &unit in units {
            if unit == 0 {
                break;
            }
            let bytes = self.wchar_to_multibyte_bytes(unit, span, standard)?;
            if let Some(limit) = precision
                && out.len().saturating_add(bytes.len()) > limit
            {
                break;
            }
            out.extend(bytes);
        }
        Ok(out)
    }

    fn render_printf_fragment_bytes(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        width: Option<i32>,
        precision: Option<i32>,
        value: Option<PrintfArgRef<'_>>,
        count_so_far: usize,
        format_span: Span,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<Vec<u8>, Diagnostic> {
        self.validate_printf_conversion_arg(
            function_name,
            conversion,
            precision,
            value,
            format_span,
            standard,
            objects,
        )?;
        let fmt = self.host_printf_conversion_text(conversion);
        let width_from_arg = matches!(conversion.width, Some(PrintfCount::FromArg));
        let precision_from_arg = matches!(conversion.precision, Some(PrintfCount::FromArg));
        macro_rules! snprintf_host_no_arg {
            () => {{
                let format = CString::new(fmt.as_str()).unwrap();
                self.snprintf_bytes_with_call(|buffer, len| unsafe {
                    match (width_from_arg, precision_from_arg) {
                        (false, false) => libc::snprintf(buffer, len, format.as_ptr()),
                        (true, false) => {
                            libc::snprintf(buffer, len, format.as_ptr(), width.unwrap_or(0))
                        }
                        (false, true) => {
                            libc::snprintf(buffer, len, format.as_ptr(), precision.unwrap_or(-1))
                        }
                        (true, true) => libc::snprintf(
                            buffer,
                            len,
                            format.as_ptr(),
                            width.unwrap_or(0),
                            precision.unwrap_or(-1),
                        ),
                    }
                })
            }};
        }
        macro_rules! snprintf_host {
            ($value:expr) => {{
                let format = CString::new(fmt.as_str()).unwrap();
                self.snprintf_bytes_with_call(|buffer, len| unsafe {
                    match (width_from_arg, precision_from_arg) {
                        (false, false) => libc::snprintf(buffer, len, format.as_ptr(), $value),
                        (true, false) => {
                            libc::snprintf(buffer, len, format.as_ptr(), width.unwrap_or(0), $value)
                        }
                        (false, true) => libc::snprintf(
                            buffer,
                            len,
                            format.as_ptr(),
                            precision.unwrap_or(-1),
                            $value,
                        ),
                        (true, true) => libc::snprintf(
                            buffer,
                            len,
                            format.as_ptr(),
                            width.unwrap_or(0),
                            precision.unwrap_or(-1),
                            $value,
                        ),
                    }
                })
            }};
        }
        let effective_precision = precision.and_then(|value| usize::try_from(value).ok());
        match conversion.spec {
            '%' => snprintf_host_no_arg!(),
            'd' | 'i' | 'o' | 'u' | 'x' | 'X' => {
                let arg = value.expect("checked by caller").value;
                let signed = matches!(conversion.spec, 'd' | 'i');
                match (conversion.length, signed) {
                    (PrintfLength::None | PrintfLength::H | PrintfLength::Hh, true) => {
                        let value = arg.to_int()? as c_int;
                        snprintf_host!(value)
                    }
                    (PrintfLength::None | PrintfLength::H | PrintfLength::Hh, false) => {
                        let value = arg.to_int()? as u32;
                        snprintf_host!(value)
                    }
                    (
                        PrintfLength::L | PrintfLength::J | PrintfLength::T | PrintfLength::Z,
                        true,
                    ) => {
                        let value = arg.to_int()? as c_longlong;
                        snprintf_host!(value)
                    }
                    (
                        PrintfLength::L | PrintfLength::J | PrintfLength::T | PrintfLength::Z,
                        false,
                    ) => {
                        let value = arg.to_int()? as c_ulonglong;
                        snprintf_host!(value)
                    }
                    (PrintfLength::Ll, true) => {
                        let value = arg.to_int()? as c_longlong;
                        snprintf_host!(value)
                    }
                    (PrintfLength::Ll, false) => {
                        let value = arg.to_int()? as c_ulonglong;
                        snprintf_host!(value)
                    }
                    _ => Err(Diagnostic::error(
                        format!("unsupported {function_name} conversion {}", fmt),
                        format_span,
                    )),
                }
            }
            'c' => {
                let arg = value.expect("checked by caller");
                match conversion.length {
                    PrintfLength::None => Ok(self.apply_printf_field_width_bytes(
                        vec![arg.value.to_int()? as u8],
                        width,
                        &conversion.flags,
                        b' ',
                    )),
                    PrintfLength::L => {
                        let bytes = self.wchar_to_multibyte_bytes(
                            arg.value.to_int()? as libc::wchar_t,
                            arg.span,
                            standard,
                        )?;
                        Ok(self.apply_printf_field_width_bytes(
                            bytes,
                            width,
                            &conversion.flags,
                            b' ',
                        ))
                    }
                    _ => unreachable!("validated above"),
                }
            }
            's' => {
                let arg = value.expect("checked by caller");
                match conversion.length {
                    PrintfLength::None => {
                        let pointer = arg.value.as_pointer(arg.span)?;
                        let (bytes, accessed) = if let Some(limit) = effective_precision {
                            let mut bounded = self
                                .read_multibyte_source_buffer(&pointer, limit, arg.span, objects)?;
                            let accessed = bounded
                                .iter()
                                .take(limit)
                                .position(|byte| *byte == 0)
                                .map_or(limit, |index| index + 1);
                            if let Some(nul) = bounded.iter().position(|byte| *byte == 0) {
                                bounded.truncate(nul);
                            }
                            (bounded, accessed)
                        } else {
                            let bytes =
                                self.read_c_string_bytes(pointer.clone(), arg.span, objects)?;
                            let accessed = bytes.len() + 1;
                            (bytes, accessed)
                        };
                        self.record_formatted_io_access(
                            &pointer,
                            &CType::UnsignedChar,
                            accessed,
                            FormattedIoAccessRole::StringArgument,
                            arg.span,
                            objects,
                        )?;
                        Ok(self.apply_printf_field_width_bytes(
                            bytes,
                            width,
                            &conversion.flags,
                            b' ',
                        ))
                    }
                    PrintfLength::L => {
                        let pointer = arg.value.as_pointer(arg.span)?;
                        let (units, accessed) = if let Some(limit) = effective_precision {
                            let units = self.read_bounded_wide_source(
                                pointer.clone(),
                                limit,
                                arg.span,
                                objects,
                            )?;
                            let accessed = units.len();
                            (units, accessed)
                        } else {
                            let units =
                                self.read_wide_string_units(pointer.clone(), arg.span, objects)?;
                            let accessed = units.len() + 1;
                            (units, accessed)
                        };
                        self.record_formatted_io_access(
                            &pointer,
                            &self.wchar_type(),
                            accessed,
                            FormattedIoAccessRole::StringArgument,
                            arg.span,
                            objects,
                        )?;
                        let bytes = self.wide_units_to_multibyte_bytes(
                            &units,
                            effective_precision,
                            arg.span,
                            standard,
                        )?;
                        Ok(self.apply_printf_field_width_bytes(
                            bytes,
                            width,
                            &conversion.flags,
                            b' ',
                        ))
                    }
                    _ => unreachable!("validated above"),
                }
            }
            'p' => {
                let arg = value.expect("checked by caller").value;
                let address = self.pointer_numeric_address(arg, &arg.ty, format_span)?;
                snprintf_host!(self.synthetic_void_pointer(address))
            }
            'n' => {
                let arg = value.expect("checked by caller");
                self.store_printf_n_count(
                    function_name,
                    conversion,
                    arg.value,
                    arg.span,
                    count_so_far,
                    objects,
                    standard,
                )?;
                Ok(Vec::new())
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A' => {
                let arg = value.expect("checked by caller").value;
                match conversion.length {
                    PrintfLength::BigL => {
                        let value = arg.to_float()? as HostLongDouble;
                        snprintf_host!(value)
                    }
                    PrintfLength::None | PrintfLength::L => {
                        let value = arg.to_float()?;
                        snprintf_host!(value)
                    }
                    _ => unreachable!("validated above"),
                }
            }
            _ => Err(Diagnostic::error(
                format!("unsupported {function_name} conversion {}", fmt),
                format_span,
            )),
        }
    }

    fn collect_printf_bytes(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<Vec<u8>, Diagnostic> {
        let format_span = args[format_index].span();
        let format_pointer = evaluated[format_index].as_pointer(format_span)?;
        let format = self.read_c_string_bytes(format_pointer.clone(), format_span, objects)?;
        self.record_formatted_io_access(
            &format_pointer,
            &CType::UnsignedChar,
            format.len() + 1,
            FormattedIoAccessRole::Format,
            format_span,
            objects,
        )?;
        let variadic_args = args
            .iter()
            .zip(evaluated.iter())
            .skip(format_index + 1)
            .map(|(expr, value)| PrintfArgRef {
                value,
                span: expr.span(),
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != b'%' {
                out.push(format[index]);
                index += 1;
                continue;
            }
            index += 1;
            let conversion = self.parse_printf_conversion_bytes(
                function_name,
                &format,
                &mut index,
                format_span,
            )?;
            let ResolvedPrintfArgs {
                width,
                precision,
                value,
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                "7.19.6.1",
            )?;
            let fragment = self.render_printf_fragment_bytes(
                function_name,
                &conversion,
                width,
                precision,
                value,
                out.len(),
                format_span,
                objects,
                "7.19.6.1",
            )?;
            out.extend(fragment);
        }
        let _ = self.ensure_int_range(
            out.len() as i128,
            span,
            "formatted output count is outside the supported int range",
        )?;
        Ok(out)
    }

    fn collect_printf_bytes_from_va_list(
        &mut self,
        function_name: &'static str,
        format_pointer: PointerValue,
        format_span: Span,
        values: &[TypedValue],
        value_span: Span,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(Vec<u8>, usize), Diagnostic> {
        let format = self.read_c_string_bytes(format_pointer.clone(), format_span, objects)?;
        self.record_formatted_io_access(
            &format_pointer,
            &CType::UnsignedChar,
            format.len() + 1,
            FormattedIoAccessRole::Format,
            format_span,
            objects,
        )?;
        let variadic_args = values
            .iter()
            .map(|value| PrintfArgRef {
                value,
                span: value_span,
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != b'%' {
                out.push(format[index]);
                index += 1;
                continue;
            }
            index += 1;
            let conversion = self.parse_printf_conversion_bytes(
                function_name,
                &format,
                &mut index,
                format_span,
            )?;
            let ResolvedPrintfArgs {
                width,
                precision,
                value,
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                "7.19.6.8",
            )?;
            let fragment = self.render_printf_fragment_bytes(
                function_name,
                &conversion,
                width,
                precision,
                value,
                out.len(),
                format_span,
                objects,
                "7.19.6.8",
            )?;
            out.extend(fragment);
        }
        Ok((out, arg_index))
    }

    fn render_printf_fragment_units(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        width: Option<i32>,
        precision: Option<i32>,
        value: Option<PrintfArgRef<'_>>,
        count_so_far: usize,
        format_span: Span,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        if conversion.spec == 'n' {
            self.validate_printf_conversion_arg(
                function_name,
                conversion,
                precision,
                value,
                format_span,
                standard,
                objects,
            )?;
            let arg = value.expect("checked by caller");
            self.store_printf_n_count(
                function_name,
                conversion,
                arg.value,
                arg.span,
                count_so_far,
                objects,
                standard,
            )?;
            return Ok(Vec::new());
        }
        let bytes = self.render_printf_fragment_bytes(
            function_name,
            conversion,
            width,
            precision,
            value,
            count_so_far,
            format_span,
            objects,
            standard,
        )?;
        self.multibyte_bytes_to_wide_units(&bytes, format_span)
    }

    fn collect_wprintf_units(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        let format_span = args[format_index].span();
        let format_pointer = evaluated[format_index].as_pointer(format_span)?;
        let format = self.read_wide_string_units(format_pointer.clone(), format_span, objects)?;
        self.record_formatted_io_access(
            &format_pointer,
            &self.wchar_type(),
            format.len() + 1,
            FormattedIoAccessRole::Format,
            format_span,
            objects,
        )?;
        let variadic_args = args
            .iter()
            .zip(evaluated.iter())
            .skip(format_index + 1)
            .map(|(expr, value)| PrintfArgRef {
                value,
                span: expr.span(),
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != '%' as libc::wchar_t {
                out.push(format[index]);
                index += 1;
                continue;
            }
            index += 1;
            let conversion =
                self.parse_printf_conversion_wide(function_name, &format, &mut index, format_span)?;
            let ResolvedPrintfArgs {
                width,
                precision,
                value,
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                "7.24.2.1",
            )?;
            let fragment = self.render_printf_fragment_units(
                function_name,
                &conversion,
                width,
                precision,
                value,
                out.len(),
                format_span,
                objects,
                "7.24.2.1",
            )?;
            out.extend(fragment);
        }
        let _ = self.ensure_int_range(
            out.len() as i128,
            span,
            "wide formatted output count is outside the supported int range",
        )?;
        Ok(out)
    }

    fn collect_wprintf_units_from_va_list(
        &mut self,
        function_name: &'static str,
        format_pointer: PointerValue,
        format_span: Span,
        values: &[TypedValue],
        value_span: Span,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(Vec<libc::wchar_t>, usize), Diagnostic> {
        let format = self.read_wide_string_units(format_pointer.clone(), format_span, objects)?;
        self.record_formatted_io_access(
            &format_pointer,
            &self.wchar_type(),
            format.len() + 1,
            FormattedIoAccessRole::Format,
            format_span,
            objects,
        )?;
        let variadic_args = values
            .iter()
            .map(|value| PrintfArgRef {
                value,
                span: value_span,
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != '%' as libc::wchar_t {
                out.push(format[index]);
                index += 1;
                continue;
            }
            index += 1;
            let conversion =
                self.parse_printf_conversion_wide(function_name, &format, &mut index, format_span)?;
            let ResolvedPrintfArgs {
                width,
                precision,
                value,
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                "7.24.2.5",
            )?;
            let fragment = self.render_printf_fragment_units(
                function_name,
                &conversion,
                width,
                precision,
                value,
                out.len(),
                format_span,
                objects,
                "7.24.2.5",
            )?;
            out.extend(fragment);
        }
        Ok((out, arg_index))
    }

    pub(super) fn eval_printf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out = self.collect_printf_bytes("printf", 0, args, evaluated, span, objects)?;
        let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
        let written = self.write_known_bytes_to_stream(stream, &out, span, "printf", "7.19.6.1")?;
        let count = self.ensure_int_range(
            written as i128,
            span,
            "printf output count is outside the supported int range",
        )?;
        Ok(TypedValue::int(count))
    }

    pub(super) fn eval_fprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out = self.collect_printf_bytes("fprintf", 1, args, evaluated, span, objects)?;
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fprintf", "7.19.6.1")?
            .expect("checked by caller");
        let written =
            self.write_known_bytes_to_stream(stream_object, &out, span, "fprintf", "7.19.6.1")?;
        let count = self.ensure_int_range(
            written as i128,
            span,
            "fprintf output count is outside the supported int range",
        )?;
        Ok(TypedValue::int(count))
    }

    pub(super) fn eval_sprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let (out, accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_printf_bytes("sprintf", 1, args, evaluated, span, objects)
        })?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        self.reject_formatted_io_destination_overlap(
            "sprintf",
            &dest_pointer,
            out.len() + 1,
            args[0].span(),
            &accesses,
            objects,
            "7.19.6.6",
        )?;
        self.write_c_string_to_pointer(&dest_pointer, &out, args[0].span(), objects)?;
        let count = self.ensure_int_range(
            out.len() as i128,
            span,
            "sprintf output count is outside the supported int range",
        )?;
        Ok(TypedValue::int(count))
    }

    pub(super) fn eval_snprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let (out, accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_printf_bytes("snprintf", 2, args, evaluated, span, objects)
        })?;
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "snprintf size")?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let copy_len = out.len().min(bound.saturating_sub(1));
        self.reject_formatted_io_destination_overlap(
            "snprintf",
            &dest_pointer,
            usize::from(bound != 0).saturating_add(copy_len),
            args[0].span(),
            &accesses,
            objects,
            "7.19.6.5",
        )?;
        if bound != 0 {
            if dest_pointer.is_null() {
                return Err(Diagnostic::ub(
                    "snprintf requires a valid destination pointer when n is nonzero",
                    args[0].span(),
                    Some("7.19.6.5"),
                ));
            }
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest_pointer, bound, args[0].span(), objects)?;
            self.ensure_library_writable_region(
                object_id,
                start,
                copy_len + 1,
                args[0].span(),
                objects,
            )?;
            let mut rendered = out[..copy_len].to_vec();
            rendered.push(0);
            self.overlay_known_bytes_into_object(
                object_id,
                start,
                &rendered,
                args[0].span(),
                objects,
            )?;
        }
        let count = self.ensure_int_range(
            out.len() as i128,
            span,
            "snprintf output count is outside the supported int range",
        )?;
        Ok(TypedValue::int(count))
    }

    pub(super) fn eval_vprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[1].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vprintf used with an invalid va_list",
                    args[1].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let (out, used) = self.collect_printf_bytes_from_va_list(
            "vprintf",
            format_pointer,
            args[0].span(),
            &values.args[values.index..],
            args[1].span(),
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
        let written =
            self.write_known_bytes_to_stream(stream, &out, span, "vprintf", "7.19.6.8")?;
        Ok(TypedValue::int(self.ensure_int_range(
            written as i128,
            span,
            "vprintf output count is outside the supported int range",
        )?))
    }

    pub(super) fn eval_vfprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vfprintf used with an invalid va_list",
                    args[2].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let (out, used) = self.collect_printf_bytes_from_va_list(
            "vfprintf",
            format_pointer,
            args[1].span(),
            &values.args[values.index..],
            args[2].span(),
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "vfprintf", "7.19.6.8")?
            .expect("checked by caller");
        let written =
            self.write_known_bytes_to_stream(stream_object, &out, span, "vfprintf", "7.19.6.8")?;
        Ok(TypedValue::int(self.ensure_int_range(
            written as i128,
            span,
            "vfprintf output count is outside the supported int range",
        )?))
    }

    pub(super) fn eval_vsprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vsprintf used with an invalid va_list",
                    args[2].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let ((out, used), accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_printf_bytes_from_va_list(
                "vsprintf",
                format_pointer,
                args[1].span(),
                &values.args[values.index..],
                args[2].span(),
                span,
                objects,
            )
        })?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        self.reject_formatted_io_destination_overlap(
            "vsprintf",
            &dest_pointer,
            out.len() + 1,
            args[0].span(),
            &accesses,
            objects,
            "7.19.6.8",
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        self.write_c_string_to_pointer(&dest_pointer, &out, args[0].span(), objects)?;
        Ok(TypedValue::int(self.ensure_int_range(
            out.len() as i128,
            span,
            "vsprintf output count is outside the supported int range",
        )?))
    }

    pub(super) fn eval_vsnprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[3].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vsnprintf used with an invalid va_list",
                    args[3].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[2].as_pointer(args[2].span())?;
        let ((out, used), accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_printf_bytes_from_va_list(
                "vsnprintf",
                format_pointer,
                args[2].span(),
                &values.args[values.index..],
                args[3].span(),
                span,
                objects,
            )
        })?;
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "vsnprintf size")?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let copy_len = out.len().min(bound.saturating_sub(1));
        self.reject_formatted_io_destination_overlap(
            "vsnprintf",
            &dest_pointer,
            if bound == 0 { 0 } else { copy_len + 1 },
            args[0].span(),
            &accesses,
            objects,
            "7.19.6.8",
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        if bound != 0 {
            if dest_pointer.is_null() {
                return Err(Diagnostic::ub(
                    "vsnprintf requires a valid destination pointer when n is nonzero",
                    args[0].span(),
                    Some("7.19.6.8"),
                ));
            }
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest_pointer, bound, args[0].span(), objects)?;
            self.ensure_library_writable_region(
                object_id,
                start,
                copy_len + 1,
                args[0].span(),
                objects,
            )?;
            let mut rendered = out[..copy_len].to_vec();
            rendered.push(0);
            self.overlay_known_bytes_into_object(
                object_id,
                start,
                &rendered,
                args[0].span(),
                objects,
            )?;
        }
        Ok(TypedValue::int(self.ensure_int_range(
            out.len() as i128,
            span,
            "vsnprintf output count is outside the supported int range",
        )?))
    }

    fn write_wide_units_to_stream_count(
        &mut self,
        stream_object: ObjectId,
        units: &[libc::wchar_t],
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<i32, Diagnostic> {
        let mut count = 0i32;
        for &unit in units {
            self.write_wide_unit_to_stream(stream_object, unit, span, function_name, standard)?;
            count = count.saturating_add(1);
        }
        Ok(count)
    }

    pub(super) fn eval_wprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out = self.collect_wprintf_units("wprintf", 0, args, evaluated, span, objects)?;
        let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
        Ok(TypedValue::int(
            self.write_wide_units_to_stream_count(stream, &out, span, "wprintf", "7.24.2.1")?
                as i128,
        ))
    }

    pub(super) fn eval_fwprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out = self.collect_wprintf_units("fwprintf", 1, args, evaluated, span, objects)?;
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fwprintf", "7.24.2.1")?
            .expect("checked by caller");
        Ok(TypedValue::int(
            self.write_wide_units_to_stream_count(stream, &out, span, "fwprintf", "7.24.2.1")?
                as i128,
        ))
    }

    pub(super) fn eval_swprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let (out, accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_wprintf_units(
                "swprintf",
                2,
                args,
                evaluated,
                args[2].span(),
                objects,
            )
        })?;
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "swprintf size")?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let destination_units = if bound == 0 {
            0
        } else {
            out.len().min(bound.saturating_sub(1)) + 1
        };
        let destination_size = destination_units
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::ub(
                    "swprintf destination size is out of supported range",
                    args[0].span(),
                    Some("7.24.2.3"),
                )
            })?;
        self.reject_formatted_io_destination_overlap(
            "swprintf",
            &dest_pointer,
            destination_size,
            args[0].span(),
            &accesses,
            objects,
            "7.24.2.3",
        )?;
        if bound == 0 || out.len() >= bound {
            if bound != 0 {
                let copy_len = bound.saturating_sub(1);
                self.write_wide_string_to_pointer(
                    &dest_pointer,
                    &out[..copy_len.min(out.len())],
                    args[0].span(),
                    objects,
                )?;
            }
            return Ok(TypedValue::int(-1));
        }
        self.write_wide_string_to_pointer(&dest_pointer, &out, args[0].span(), objects)?;
        Ok(TypedValue::int(out.len() as i128))
    }

    pub(super) fn eval_vwprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[1].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vwprintf used with an invalid va_list",
                    args[1].span(),
                    Some("7.24.2.5"),
                )
            })?
            .clone();
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let (out, used) = self.collect_wprintf_units_from_va_list(
            "vwprintf",
            format_pointer,
            args[0].span(),
            &values.args[values.index..],
            args[1].span(),
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
        Ok(TypedValue::int(
            self.write_wide_units_to_stream_count(stream, &out, span, "vwprintf", "7.24.2.5")?
                as i128,
        ))
    }

    pub(super) fn eval_vfwprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vfwprintf used with an invalid va_list",
                    args[2].span(),
                    Some("7.24.2.5"),
                )
            })?
            .clone();
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let (out, used) = self.collect_wprintf_units_from_va_list(
            "vfwprintf",
            format_pointer,
            args[1].span(),
            &values.args[values.index..],
            args[2].span(),
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "vfwprintf", "7.24.2.5")?
            .expect("checked by caller");
        Ok(TypedValue::int(self.write_wide_units_to_stream_count(
            stream,
            &out,
            span,
            "vfwprintf",
            "7.24.2.5",
        )? as i128))
    }

    pub(super) fn eval_vswprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[3].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vswprintf used with an invalid va_list",
                    args[3].span(),
                    Some("7.24.2.5"),
                )
            })?
            .clone();
        let format_pointer = evaluated[2].as_pointer(args[2].span())?;
        let ((out, used), accesses) = self.capture_formatted_io_accesses(|interpreter| {
            interpreter.collect_wprintf_units_from_va_list(
                "vswprintf",
                format_pointer,
                args[2].span(),
                &values.args[values.index..],
                args[3].span(),
                span,
                objects,
            )
        })?;
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "vswprintf size")?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let destination_units = if bound == 0 {
            0
        } else {
            out.len().min(bound.saturating_sub(1)) + 1
        };
        let destination_size = destination_units
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vswprintf destination size is out of supported range",
                    args[0].span(),
                    Some("7.24.2.5"),
                )
            })?;
        self.reject_formatted_io_destination_overlap(
            "vswprintf",
            &dest_pointer,
            destination_size,
            args[0].span(),
            &accesses,
            objects,
            "7.24.2.5",
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        if bound == 0 || out.len() >= bound {
            if bound != 0 {
                let copy_len = bound.saturating_sub(1);
                self.write_wide_string_to_pointer(
                    &dest_pointer,
                    &out[..copy_len.min(out.len())],
                    args[0].span(),
                    objects,
                )?;
            }
            return Ok(TypedValue::int(-1));
        }
        self.write_wide_string_to_pointer(&dest_pointer, &out, args[0].span(), objects)?;
        Ok(TypedValue::int(out.len() as i128))
    }

    fn eval_scanf_conversion(
        &mut self,
        function_name: &'static str,
        conv: &ScanfConversion,
        source: &mut ByteScanfSource,
        dest: Option<&TypedValue>,
        dest_span: Span,
        call_span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<ScanConversionStatus, Diagnostic> {
        match conv.spec {
            'n' => {
                if let Some(dest) = dest {
                    self.store_scan_integer(
                        function_name,
                        conv,
                        dest,
                        dest_span,
                        Self::scan_consumed(source) as i128,
                        objects,
                    )?;
                }
                Ok(ScanConversionStatus::NoAssignment)
            }
            'c' => match conv.length {
                ScanfLength::None => {
                    let width = conv.width.unwrap_or(1);
                    let Some(bytes) = self.scan_read_exact(source, width, call_span)? else {
                        return Ok(ScanConversionStatus::InputFailure);
                    };
                    if let Some(dest) = dest {
                        self.store_scan_narrow_buffer(
                            function_name,
                            dest,
                            dest_span,
                            &bytes,
                            false,
                            objects,
                        )?;
                    }
                    Ok(if conv.suppress {
                        ScanConversionStatus::NoAssignment
                    } else {
                        ScanConversionStatus::Assigned
                    })
                }
                ScanfLength::L => {
                    let width = conv.width.unwrap_or(1);
                    let mut state = HostMbState { opaque: [0; 128] };
                    let mut units = Vec::with_capacity(width);
                    for _ in 0..width {
                        let Some((unit, _bytes)) =
                            self.scan_read_multibyte_char(source, call_span, &mut state)?
                        else {
                            return Ok(ScanConversionStatus::InputFailure);
                        };
                        units.push(unit);
                    }
                    if let Some(dest) = dest {
                        self.store_scan_wide_buffer(
                            function_name,
                            dest,
                            dest_span,
                            &units,
                            false,
                            objects,
                        )?;
                    }
                    Ok(if conv.suppress {
                        ScanConversionStatus::NoAssignment
                    } else {
                        ScanConversionStatus::Assigned
                    })
                }
                _ => Err(Diagnostic::error(
                    format!("{function_name} %c uses an unsupported length modifier"),
                    dest_span,
                )),
            },
            's' | '[' => match conv.length {
                ScanfLength::None => {
                    if conv.spec == 's' {
                        self.scan_skip_ws(source, call_span)?;
                    }
                    let mut bytes = Vec::new();
                    let mut hit_eof = false;
                    loop {
                        if conv.width.is_some_and(|limit| bytes.len() >= limit) {
                            break;
                        }
                        let Some(byte) = self.scan_next(source, call_span)? else {
                            hit_eof = true;
                            break;
                        };
                        let matches = if conv.spec == 's' {
                            (unsafe { libc::isspace(c_int::from(byte)) }) == 0
                        } else {
                            self.scanset_matches(
                                conv.scanset.as_ref().expect("scanset"),
                                libc::wchar_t::from(byte),
                            )
                        };
                        if !matches {
                            self.scan_unread(source, byte);
                            break;
                        }
                        bytes.push(byte);
                    }
                    if bytes.is_empty() {
                        return Ok(if hit_eof {
                            ScanConversionStatus::InputFailure
                        } else {
                            ScanConversionStatus::MatchingFailure
                        });
                    }
                    if let Some(dest) = dest {
                        self.store_scan_narrow_buffer(
                            function_name,
                            dest,
                            dest_span,
                            &bytes,
                            true,
                            objects,
                        )?;
                    }
                    Ok(if conv.suppress {
                        ScanConversionStatus::NoAssignment
                    } else {
                        ScanConversionStatus::Assigned
                    })
                }
                ScanfLength::L => {
                    if conv.spec == 's' {
                        self.scan_skip_ws(source, call_span)?;
                    }
                    let mut state = HostMbState { opaque: [0; 128] };
                    let mut units = Vec::new();
                    let mut hit_eof = false;
                    loop {
                        if conv.width.is_some_and(|limit| units.len() >= limit) {
                            break;
                        }
                        let Some((unit, raw_bytes)) =
                            self.scan_read_multibyte_char(source, call_span, &mut state)?
                        else {
                            hit_eof = true;
                            break;
                        };
                        let matches = if conv.spec == 's' {
                            (unsafe { iswspace(unit as c_int) }) == 0
                        } else {
                            self.scanset_matches(conv.scanset.as_ref().expect("scanset"), unit)
                        };
                        if !matches {
                            for byte in raw_bytes.iter().rev() {
                                self.scan_unread(source, *byte);
                            }
                            break;
                        }
                        units.push(unit);
                    }
                    if units.is_empty() {
                        return Ok(if hit_eof {
                            ScanConversionStatus::InputFailure
                        } else {
                            ScanConversionStatus::MatchingFailure
                        });
                    }
                    if let Some(dest) = dest {
                        self.store_scan_wide_buffer(
                            function_name,
                            dest,
                            dest_span,
                            &units,
                            true,
                            objects,
                        )?;
                    }
                    Ok(if conv.suppress {
                        ScanConversionStatus::NoAssignment
                    } else {
                        ScanConversionStatus::Assigned
                    })
                }
                _ => Err(Diagnostic::error(
                    format!(
                        "{function_name} %{} uses an unsupported length modifier",
                        conv.spec
                    ),
                    dest_span,
                )),
            },
            'd' | 'i' | 'o' | 'u' | 'x' | 'X' | 'p' | 'a' | 'A' | 'e' | 'E' | 'f' | 'F' | 'g'
            | 'G' => {
                self.scan_skip_ws(source, call_span)?;
                let (token, hit_eof) = self.scan_collect_token(source, conv.width, call_span)?;
                if token.is_empty() {
                    return Ok(if hit_eof {
                        ScanConversionStatus::InputFailure
                    } else {
                        ScanConversionStatus::MatchingFailure
                    });
                }
                let Some((item_len, complete)) =
                    self.scan_numeric_input_item_bytes(&token, conv.spec)
                else {
                    for &byte in token.iter().rev() {
                        self.scan_unread(source, byte);
                    }
                    return Ok(ScanConversionStatus::MatchingFailure);
                };
                for &byte in token[item_len..].iter().rev() {
                    self.scan_unread(source, byte);
                }
                if !complete {
                    return Ok(ScanConversionStatus::MatchingFailure);
                }
                let token = &token[..item_len];
                let parsed = match conv.spec {
                    'd' => self.scan_token_to_signed_bytes(token, 10).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    'i' => self.scan_token_to_signed_bytes(token, 0).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    'o' => self.scan_token_to_unsigned_bytes(token, 8).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    'u' => self.scan_token_to_unsigned_bytes(token, 10).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    'x' | 'X' => self.scan_token_to_unsigned_bytes(token, 16).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    'p' => self.scan_token_to_unsigned_bytes(token, 16).map(
                        |(value, consumed, overflow)| (TypedValue::int(value), consumed, overflow),
                    ),
                    _ => {
                        self.scan_token_to_float_bytes(token)
                            .map(|(value, consumed, overflow)| {
                                (
                                    TypedValue::floating(CType::Double, value),
                                    consumed,
                                    overflow,
                                )
                            })
                    }
                };
                let Some((parsed, consumed, overflow)) = parsed else {
                    return Ok(ScanConversionStatus::MatchingFailure);
                };
                for &byte in token[consumed..].iter().rev() {
                    self.scan_unread(source, byte);
                }
                if let Some(dest) = dest {
                    if overflow {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} conversion result is not representable in the receiving object"
                            ),
                            dest_span,
                            Some("7.21.6.2"),
                        ));
                    }
                    match conv.spec {
                        'd' | 'i' | 'o' | 'u' | 'x' | 'X' => {
                            self.store_scan_integer(
                                function_name,
                                conv,
                                dest,
                                dest_span,
                                parsed.to_int()?,
                                objects,
                            )?;
                        }
                        'p' => {
                            let Some(pointer_value) = self.pointer_from_scanned_address(
                                u64::try_from(parsed.to_int()?).unwrap_or(0),
                            ) else {
                                return Err(Diagnostic::ub(
                                    format!(
                                        "{function_name} %p input must be a pointer value previously produced earlier in this execution"
                                    ),
                                    dest_span,
                                    Some(Self::scanf_pointer_input_standard(function_name)),
                                ));
                            };
                            self.store_scan_pointer(
                                function_name,
                                dest,
                                dest_span,
                                pointer_value,
                                objects,
                            )?;
                        }
                        _ => {
                            self.store_scan_float(
                                function_name,
                                conv,
                                dest,
                                dest_span,
                                parsed.to_float()?,
                                objects,
                            )?;
                        }
                    }
                }
                Ok(if conv.suppress {
                    ScanConversionStatus::NoAssignment
                } else {
                    ScanConversionStatus::Assigned
                })
            }
            _ => Err(Diagnostic::error(
                format!("unsupported {function_name} conversion %{}", conv.spec),
                dest_span,
            )),
        }
    }

    fn eval_scanf_like(
        &mut self,
        function_name: &'static str,
        directives: &[ScanfDirective],
        source: &mut ByteScanfSource,
        outputs: &[TypedValue],
        output_spans: &[Span],
        call_span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(i32, usize), Diagnostic> {
        let mut progress = ScanProgress::default();
        let mut used_args = 0usize;
        for directive in directives {
            match directive {
                ScanfDirective::Whitespace => self.scan_skip_ws(source, call_span)?,
                ScanfDirective::Literal(_) | ScanfDirective::Percent => {
                    let expected = match directive {
                        ScanfDirective::Literal(unit) => *unit,
                        _ => {
                            self.scan_skip_ws(source, call_span)?;
                            '%' as libc::wchar_t
                        }
                    };
                    let Some(byte) = self.scan_next(source, call_span)? else {
                        return Ok((progress.input_failure_result(), used_args));
                    };
                    if libc::wchar_t::from(byte) != expected {
                        self.scan_unread(source, byte);
                        return Ok((progress.assignments, used_args));
                    }
                    if matches!(directive, ScanfDirective::Percent) {
                        progress.converted = true;
                    }
                }
                ScanfDirective::Conversion(conv) => {
                    let (dest, dest_span) = if conv.suppress {
                        (None, call_span)
                    } else {
                        let value = outputs.get(used_args).ok_or_else(|| {
                            Diagnostic::error(
                                format!("{function_name} is missing an output argument"),
                                call_span,
                            )
                        })?;
                        let span = *output_spans.get(used_args).unwrap_or(&call_span);
                        used_args += 1;
                        (Some(value), span)
                    };
                    let status = self.eval_scanf_conversion(
                        function_name,
                        conv,
                        source,
                        dest,
                        dest_span,
                        call_span,
                        objects,
                    )?;
                    if let Some(result) = progress.conversion_result(status) {
                        return Ok((result, used_args));
                    }
                }
            }
        }
        Ok((progress.assignments, used_args))
    }

    fn run_scan_source(
        &mut self,
        function_name: &'static str,
        directives: &[ScanfDirective],
        mut source: ByteScanfSource,
        outputs: &[TypedValue],
        output_spans: &[Span],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(i32, usize), Diagnostic> {
        let result = self.eval_scanf_like(
            function_name,
            directives,
            &mut source,
            outputs,
            output_spans,
            span,
            objects,
        );
        let finalize = self.finalize_scan_source(&mut source, span);
        match (result, finalize) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(diag), _) => Err(diag),
            (Ok(_), Err(diag)) => Err(diag),
        }
    }

    pub(super) fn eval_scanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[0].span(), objects)?;
        let directives = self.parse_scanf_format("scanf", &format_bytes, args[0].span())?;
        let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
        let output_spans = args[1..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_scan_source(
            "scanf",
            &directives,
            self.scan_source_from_stream(stream, "scanf", "7.19.6.2"),
            &evaluated[1..],
            &output_spans,
            span,
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_fscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_scanf_format("fscanf", &format_bytes, args[1].span())?;
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fscanf", "7.19.6.2")?
            .expect("checked by caller");
        let output_spans = args[2..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_scan_source(
            "fscanf",
            &directives,
            self.scan_source_from_stream(stream, "fscanf", "7.19.6.2"),
            &evaluated[2..],
            &output_spans,
            span,
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_sscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let source_bytes = self.read_c_string_bytes(source_pointer, args[0].span(), objects)?;
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_scanf_format("sscanf", &format_bytes, args[1].span())?;
        let output_spans = args[2..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_scan_source(
            "sscanf",
            &directives,
            self.scan_source_from_bytes(source_bytes),
            &evaluated[2..],
            &output_spans,
            args[1].span(),
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[1].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vscanf used with an invalid va_list",
                    args[1].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[0].span(), objects)?;
        let directives = self.parse_scanf_format("vscanf", &format_bytes, args[0].span())?;
        let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
        let output_spans = vec![args[1].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_scan_source(
            "vscanf",
            &directives,
            self.scan_source_from_stream(stream, "vscanf", "7.19.6.8"),
            &values.args[values.index..],
            &output_spans,
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vfscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vfscanf used with an invalid va_list",
                    args[2].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_scanf_format("vfscanf", &format_bytes, args[1].span())?;
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "vfscanf", "7.19.6.8")?
            .expect("checked by caller");
        let output_spans = vec![args[2].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_scan_source(
            "vfscanf",
            &directives,
            self.scan_source_from_stream(stream, "vfscanf", "7.19.6.8"),
            &values.args[values.index..],
            &output_spans,
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vsscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vsscanf used with an invalid va_list",
                    args[2].span(),
                    Some("7.19.6.8"),
                )
            })?
            .clone();
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let source_bytes = self.read_c_string_bytes(source_pointer, args[0].span(), objects)?;
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_bytes = self.read_c_string_bytes(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_scanf_format("vsscanf", &format_bytes, args[1].span())?;
        let output_spans = vec![args[2].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_scan_source(
            "vsscanf",
            &directives,
            self.scan_source_from_bytes(source_bytes),
            &values.args[values.index..],
            &output_spans,
            args[1].span(),
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    fn run_wscanf_source(
        &mut self,
        function_name: &'static str,
        directives: &[ScanfDirective],
        mut source: WideScanfSource,
        outputs: &[TypedValue],
        output_spans: &[Span],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(i32, usize), Diagnostic> {
        let result = self.eval_wscanf_like(
            function_name,
            directives,
            &mut source,
            outputs,
            output_spans,
            span,
            objects,
        );
        let finalize = self.finalize_wide_scan_source(&mut source, span);
        match (result, finalize) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(diag), _) => Err(diag),
            (Ok(_), Err(diag)) => Err(diag),
        }
    }

    pub(super) fn eval_wscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[0].span(), objects)?;
        let directives = self.parse_wide_scanf_format("wscanf", &format_units, args[0].span())?;
        let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
        let source = self.wide_scan_source_from_stream(stream, "wscanf", "7.24.2.2");
        let output_spans = args[1..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_wscanf_source(
            "wscanf",
            &directives,
            source,
            &evaluated[1..],
            &output_spans,
            span,
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_fwscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_wide_scanf_format("fwscanf", &format_units, args[1].span())?;
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fwscanf", "7.24.2.2")?
            .expect("checked by caller");
        let source = self.wide_scan_source_from_stream(stream, "fwscanf", "7.24.2.2");
        let output_spans = args[2..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_wscanf_source(
            "fwscanf",
            &directives,
            source,
            &evaluated[2..],
            &output_spans,
            span,
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_swscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let source_units = self.read_wide_string_units(source_pointer, args[0].span(), objects)?;
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_wide_scanf_format("swscanf", &format_units, args[1].span())?;
        let output_spans = args[2..].iter().map(Expr::span).collect::<Vec<_>>();
        let (assigned, _) = self.run_wscanf_source(
            "swscanf",
            &directives,
            self.wide_scan_source_from_units(source_units),
            &evaluated[2..],
            &output_spans,
            args[1].span(),
            objects,
        )?;
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vwscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[1].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vwscanf used with an invalid va_list",
                    args[1].span(),
                    Some("7.24.2.6"),
                )
            })?
            .clone();
        let format_pointer = evaluated[0].as_pointer(args[0].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[0].span(), objects)?;
        let directives = self.parse_wide_scanf_format("vwscanf", &format_units, args[0].span())?;
        let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
        let output_spans = vec![args[1].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_wscanf_source(
            "vwscanf",
            &directives,
            self.wide_scan_source_from_stream(stream, "vwscanf", "7.24.2.6"),
            &values.args[values.index..],
            &output_spans,
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vfwscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vfwscanf used with an invalid va_list",
                    args[2].span(),
                    Some("7.24.2.6"),
                )
            })?
            .clone();
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_wide_scanf_format("vfwscanf", &format_units, args[1].span())?;
        let stream = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "vfwscanf", "7.24.2.6")?
            .expect("checked by caller");
        let output_spans = vec![args[2].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_wscanf_source(
            "vfwscanf",
            &directives,
            self.wide_scan_source_from_stream(stream, "vfwscanf", "7.24.2.6"),
            &values.args[values.index..],
            &output_spans,
            span,
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_vswscanf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let handle = u64::try_from(evaluated[2].to_int()?).unwrap_or(0);
        let values = self
            .va_lists
            .get(&handle)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "vswscanf used with an invalid va_list",
                    args[2].span(),
                    Some("7.24.2.6"),
                )
            })?
            .clone();
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let source_units = self.read_wide_string_units(source_pointer, args[0].span(), objects)?;
        let format_pointer = evaluated[1].as_pointer(args[1].span())?;
        let format_units = self.read_wide_string_units(format_pointer, args[1].span(), objects)?;
        let directives = self.parse_wide_scanf_format("vswscanf", &format_units, args[1].span())?;
        let output_spans = vec![args[2].span(); values.args.len().saturating_sub(values.index)];
        let (assigned, used) = self.run_wscanf_source(
            "vswscanf",
            &directives,
            self.wide_scan_source_from_units(source_units),
            &values.args[values.index..],
            &output_spans,
            args[1].span(),
            objects,
        )?;
        if let Some(cursor) = self.va_lists.get_mut(&handle) {
            cursor.index = cursor.index.saturating_add(used);
        }
        Ok(TypedValue::int(assigned as i128))
    }

    pub(super) fn eval_puts_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let bytes = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
        let mut rendered = bytes.clone();
        rendered.push(b'\n');
        let len = self.write_known_bytes_to_stream(stream, &rendered, span, "puts", "7.19.7.9")?;
        let count = self.ensure_int_range(
            len as i128,
            span,
            "puts output count is outside the supported int range",
        )?;
        Ok(TypedValue::int(count))
    }

    pub(super) fn eval_remove_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let path = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let result = match VirtualFileSystem::normalize_path(&path)
            .and_then(|path| self.virtual_filesystem.remove(&path))
        {
            Ok(()) => 0,
            Err(errno) => {
                Self::set_host_errno(errno);
                -1
            }
        };
        Ok(TypedValue::int(result))
    }

    pub(super) fn eval_rename_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let old_path = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let new_path = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let result = match (
            VirtualFileSystem::normalize_path(&old_path),
            VirtualFileSystem::normalize_path(&new_path),
        ) {
            (Ok(old_path), Ok(new_path)) => {
                match self.virtual_filesystem.rename(&old_path, &new_path) {
                    Ok(()) => 0,
                    Err(errno) => {
                        Self::set_host_errno(errno);
                        -1
                    }
                }
            }
            (Err(errno), _) | (_, Err(errno)) => {
                Self::set_host_errno(errno);
                -1
            }
        };
        Ok(TypedValue::int(result))
    }

    pub(super) fn eval_tmpfile_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let return_ty = self.host_function_return_type("tmpfile", span.file, span)?;
        let file_id = self.virtual_filesystem.create_file(None);
        let stream_object = self.allocate_host_stream_object(
            objects,
            StorageDuration::Dynamic,
            span,
            StreamBacking::File { file_id },
            HostStreamMode {
                kind: HostStreamModeKind::Update,
                binary: true,
            },
            false,
        )?;
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(stream_object)))
    }

    pub(super) fn eval_tmpnam_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let return_ty = self.host_function_return_type("tmpnam", span.file, span)?;
        let temp = self.virtual_filesystem.temporary_name();
        let dest = if pointer.is_null() {
            self.tmpnam_buffer_pointer(objects, span)?
        } else {
            pointer
        };
        self.write_c_string_to_pointer(&dest, temp.as_bytes(), span, objects)?;
        Ok(self.pointer_value_with_type(return_ty, dest))
    }

    pub(super) fn eval_fclose_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fclose", "7.19.5.1")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[0].span(), "fclose", "7.19.5.1")?;
        self.clear_stream_buffer_config(stream_object);
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.open = false;
            stream.pushback.clear();
        }
        let _ = objects;
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_fflush_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let result = if evaluated[0].as_pointer(args[0].span())?.is_null() {
            for stream in self.host_streams.values_mut() {
                if stream.open
                    && matches!(
                        stream.mode.kind,
                        HostStreamModeKind::Output | HostStreamModeKind::Update
                    )
                {
                    stream.last_operation = None;
                    stream.last_input_hit_eof = false;
                    stream.operation_performed = true;
                }
            }
            0
        } else {
            let stream_object = self
                .stream_object_from_value(&evaluated[0], args[0].span(), "fflush", "7.19.5.2")?
                .expect("checked by caller");
            self.require_open_stream(stream_object, args[0].span(), "fflush", "7.19.5.2")?;
            self.clear_stream_last_operation(stream_object);
            0
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fopen_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let path = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let mode = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let parsed_mode = self
            .parse_standard_fopen_mode(&mode)
            .expect("validated by check_fopen_call");
        let return_ty = self.host_function_return_type("fopen", span.file, span)?;
        let normalized = match VirtualFileSystem::normalize_path(&path) {
            Ok(path) => path,
            Err(errno) => {
                Self::set_host_errno(errno);
                return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
            }
        };
        let (file_id, position) = match self.virtual_filesystem.open_file(&normalized, parsed_mode)
        {
            Ok(opened) => opened,
            Err(errno) => {
                Self::set_host_errno(errno);
                return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
            }
        };
        if !self.virtual_filesystem.files.contains_key(&file_id) {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let stream_object = self.allocate_host_stream_object(
            objects,
            StorageDuration::Dynamic,
            span,
            StreamBacking::File { file_id },
            HostStreamMode {
                kind: parsed_mode.kind,
                binary: parsed_mode.binary,
            },
            parsed_mode.opening == FopenOpening::Append,
        )?;
        self.host_streams.get_mut(&stream_object).unwrap().position = position;
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(stream_object)))
    }

    pub(super) fn eval_freopen_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let path = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let mode = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let parsed_mode = self
            .parse_standard_fopen_mode(&mode)
            .expect("validated by check_freopen_call");
        let stream_object = self
            .stream_object_from_value(&evaluated[2], args[2].span(), "freopen", "7.19.5.4")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[2].span(), "freopen", "7.19.5.4")?;
        let return_ty = self.host_function_return_type("freopen", span.file, span)?;
        self.clear_stream_buffer_config(stream_object);
        let opened = VirtualFileSystem::normalize_path(&path)
            .and_then(|path| self.virtual_filesystem.open_file(&path, parsed_mode));
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            match opened {
                Ok((file_id, position)) => {
                    stream.backing = StreamBacking::File { file_id };
                    stream.open = true;
                    stream.position = position;
                    stream.append = parsed_mode.opening == FopenOpening::Append;
                    stream.pushback.clear();
                    stream.orientation = 0;
                    stream.eof = false;
                    stream.error = false;
                    stream.mode = HostStreamMode {
                        kind: parsed_mode.kind,
                        binary: parsed_mode.binary,
                    };
                    stream.last_operation = None;
                    stream.last_input_hit_eof = false;
                    stream.operation_performed = false;
                }
                Err(errno) => {
                    Self::set_host_errno(errno);
                    stream.open = false;
                    stream.error = true;
                }
            }
        }
        if opened.is_err() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(stream_object)))
    }

    pub(super) fn eval_setbuf_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "setbuf", "7.19.5.5")?
            .expect("checked by caller");
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        let is_null = pointer.is_null();
        self.replace_stream_buffer_config(
            stream_object,
            Some(StreamBufferConfig {
                pointer: (!is_null).then_some(pointer),
                mode: if is_null { libc::_IONBF } else { libc::_IOFBF },
                size: HOST_BUFSIZ,
                supplied_at: args[1].span(),
            }),
        );
        Ok(TypedValue::void())
    }

    pub(super) fn eval_setvbuf_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "setvbuf", "7.19.5.6")?
            .expect("checked by caller");
        let mode = evaluated[2].to_int()? as c_int;
        if !matches!(mode, libc::_IOFBF | libc::_IOLBF | libc::_IONBF) {
            return Ok(TypedValue::int(1));
        }
        let size =
            self.checked_usize_from_unsigned_long(&evaluated[3], args[3].span(), "setvbuf size")?;
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        let use_supplied_buffer = !pointer.is_null() && mode != libc::_IONBF && size != 0;
        self.replace_stream_buffer_config(
            stream_object,
            Some(StreamBufferConfig {
                pointer: use_supplied_buffer.then_some(pointer),
                mode,
                size,
                supplied_at: args[1].span(),
            }),
        );
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_fgetc_like_call(
        &mut self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<TypedValue, Diagnostic> {
        let ch = self
            .read_one_byte_from_stream(stream_object, span, function_name, standard)?
            .map_or(HOST_EOF, c_int::from);
        Ok(TypedValue::int(ch as i128))
    }

    pub(super) fn eval_fputc_like_call(
        &mut self,
        ch: c_int,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<TypedValue, Diagnostic> {
        let byte = ch as u8;
        self.write_known_bytes_to_stream(stream_object, &[byte], span, function_name, standard)?;
        let result = c_int::from(byte);
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fgetwc_like_call(
        &mut self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<TypedValue, Diagnostic> {
        let ch = self
            .read_wide_unit_from_stream(stream_object, span, function_name, standard)?
            .map_or(HOST_EOF, |unit| unit as c_int);
        Ok(TypedValue::int(ch as i128))
    }

    pub(super) fn eval_fputwc_like_call(
        &mut self,
        ch: libc::wchar_t,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<TypedValue, Diagnostic> {
        self.write_wide_unit_to_stream(stream_object, ch, span, function_name, standard)?;
        let result = ch;
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fputs_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let bytes = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let stream_object = self
            .stream_object_from_value(&evaluated[1], args[1].span(), "fputs", "7.19.7.4")?
            .expect("checked by caller");
        self.write_known_bytes_to_stream(
            stream_object,
            &bytes,
            args[1].span(),
            "fputs",
            "7.19.7.4",
        )?;
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_fputws_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let units = self.wide_c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let stream_object = self
            .stream_object_from_value(&evaluated[1], args[1].span(), "fputws", "7.24.3.6")?
            .expect("checked by caller");
        for unit in units {
            self.write_wide_unit_to_stream(
                stream_object,
                unit,
                args[1].span(),
                "fputws",
                "7.24.3.6",
            )?;
        }
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_fgets_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count = evaluated[1].to_int()? as c_int;
        let return_ty = self.host_function_return_type("fgets", span.file, span)?;
        if count <= 0 {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let stream_object = self
            .stream_object_from_value(&evaluated[2], args[2].span(), "fgets", "7.19.7.2")?
            .expect("checked by caller");
        let mut buffer = Vec::with_capacity(count as usize);
        while buffer.len() + 1 < count as usize {
            let Some(byte) =
                self.read_one_byte_from_stream(stream_object, args[2].span(), "fgets", "7.19.7.2")?
            else {
                break;
            };
            buffer.push(byte);
            if byte == b'\n' {
                break;
            }
        }
        if buffer.is_empty()
            && self
                .host_streams
                .get(&stream_object)
                .is_some_and(|stream| stream.eof)
        {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        self.write_c_string_to_pointer(&dest_pointer, &buffer, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, dest_pointer))
    }

    pub(super) fn eval_fgetws_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count = evaluated[1].to_int()? as c_int;
        let return_ty = self.host_function_return_type("fgetws", span.file, span)?;
        if count <= 0 {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let stream_object = self
            .stream_object_from_value(&evaluated[2], args[2].span(), "fgetws", "7.24.3.2")?
            .expect("checked by caller");
        let mut buffer = Vec::with_capacity(count as usize);
        while buffer.len() + 1 < count as usize {
            let Some(unit) = self.read_wide_unit_from_stream(
                stream_object,
                args[2].span(),
                "fgetws",
                "7.24.3.2",
            )?
            else {
                break;
            };
            buffer.push(unit);
            if unit == '\n' as libc::wchar_t {
                break;
            }
        }
        if buffer.is_empty()
            && self
                .host_streams
                .get(&stream_object)
                .is_some_and(|stream| stream.eof)
        {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        self.write_wide_string_to_pointer(&dest_pointer, &buffer, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, dest_pointer))
    }

    pub(super) fn eval_fwide_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fwide", "7.24.3.5")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[0].span(), "fwide", "7.24.3.5")?;
        let requested = evaluated[1].to_int()?;
        let stream = self.host_streams.get_mut(&stream_object).unwrap();
        if stream.orientation == 0 && requested != 0 {
            stream.orientation = if requested < 0 { -1 } else { 1 };
        }
        Ok(TypedValue::int(stream.orientation as i128))
    }

    pub(super) fn eval_gets_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let stream_object = self.standard_stream_object(CaptureStream::Stdin, span)?;
        let bytes =
            self.read_entire_c_string_from_stream(stream_object, span, "gets", "7.19.7.7")?;
        let max = self
            .object_pointer_limit(objects, &dest_pointer, &CType::Char)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "gets destination is not a valid live character array",
                    args[0].span(),
                    Some("7.19.7.7"),
                )
            })?;
        if (bytes.len() + 1) as isize > max.saturating_sub(dest_pointer.offset) {
            return Err(Diagnostic::ub(
                "gets would write past the end of the destination array",
                args[0].span(),
                Some("7.19.7.7"),
            ));
        }
        let return_ty = self.host_function_return_type("gets", span.file, span)?;
        if bytes.is_empty()
            && self
                .host_streams
                .get(&stream_object)
                .is_some_and(|stream| stream.eof)
        {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        self.write_c_string_to_pointer(&dest_pointer, &bytes, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, dest_pointer))
    }

    pub(super) fn eval_fread_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "fread size")?;
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "fread count")?;
        let total = size.checked_mul(count).ok_or_else(|| {
            Diagnostic::error(
                "fread byte count is out of supported range",
                args[1].span().merge(args[2].span()),
            )
        })?;
        let stream_object = self
            .stream_object_from_value(&evaluated[3], args[3].span(), "fread", "7.19.8.1")?
            .expect("checked by caller");
        if total == 0 {
            return Ok(TypedValue::integer(CType::UnsignedLong, 0));
        }
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let (object_id, start, _) =
            self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, total, args[0].span(), objects)?;
        let bytes =
            self.read_bytes_from_stream(stream_object, total, args[3].span(), "fread", "7.19.8.1")?;
        self.overlay_known_bytes_into_object(object_id, start, &bytes, args[0].span(), objects)?;
        if !bytes.len().is_multiple_of(size) {
            let partial_start = start + (bytes.len() / size) * size;
            self.overlay_byte_cells_into_object(
                object_id,
                partial_start,
                &vec![ByteCell::Indeterminate; size],
                args[0].span(),
                objects,
            )?;
        }
        let items = bytes.len() / size;
        Ok(TypedValue::integer(CType::UnsignedLong, items as i128))
    }

    pub(super) fn eval_fwrite_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "fwrite size")?;
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "fwrite count")?;
        let total = size.checked_mul(count).ok_or_else(|| {
            Diagnostic::error(
                "fwrite byte count is out of supported range",
                args[1].span().merge(args[2].span()),
            )
        })?;
        let stream_object = self
            .stream_object_from_value(&evaluated[3], args[3].span(), "fwrite", "7.19.8.2")?
            .expect("checked by caller");
        if total == 0 {
            return Ok(TypedValue::integer(CType::UnsignedLong, 0));
        }
        let src_pointer = evaluated[0].as_pointer(args[0].span())?;
        let bytes = self.read_pointer_bytes(&src_pointer, total, args[0].span(), objects)?;
        let written = self.write_known_bytes_to_stream(
            stream_object,
            &bytes,
            args[3].span(),
            "fwrite",
            "7.19.8.2",
        )?;
        let items = written / size;
        Ok(TypedValue::integer(CType::UnsignedLong, items as i128))
    }

    pub(super) fn eval_fseek_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fseek", "7.19.9.2")?
            .expect("checked by caller");
        let result = self.seek_stream(
            stream_object,
            evaluated[1].to_int()?,
            evaluated[2].to_int()? as c_int,
            args[0].span(),
            "fseek",
            "7.19.9.2",
        )?;
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_ftell_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "ftell", "7.19.9.3")?
            .expect("checked by caller");
        let result = self.tell_stream(stream_object, args[0].span(), "ftell", "7.19.9.3")? as i128;
        let backing = self
            .host_streams
            .get(&stream_object)
            .expect("checked stream object")
            .backing;
        self.ftell_provenance.insert((backing, result));
        Ok(TypedValue::integer(CType::Long, result))
    }

    pub(super) fn eval_rewind_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "rewind", "7.19.9.5")?
            .expect("checked by caller");
        let _ = self.seek_stream(
            stream_object,
            0,
            libc::SEEK_SET,
            args[0].span(),
            "rewind",
            "7.19.9.5",
        )?;
        Ok(TypedValue::void())
    }

    pub(super) fn eval_fgetpos_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fgetpos", "7.19.9.1")?
            .expect("checked by caller");
        let pos = self.tell_stream(stream_object, args[0].span(), "fgetpos", "7.19.9.1")? as i128;
        let pointer = evaluated[1].as_pointer(args[1].span())?;
        let lvalue = LValue {
            object: pointer
                .object
                .expect("position pointer must name an object"),
            ty: CType::Long,
            base_offset: pointer.base_offset,
            offset: pointer.offset,
            member_path: pointer.member_path.clone(),
            designated_root_ty: pointer.designated_root_ty.clone(),
            byte_offset_override: pointer.byte_offset_override,
            arithmetic_domain_start: pointer.arithmetic_domain_start,
            object_representation_domain: pointer.object_representation_domain,
            bit_field_width: None,
            restrict_source: None,
        };
        self.store_lvalue(
            objects,
            &lvalue,
            TypedValue::integer(CType::Long, pos),
            args[1].span(),
        )?;
        if pos >= 0 {
            let (object_id, start, _) =
                self.byte_region_from_pointer(&pointer, 8, args[1].span(), objects)?;
            let version = self
                .lookup_object(objects, object_id)
                .expect("fpos_t output object exists")
                .modification_count;
            let backing = self
                .host_streams
                .get(&stream_object)
                .expect("checked stream object")
                .backing;
            self.fpos_provenance
                .insert((object_id, start), (version, backing));
        }
        Ok(TypedValue::int(if pos < 0 { -1 } else { 0 }))
    }

    pub(super) fn eval_fsetpos_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "fsetpos", "7.19.9.4")?
            .expect("checked by caller");
        let pos_pointer = evaluated[1].as_pointer(args[1].span())?;
        let pos_lvalue = LValue {
            object: pos_pointer
                .object
                .expect("position pointer must name an object"),
            ty: CType::Long,
            base_offset: pos_pointer.base_offset,
            offset: pos_pointer.offset,
            member_path: pos_pointer.member_path.clone(),
            designated_root_ty: pos_pointer.designated_root_ty.clone(),
            byte_offset_override: pos_pointer.byte_offset_override,
            arithmetic_domain_start: pos_pointer.arithmetic_domain_start,
            object_representation_domain: pos_pointer.object_representation_domain,
            bit_field_width: None,
            restrict_source: None,
        };
        let pos = self
            .load_lvalue(pos_lvalue, args[1].span(), objects)?
            .to_int()?;
        let result = self.seek_stream(
            stream_object,
            pos,
            libc::SEEK_SET,
            args[0].span(),
            "fsetpos",
            "7.19.9.4",
        )?;
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_feof_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "feof", "7.19.10.2")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[0].span(), "feof", "7.19.10.2")?;
        Ok(TypedValue::int(
            self.host_streams
                .get(&stream_object)
                .is_some_and(|stream| stream.eof) as i128,
        ))
    }

    pub(super) fn eval_ferror_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "ferror", "7.19.10.1")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[0].span(), "ferror", "7.19.10.1")?;
        Ok(TypedValue::int(
            self.host_streams
                .get(&stream_object)
                .is_some_and(|stream| stream.error) as i128,
        ))
    }

    pub(super) fn eval_clearerr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[0], args[0].span(), "clearerr", "7.19.10.1")?
            .expect("checked by caller");
        self.require_open_stream(stream_object, args[0].span(), "clearerr", "7.19.10.1")?;
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.eof = false;
            stream.error = false;
        }
        Ok(TypedValue::void())
    }

    pub(super) fn eval_ungetc_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[1], args[1].span(), "ungetc", "7.19.7.10")?
            .expect("checked by caller");
        let input = evaluated[0].to_int()? as c_int;
        let result = if input == HOST_EOF {
            HOST_EOF
        } else {
            self.orient_stream_if_unoriented(stream_object, -1);
            let byte = input as u8;
            self.unread_byte_to_stream(stream_object, byte, args[1].span(), "ungetc", "7.19.7.10")?;
            c_int::from(byte)
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_ungetwc_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let stream_object = self
            .stream_object_from_value(&evaluated[1], args[1].span(), "ungetwc", "7.24.3.11")?
            .expect("checked by caller");
        let wc = self.wchar_scalar_to_host(&evaluated[0], args[0].span(), "ungetwc")?;
        self.orient_stream_if_unoriented(stream_object, 1);
        let scalar = char::from_u32(wc as u32);
        let result = if let Some(scalar) = scalar {
            let mut encoded = [0u8; 4];
            for &byte in scalar.encode_utf8(&mut encoded).as_bytes().iter().rev() {
                self.unread_byte_to_stream(
                    stream_object,
                    byte,
                    args[1].span(),
                    "ungetwc",
                    "7.24.3.11",
                )?;
            }
            wc as c_int
        } else {
            HOST_EOF
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_perror_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let prefix = if evaluated[0].as_pointer(args[0].span())?.is_null() {
            Vec::new()
        } else {
            self.read_c_string_bytes(
                evaluated[0].as_pointer(args[0].span())?,
                args[0].span(),
                objects,
            )?
        };
        let errno = unsafe { *host_errno_ptr() };
        let message_ptr = unsafe { libc::strerror(errno) };
        let message = if message_ptr.is_null() {
            Vec::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(message_ptr) }
                .to_bytes()
                .to_vec()
        };
        let mut rendered = Vec::new();
        if !prefix.is_empty() {
            rendered.extend_from_slice(&prefix);
            rendered.extend_from_slice(b": ");
        }
        rendered.extend_from_slice(&message);
        rendered.push(b'\n');
        let stream = self.standard_stream_object(CaptureStream::Stderr, span)?;
        let _ = self.write_known_bytes_to_stream(stream, &rendered, span, "perror", "7.19.10.4")?;
        Ok(TypedValue::void())
    }

    pub(super) fn eval_assert_fail_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let expression = self.read_c_string(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let file = self.read_c_string(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let line = evaluated[2].to_int()?;
        let function = self.read_c_string(
            evaluated[3].as_pointer(args[3].span())?,
            args[3].span(),
            objects,
        )?;
        let rendered = format!(
            "{}:{}: {}: assertion `{}` failed\n",
            file, line, function, expression
        );
        let stderr = self.standard_stream_object(CaptureStream::Stderr, span)?;
        let _ = self.write_known_bytes_to_stream(
            stderr,
            rendered.as_bytes(),
            span,
            "assert",
            "7.2.1.1",
        )?;
        Err(
            Diagnostic::error(format!("assertion failed: {}", expression), span)
                .with_note(format!("in {}:{} ({})", file, line, function)),
        )
    }

    pub(super) fn eval_errno_location_call(
        &mut self,
        span: Span,
        _objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let object = self
            .errno_binding
            .ok_or_else(|| Diagnostic::error("errno storage is not initialized", span))?;
        let return_ty =
            self.host_function_return_type("__codex_errno_location", span.file, span)?;
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(object)))
    }

    pub(super) fn eval_ctype_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        _span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let value = evaluated[0].to_int()? as c_int;
        let result = unsafe {
            match function_name {
                "isalnum" => libc::isalnum(value),
                "isalpha" => libc::isalpha(value),
                "isblank" => libc::isblank(value),
                "iscntrl" => libc::iscntrl(value),
                "isdigit" => libc::isdigit(value),
                "isgraph" => libc::isgraph(value),
                "islower" => libc::islower(value),
                "isprint" => libc::isprint(value),
                "ispunct" => libc::ispunct(value),
                "isspace" => libc::isspace(value),
                "isupper" => libc::isupper(value),
                "isxdigit" => libc::isxdigit(value),
                "tolower" => libc::tolower(value),
                "toupper" => libc::toupper(value),
                _ => unreachable!("checked by caller"),
            }
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn validated_wctype_descriptor(
        &self,
        raw: i128,
        span: Span,
    ) -> Result<libc::c_uint, Diagnostic> {
        if raw == 0 || self.wctype_descriptors.get(&raw).copied() != Some(self.locale_generation) {
            return Err(Diagnostic::ub(
                "iswctype was called with an invalid or locale-stale wctype_t descriptor",
                span,
                Some("7.25.2.2"),
            ));
        }
        Ok(raw as libc::c_uint)
    }

    pub(super) fn validated_wctrans_descriptor(
        &self,
        raw: i128,
        span: Span,
    ) -> Result<c_int, Diagnostic> {
        if raw == 0 || self.wctrans_descriptors.get(&raw).copied() != Some(self.locale_generation) {
            return Err(Diagnostic::ub(
                "towctrans was called with an invalid or locale-stale wctrans_t descriptor",
                span,
                Some("7.25.3.2"),
            ));
        }
        Ok(raw as c_int)
    }

    pub(super) fn eval_wctype_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        match function_name {
            "iswalnum" | "iswalpha" | "iswblank" | "iswcntrl" | "iswdigit" | "iswgraph"
            | "iswlower" | "iswprint" | "iswpunct" | "iswspace" | "iswupper" | "iswxdigit"
            | "towlower" | "towupper" => {
                let wc = evaluated[0].to_int()? as c_int;
                let result = unsafe {
                    match function_name {
                        "iswalnum" => iswalnum(wc),
                        "iswalpha" => iswalpha(wc),
                        "iswblank" => iswblank(wc),
                        "iswcntrl" => iswcntrl(wc),
                        "iswdigit" => iswdigit(wc),
                        "iswgraph" => iswgraph(wc),
                        "iswlower" => iswlower(wc),
                        "iswprint" => iswprint(wc),
                        "iswpunct" => iswpunct(wc),
                        "iswspace" => iswspace(wc),
                        "iswupper" => iswupper(wc),
                        "iswxdigit" => iswxdigit(wc),
                        "towlower" => towlower(wc),
                        "towupper" => towupper(wc),
                        _ => unreachable!("checked by caller"),
                    }
                };
                Ok(TypedValue::integer(
                    self.host_function_return_type(function_name, span.file, span)?,
                    result as i128,
                ))
            }
            "wctrans" => {
                let property = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
                let desc = unsafe { wctrans(property.as_ptr()) };
                if desc != 0 {
                    self.wctrans_descriptors
                        .insert(desc as i128, self.locale_generation);
                }
                Ok(TypedValue::integer(
                    self.host_function_return_type("wctrans", span.file, span)?,
                    desc as i128,
                ))
            }
            "wctype" => {
                let property = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
                let desc = unsafe { wctype(property.as_ptr()) };
                if desc != 0 {
                    self.wctype_descriptors
                        .insert(desc as i128, self.locale_generation);
                }
                Ok(TypedValue::integer(
                    self.host_function_return_type("wctype", span.file, span)?,
                    desc as i128,
                ))
            }
            "iswctype" => {
                let wc = evaluated[0].to_int()? as c_int;
                let desc =
                    self.validated_wctype_descriptor(evaluated[1].to_int()?, args[1].span())?;
                Ok(TypedValue::int(unsafe { iswctype(wc, desc) } as i128))
            }
            "towctrans" => {
                let wc = evaluated[0].to_int()? as c_int;
                let desc =
                    self.validated_wctrans_descriptor(evaluated[1].to_int()?, args[1].span())?;
                Ok(TypedValue::int(unsafe { towctrans(wc, desc) } as i128))
            }
            _ => Err(Diagnostic::error(
                format!("unsupported host library function {function_name}"),
                span,
            )),
        }
    }
}
