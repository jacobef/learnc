use super::*;

impl<'a> Interpreter<'a> {
    fn parse_printf_count_core<F>(
        &self,
        function_name: &str,
        what: &str,
        span: Span,
        index: &mut usize,
        char_at: &mut F,
    ) -> Result<Option<PrintfCount>, Diagnostic>
    where
        F: FnMut(usize) -> Option<char>,
    {
        match char_at(*index) {
            Some('*') => {
                *index += 1;
                Ok(Some(PrintfCount::FromArg))
            }
            Some(ch) if ch.is_ascii_digit() => {
                let start = *index;
                let mut value = 0i32;
                while let Some(ch) = char_at(*index) {
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    value = value
                        .checked_mul(10)
                        .and_then(|current| current.checked_add((ch as i32) - ('0' as i32)))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("{function_name} {what} is out of supported range"),
                                span,
                            )
                        })?;
                    *index += 1;
                }
                debug_assert!(*index > start);
                Ok(Some(PrintfCount::Literal(value)))
            }
            _ => Ok(None),
        }
    }

    fn parse_printf_conversion_core<F>(
        &self,
        function_name: &str,
        span: Span,
        index: &mut usize,
        mut char_at: F,
    ) -> Result<PrintfConversion, Diagnostic>
    where
        F: FnMut(usize) -> Option<char>,
    {
        let mut flags = String::new();
        while let Some(ch) = char_at(*index) {
            if matches!(ch, '-' | '+' | ' ' | '#' | '0') {
                flags.push(ch);
                *index += 1;
            } else {
                break;
            }
        }
        let width =
            self.parse_printf_count_core(function_name, "field width", span, index, &mut char_at)?;
        let precision = if char_at(*index) == Some('.') {
            *index += 1;
            self.parse_printf_count_core(function_name, "precision", span, index, &mut char_at)?
                .or(Some(PrintfCount::Literal(0)))
        } else {
            None
        };
        let length = if char_at(*index) == Some('h') && char_at(*index + 1) == Some('h') {
            *index += 2;
            PrintfLength::Hh
        } else if char_at(*index) == Some('h') {
            *index += 1;
            PrintfLength::H
        } else if char_at(*index) == Some('l') && char_at(*index + 1) == Some('l') {
            *index += 2;
            PrintfLength::Ll
        } else if char_at(*index) == Some('l') {
            *index += 1;
            PrintfLength::L
        } else if char_at(*index) == Some('j') {
            *index += 1;
            PrintfLength::J
        } else if char_at(*index) == Some('z') {
            *index += 1;
            PrintfLength::Z
        } else if char_at(*index) == Some('t') {
            *index += 1;
            PrintfLength::T
        } else if char_at(*index) == Some('L') {
            *index += 1;
            PrintfLength::BigL
        } else {
            PrintfLength::None
        };
        let spec = char_at(*index).ok_or_else(|| {
            Diagnostic::error(format!("incomplete {function_name} format specifier"), span)
        })?;
        *index += 1;
        Ok(PrintfConversion {
            flags,
            width,
            precision,
            length,
            spec,
        })
    }

    pub(super) fn parse_printf_conversion_bytes(
        &self,
        function_name: &str,
        format: &[u8],
        index: &mut usize,
        span: Span,
    ) -> Result<PrintfConversion, Diagnostic> {
        self.parse_printf_conversion_core(function_name, span, index, |offset| {
            format.get(offset).copied().map(char::from)
        })
    }

    pub(super) fn parse_printf_conversion_wide(
        &self,
        function_name: &str,
        format: &[libc::wchar_t],
        index: &mut usize,
        span: Span,
    ) -> Result<PrintfConversion, Diagnostic> {
        self.parse_printf_conversion_core(function_name, span, index, |offset| {
            format
                .get(offset)
                .and_then(|unit| char::from_u32(*unit as u32))
        })
    }

    fn printf_length_text(&self, length: PrintfLength) -> &'static str {
        match length {
            PrintfLength::None => "",
            PrintfLength::Hh => "hh",
            PrintfLength::H => "h",
            PrintfLength::L => "l",
            PrintfLength::Ll => "ll",
            PrintfLength::J => "j",
            PrintfLength::Z => "z",
            PrintfLength::T => "t",
            PrintfLength::BigL => "L",
        }
    }

    pub(super) fn printf_conversion_text(&self, conv: &PrintfConversion) -> String {
        let mut text = String::from("%");
        text.push_str(&conv.flags);
        match conv.width {
            Some(PrintfCount::Literal(value)) => text.push_str(&value.to_string()),
            Some(PrintfCount::FromArg) => text.push('*'),
            None => {}
        }
        if let Some(precision) = conv.precision {
            text.push('.');
            match precision {
                PrintfCount::Literal(value) => text.push_str(&value.to_string()),
                PrintfCount::FromArg => text.push('*'),
            }
        }
        text.push_str(self.printf_length_text(conv.length));
        text.push(conv.spec);
        text
    }

    pub(super) fn host_printf_conversion_text(&self, conv: &PrintfConversion) -> String {
        let mut host = conv.clone();
        if matches!(host.spec, 'd' | 'i' | 'o' | 'u' | 'x' | 'X')
            && matches!(
                host.length,
                PrintfLength::L | PrintfLength::J | PrintfLength::Z | PrintfLength::T
            )
        {
            // These are all 64-bit in the interpreted data model. `long`,
            // size_t, and ptrdiff_t are only 32-bit in the Wasm host ABI.
            host.length = PrintfLength::Ll;
        } else if matches!(host.spec, 'f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A')
            && host.length == PrintfLength::BigL
        {
            // Interpreted long double is binary64, so pass a host double.
            host.length = PrintfLength::None;
        }
        self.printf_conversion_text(&host)
    }

    pub(super) fn printf_integer_type(
        &self,
        length: PrintfLength,
        signed: bool,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let ty = match (length, signed) {
            (PrintfLength::None, true) => CType::Int,
            (PrintfLength::None, false) => CType::UnsignedInt,
            (PrintfLength::H | PrintfLength::Hh, _) => CType::Int,
            (PrintfLength::L | PrintfLength::J | PrintfLength::T, true) => CType::Long,
            (PrintfLength::L | PrintfLength::J | PrintfLength::Z | PrintfLength::T, false) => {
                CType::UnsignedLong
            }
            (PrintfLength::Z, true) => CType::Long,
            (PrintfLength::Ll, true) => CType::LongLong,
            (PrintfLength::Ll, false) => CType::UnsignedLongLong,
            _ => {
                return Err(Diagnostic::error(
                    "unsupported printf length modifier",
                    span,
                ));
            }
        };
        Ok(ty)
    }

    pub(super) fn snprintf_bytes_with_call<F>(&self, call: F) -> Result<Vec<u8>, Diagnostic>
    where
        F: Fn(*mut c_char, usize) -> c_int,
    {
        let mut buffer = vec![0u8; 256];
        loop {
            let written = call(buffer.as_mut_ptr().cast::<c_char>(), buffer.len());
            if written < 0 {
                return Err(Diagnostic::error(
                    "host snprintf failed",
                    Span::new(FileId(0), 0, 0),
                ));
            }
            let written = written as usize;
            if written < buffer.len() {
                buffer.truncate(written);
                return Ok(buffer);
            }
            buffer.resize(written + 1, 0);
        }
    }

    fn is_pointer_to_character_type(&self, ty: &CType) -> bool {
        matches!(ty.unqualified(), CType::Pointer(inner) if inner.is_character())
    }

    fn is_pointer_to_wchar_type(&self, ty: &CType) -> bool {
        matches!(ty.unqualified(), CType::Pointer(inner)
            if inner.unqualified() == self.wchar_type().unqualified())
    }

    fn type_contains_character_array(ty: &CType) -> bool {
        match ty.unqualified() {
            CType::Array(inner, _) if inner.is_character() => true,
            CType::Array(inner, _) => Self::type_contains_character_array(inner),
            _ => false,
        }
    }

    fn pointer_designates_character_array(
        &self,
        pointer: &PointerValue,
        objects: &ObjectFrames,
    ) -> bool {
        let Some(root_ty) = self.pointer_root_type(pointer, objects) else {
            return false;
        };
        let Some(designated_ty) = self.storage_path_type(root_ty, &pointer.member_path) else {
            return false;
        };
        Self::type_contains_character_array(&designated_ty)
    }

    fn check_printf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_printf_like_call_with_standard(
            function_name,
            format_index,
            args,
            evaluated,
            span,
            objects,
            "7.19.6.1",
        )
    }

    pub(super) fn printf_n_pointee_type(
        &self,
        length: PrintfLength,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        match length {
            PrintfLength::None => Ok(CType::Int),
            PrintfLength::Hh => Ok(CType::SignedChar),
            PrintfLength::H => Ok(CType::Short),
            PrintfLength::L | PrintfLength::J | PrintfLength::Z | PrintfLength::T => {
                Ok(CType::Long)
            }
            PrintfLength::Ll => Ok(CType::LongLong),
            PrintfLength::BigL => Err(Diagnostic::error(
                "unsupported printf length modifier",
                span,
            )),
        }
    }

    pub(super) fn require_exact_call_args(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        expected: usize,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.len() != expected || evaluated.len() != expected {
            let expected = match expected {
                0 => "no arguments".to_owned(),
                1 => "exactly one argument".to_owned(),
                2 => "exactly two arguments".to_owned(),
                3 => "exactly three arguments".to_owned(),
                4 => "exactly four arguments".to_owned(),
                5 => "exactly five arguments".to_owned(),
                count => format!("exactly {count} arguments"),
            };
            return Err(Diagnostic::error(
                format!("{function_name} requires {expected}"),
                span,
            ));
        }
        Ok(())
    }

    fn require_min_call_args(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        minimum: usize,
        message: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.len() < minimum || evaluated.len() < minimum {
            return Err(Diagnostic::error(
                format!("{function_name} requires {message}"),
                span,
            ));
        }
        Ok(())
    }

    fn resolve_printf_count_arg<'b>(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        what: &str,
        count: Option<PrintfCount>,
        args: &[PrintfArgRef<'b>],
        arg_index: &mut usize,
        fallback_span: Span,
        standard: &'static str,
    ) -> Result<Option<i32>, Diagnostic> {
        match count {
            None => Ok(None),
            Some(PrintfCount::Literal(value)) => Ok(Some(value)),
            Some(PrintfCount::FromArg) => {
                let Some(arg) = args.get(*arg_index).copied() else {
                    return Err(Diagnostic::error(
                        format!(
                            "{function_name} is missing the {what} argument for {}",
                            self.printf_conversion_text(conversion)
                        ),
                        fallback_span,
                    ));
                };
                self.check_library_argument_value(arg.value, arg.span, function_name)?;
                if arg.value.ty != CType::Int {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {what} argument for {} must have type int",
                            self.printf_conversion_text(conversion)
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
                *arg_index += 1;
                Ok(Some(i32::try_from(arg.value.to_int()?).map_err(|_| {
                    Diagnostic::error(
                        format!("{function_name} {what} is out of supported range"),
                        arg.span,
                    )
                })?))
            }
        }
    }

    pub(super) fn resolve_printf_conversion_args<'b>(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        args: &[PrintfArgRef<'b>],
        arg_index: &mut usize,
        fallback_span: Span,
        standard: &'static str,
    ) -> Result<ResolvedPrintfArgs<'b>, Diagnostic> {
        let width = self.resolve_printf_count_arg(
            function_name,
            conversion,
            "field width",
            conversion.width,
            args,
            arg_index,
            fallback_span,
            standard,
        )?;
        let precision = self.resolve_printf_count_arg(
            function_name,
            conversion,
            "precision",
            conversion.precision,
            args,
            arg_index,
            fallback_span,
            standard,
        )?;
        let value = if conversion.spec == '%' {
            None
        } else {
            let Some(arg) = args.get(*arg_index).copied() else {
                return Err(Diagnostic::error(
                    format!(
                        "{function_name} is missing an argument for {}",
                        self.printf_conversion_text(conversion)
                    ),
                    fallback_span,
                ));
            };
            self.check_library_argument_value(arg.value, arg.span, function_name)?;
            *arg_index += 1;
            Some(arg)
        };
        Ok(ResolvedPrintfArgs {
            width,
            precision,
            value,
        })
    }

    fn validate_printf_n_destination(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        value: &TypedValue,
        span: Span,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let pointee = self.printf_n_pointee_type(conversion.length, span)?;
        let expected = CType::pointer_to(pointee.clone());
        if !self.pointer_assignment_compatible(&expected, &value.ty) {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} {} requires an argument of type {}",
                    self.printf_conversion_text(conversion),
                    expected
                ),
                span,
                Some(standard),
            ));
        }
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} {} requires a non-null output pointer",
                    self.printf_conversion_text(conversion)
                ),
                span,
                Some(standard),
            ));
        }
        let limit = self
            .object_pointer_limit(objects, &pointer, &pointee)
            .ok_or_else(|| {
                Diagnostic::ub(
                    format!(
                        "{function_name} {} destination does not designate a valid writable object",
                        self.printf_conversion_text(conversion)
                    ),
                    span,
                    Some(standard),
                )
            })?;
        if limit < 1 {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} {} destination does not designate a valid writable object",
                    self.printf_conversion_text(conversion)
                ),
                span,
                Some(standard),
            ));
        }
        let size = self
            .type_size_of(&pointee)
            .ok_or_else(|| Diagnostic::error("printf %n destination type has no size", span))?;
        let (object_id, start, _) = self.byte_region_from_pointer(&pointer, size, span, objects)?;
        self.ensure_library_writable_region(object_id, start, size, span, objects)
    }

    pub(super) fn validate_printf_conversion_arg(
        &mut self,
        function_name: &'static str,
        conversion: &PrintfConversion,
        precision: Option<i32>,
        value: Option<PrintfArgRef<'_>>,
        format_span: Span,
        standard: &'static str,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let conversion_text = self.printf_conversion_text(conversion);
        let effective_precision = precision.and_then(|value| usize::try_from(value).ok());
        match conversion.spec {
            '%' => {
                if conversion.length != PrintfLength::None {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} uses an invalid length modifier",
                            conversion_text
                        ),
                        format_span,
                        Some(standard),
                    ));
                }
            }
            'd' | 'i' | 'o' | 'u' | 'x' | 'X' => {
                let arg = value.expect("checked by caller");
                if conversion.length == PrintfLength::BigL {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} uses an invalid length modifier",
                            conversion_text
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
                let expected = self.printf_integer_type(
                    conversion.length,
                    matches!(conversion.spec, 'd' | 'i'),
                    arg.span,
                )?;
                if arg.value.ty != expected
                    && !self.corresponding_signedness_value_exception(&expected, arg.value)
                {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} requires an argument of type {}",
                            conversion_text, expected
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
            }
            'c' => {
                let arg = value.expect("checked by caller");
                if conversion.precision.is_some() {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} uses an invalid precision",
                            conversion_text
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
                match conversion.length {
                    PrintfLength::None => {
                        if arg.value.ty != CType::Int {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires an int argument after default argument promotions",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                    }
                    PrintfLength::L => {
                        if arg.value.ty != self.wchar_type() {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires an argument of type wchar_t",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                    }
                    _ => {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} {} uses an invalid length modifier",
                                conversion_text
                            ),
                            arg.span,
                            Some(standard),
                        ));
                    }
                }
            }
            's' => {
                let arg = value.expect("checked by caller");
                match conversion.length {
                    PrintfLength::None => {
                        if !self.is_pointer_to_character_type(&arg.value.ty) {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires a pointer to a character type",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                        let pointer = arg.value.as_pointer(arg.span)?;
                        if !self.pointer_designates_character_array(&pointer, objects) {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} argument does not point into an array of character type",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                        let _ = self.pointer_object_bytes(&pointer, arg.span, objects)?;
                        if let Some(limit) = effective_precision {
                            let _ = self
                                .read_bounded_c_string_source(pointer, limit, arg.span, objects)?;
                        } else {
                            let _ = self.read_c_string_bytes(pointer, arg.span, objects)?;
                        }
                    }
                    PrintfLength::L => {
                        if !self.is_pointer_to_wchar_type(&arg.value.ty) {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires an argument of type wchar_t *",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                        let pointer = arg.value.as_pointer(arg.span)?;
                        let _ = self.pointer_object_bytes(&pointer, arg.span, objects)?;
                        if let Some(limit) = effective_precision {
                            if limit == 0 {
                                self.pointer_byte_offset(&pointer, &self.wchar_type(), objects)
                                    .ok_or_else(|| {
                                        Diagnostic::ub(
                                            "wide string pointer does not designate a valid wchar_t region",
                                            arg.span,
                                            Some("7.1.4"),
                                        )
                                    })?;
                            } else {
                                let _ = self
                                    .read_bounded_wide_source(pointer, limit, arg.span, objects)?;
                            }
                        } else {
                            let _ = self.read_wide_string_units(pointer, arg.span, objects)?;
                        }
                    }
                    _ => {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} {} uses an invalid length modifier",
                                conversion_text
                            ),
                            arg.span,
                            Some(standard),
                        ));
                    }
                }
            }
            'p' => {
                let arg = value.expect("checked by caller");
                if conversion.length != PrintfLength::None {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} uses an invalid length modifier",
                            conversion_text
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
                if arg.value.ty != self.void_ptr_type() {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} requires an argument of type void *",
                            conversion_text
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
            }
            'n' => {
                let arg = value.expect("checked by caller");
                if !conversion.flags.is_empty()
                    || conversion.width.is_some()
                    || conversion.precision.is_some()
                {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} {} uses invalid flags, width, or precision",
                            conversion_text
                        ),
                        arg.span,
                        Some(standard),
                    ));
                }
                self.validate_printf_n_destination(
                    function_name,
                    conversion,
                    arg.value,
                    arg.span,
                    objects,
                    standard,
                )?;
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A' => {
                let arg = value.expect("checked by caller");
                match conversion.length {
                    PrintfLength::BigL => {
                        if arg.value.ty != CType::LongDouble {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires an argument of type long double",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                    }
                    PrintfLength::None | PrintfLength::L => {
                        if arg.value.ty != CType::Double {
                            return Err(Diagnostic::ub(
                                format!(
                                    "{function_name} {} requires an argument of type double",
                                    conversion_text
                                ),
                                arg.span,
                                Some(standard),
                            ));
                        }
                    }
                    _ => {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} {} uses an invalid length modifier",
                                conversion_text
                            ),
                            arg.span,
                            Some(standard),
                        ));
                    }
                }
            }
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported {function_name} conversion {}", conversion_text),
                    format_span,
                ));
            }
        }
        Ok(())
    }

    fn check_printf_like_call_with_standard(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        if args.len() <= format_index || evaluated.len() <= format_index {
            return Err(Diagnostic::error(
                format!("{function_name} requires a format argument"),
                span,
            ));
        }
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.reject_missing_return_value(value, expr.span())?;
        }
        let format_value = evaluated[format_index].clone();
        let format_span = args[format_index].span();
        self.reject_indeterminate_library_value(&format_value, format_span, function_name)?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const char *"),
                format_span,
                Some(standard),
            ));
        }
        let format =
            self.read_c_string_bytes(format_value.as_pointer(format_span)?, format_span, objects)?;
        let variadic_args = args
            .iter()
            .zip(evaluated.iter())
            .skip(format_index + 1)
            .map(|(expr, value)| PrintfArgRef {
                value,
                span: expr.span(),
            })
            .collect::<Vec<_>>();
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != b'%' {
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
                precision, value, ..
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                standard,
            )?;
            self.validate_printf_conversion_arg(
                function_name,
                &conversion,
                precision,
                value,
                format_span,
                standard,
                objects,
            )?;
        }
        Ok(())
    }

    pub(super) fn check_printf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_printf_like_call("printf", 0, args, evaluated, span, objects)
    }

    pub(super) fn check_fprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if args.len() < 2 || evaluated.len() < 2 {
            return Err(Diagnostic::error(
                "fprintf requires a stream and format argument",
                span,
            ));
        }
        self.check_stream_call("fprintf", &args[..1], &evaluated[..1], span, "7.19.6.1")?;
        self.check_printf_like_call("fprintf", 1, args, evaluated, span, objects)
    }

    pub(super) fn check_sprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if args.len() < 2 || evaluated.len() < 2 {
            return Err(Diagnostic::error(
                "sprintf requires a destination and format argument",
                span,
            ));
        }
        self.check_library_argument_value(&evaluated[0], args[0].span(), "sprintf")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "sprintf requires a char * destination",
                args[0].span(),
            ));
        }
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&destination, 1, 1, args[0].span(), objects)?;
        self.check_printf_like_call("sprintf", 1, args, evaluated, span, objects)
    }

    pub(super) fn check_snprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            "snprintf",
            args,
            evaluated,
            3,
            "destination, size, and format arguments",
            span,
        )?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "snprintf")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "snprintf")?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        if !dest_pointer.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "snprintf requires a char * destination or a null pointer when n is 0",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "snprintf size argument must have type unsigned long",
                args[1].span(),
            ));
        }
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "snprintf size")?;
        if dest_pointer.is_null() {
            if bound != 0 {
                return Err(Diagnostic::ub(
                    "snprintf requires a valid destination pointer when n is nonzero",
                    args[0].span(),
                    Some("7.21.6.5"),
                ));
            }
        } else {
            let _ = self.ensure_library_array_destination(
                &dest_pointer,
                bound,
                1,
                args[0].span(),
                objects,
            )?;
        }
        self.check_printf_like_call("snprintf", 2, args, evaluated, span, objects)
    }

    pub(super) fn check_vsprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if args.len() != 3 || evaluated.len() != 3 {
            return Err(Diagnostic::error(
                "vsprintf requires destination, format, and va_list arguments",
                span,
            ));
        }
        self.check_library_argument_value(&evaluated[0], args[0].span(), "vsprintf")?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "vsprintf requires a char * destination",
                args[0].span(),
            ));
        }
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&destination, 1, 1, args[0].span(), objects)?;
        self.check_vprintf_like_call("vsprintf", 1, 2, args, evaluated, span, objects)
    }

    pub(super) fn check_vsnprintf_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if args.len() != 4 || evaluated.len() != 4 {
            return Err(Diagnostic::error(
                "vsnprintf requires destination, size, format, and va_list arguments",
                span,
            ));
        }
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "vsnprintf")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "vsnprintf")?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        if !destination.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "vsnprintf requires a char * destination or a null pointer when n is 0",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "vsnprintf size argument must have type unsigned long",
                args[1].span(),
            ));
        }
        let bound =
            self.checked_usize_from_unsigned_long(&evaluated[1], args[1].span(), "vsnprintf size")?;
        if destination.is_null() {
            if bound != 0 {
                return Err(Diagnostic::ub(
                    "vsnprintf requires a valid destination pointer when n is nonzero",
                    args[0].span(),
                    Some("7.21.6.12"),
                ));
            }
        } else {
            let _ = self.ensure_library_array_destination(
                &destination,
                bound,
                1,
                args[0].span(),
                objects,
            )?;
        }
        self.check_vprintf_like_call("vsnprintf", 2, 3, args, evaluated, span, objects)
    }

    pub(super) fn check_bounded_wide_printf_destination(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), function_name)?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), function_name)?;
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wchar_t * destination"),
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} size argument must have type unsigned long"),
                args[1].span(),
            ));
        }
        let bound = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            &format!("{function_name} size"),
        )?;
        let total = bound
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error(
                    format!("{function_name} destination size is out of supported range"),
                    span,
                )
            })?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ = self.ensure_library_array_destination(
            &destination,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(())
    }

    pub(super) fn check_vprintf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        va_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            va_index + 1,
            "a format and va_list argument",
            span,
        )?;
        for index in 0..=format_index {
            self.reject_missing_return_value(&evaluated[index], args[index].span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        let const_char_ptr = CType::pointer_to(CType::qualified(
            CType::Char,
            crate::types::TypeQualifiers {
                is_atomic: false,
                is_const: true,
                is_restrict: false,
                is_volatile: false,
            },
        ));
        if !self.pointer_assignment_compatible(&const_char_ptr, &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const char *"),
                args[format_index].span(),
                Some("7.19.6.1"),
            ));
        }
        let _ = self.read_c_string(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        self.check_library_argument_value(
            &evaluated[va_index],
            args[va_index].span(),
            function_name,
        )?;
        if evaluated[va_index].ty != CType::VaList {
            return Err(Diagnostic::error(
                format!("{function_name} requires a va_list argument"),
                args[va_index].span(),
            ));
        }
        let handle = u64::try_from(evaluated[va_index].to_int()?).unwrap_or(0);
        if handle == 0 || !self.va_lists.contains_key(&handle) {
            return Err(Diagnostic::ub(
                format!("{function_name} used with an invalid va_list"),
                args[va_index].span(),
                Some("7.15.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_wprintf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            format_index + 1,
            "a format argument",
            span,
        )?;
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.reject_missing_return_value(value, expr.span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const wchar_t *"),
                args[format_index].span(),
                Some("7.24.2.1"),
            ));
        }
        let format = self.read_wide_string_units(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
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
        let mut arg_index = 0usize;
        let mut index = 0usize;
        while index < format.len() {
            if format[index] != '%' as libc::wchar_t {
                index += 1;
                continue;
            }
            index += 1;
            let conversion = self.parse_printf_conversion_wide(
                function_name,
                &format,
                &mut index,
                args[format_index].span(),
            )?;
            let ResolvedPrintfArgs {
                precision, value, ..
            } = self.resolve_printf_conversion_args(
                function_name,
                &conversion,
                &variadic_args,
                &mut arg_index,
                span,
                "7.24.2.1",
            )?;
            self.validate_printf_conversion_arg(
                function_name,
                &conversion,
                precision,
                value,
                args[format_index].span(),
                "7.24.2.1",
                objects,
            )?;
        }
        Ok(())
    }

    pub(super) fn check_vwprintf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        va_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            va_index + 1,
            "a format and va_list argument",
            span,
        )?;
        for index in 0..=format_index {
            self.reject_missing_return_value(&evaluated[index], args[index].span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const wchar_t *"),
                args[format_index].span(),
                Some("7.24.2.1"),
            ));
        }
        let _ = self.read_wide_string_units(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        self.check_library_argument_value(
            &evaluated[va_index],
            args[va_index].span(),
            function_name,
        )?;
        if evaluated[va_index].ty != CType::VaList {
            return Err(Diagnostic::error(
                format!("{function_name} requires a va_list argument"),
                args[va_index].span(),
            ));
        }
        let handle = u64::try_from(evaluated[va_index].to_int()?).unwrap_or(0);
        if handle == 0 || !self.va_lists.contains_key(&handle) {
            return Err(Diagnostic::ub(
                format!("{function_name} used with an invalid va_list"),
                args[va_index].span(),
                Some("7.15.1"),
            ));
        }
        Ok(())
    }

    fn check_wscanf_target(
        &mut self,
        function_name: &'static str,
        conv: &ScanfConversion,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        match conv.spec {
            'd' | 'i' | 'o' | 'u' | 'x' | 'X' | 'n' => {
                let pointee = self.scanf_integer_pointee_type(conv, span)?;
                let pointer = self.wide_scan_output_pointer(
                    function_name,
                    value,
                    &CType::pointer_to(pointee.clone()),
                    span,
                )?;
                let limit = self.wide_scan_capacity(&pointer, &pointee, span, objects)?;
                if limit == 0 {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} destination is not large enough for the conversion result"
                        ),
                        span,
                        Some("7.24.2.2"),
                    ));
                }
            }
            'p' => {
                let pointer = self.wide_scan_output_pointer(
                    function_name,
                    value,
                    &CType::pointer_to(self.void_ptr_type()),
                    span,
                )?;
                let limit =
                    self.wide_scan_capacity(&pointer, &self.void_ptr_type(), span, objects)?;
                if limit == 0 {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} destination is not large enough for the conversion result"
                        ),
                        span,
                        Some("7.24.2.2"),
                    ));
                }
            }
            'a' | 'A' | 'e' | 'E' | 'f' | 'F' | 'g' | 'G' => {
                let pointee = self.scanf_float_pointee_type(conv, span)?;
                let pointer = self.wide_scan_output_pointer(
                    function_name,
                    value,
                    &CType::pointer_to(pointee.clone()),
                    span,
                )?;
                let limit = self.wide_scan_capacity(&pointer, &pointee, span, objects)?;
                if limit == 0 {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} destination is not large enough for the conversion result"
                        ),
                        span,
                        Some("7.24.2.2"),
                    ));
                }
            }
            'c' | 's' | '[' => match conv.length {
                ScanfLength::None => {
                    let (pointer, element_ty) =
                        self.scan_character_output_pointer(function_name, value, span)?;
                    let limit = self.wide_scan_capacity(&pointer, &element_ty, span, objects)?;
                    let needed = match conv.spec {
                        'c' => conv.width.unwrap_or(1),
                        _ => conv.width.unwrap_or(0).saturating_add(1),
                    };
                    if conv.width.is_some() && limit < needed {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} destination array is too small for the requested field width"
                            ),
                            span,
                            Some("7.24.2.2"),
                        ));
                    }
                }
                ScanfLength::L => {
                    let pointer = self.wide_scan_output_pointer(
                        function_name,
                        value,
                        &self.wchar_ptr_type(),
                        span,
                    )?;
                    let limit =
                        self.wide_scan_capacity(&pointer, &self.wchar_type(), span, objects)?;
                    let needed = match conv.spec {
                        'c' => conv.width.unwrap_or(1),
                        _ => conv.width.unwrap_or(0).saturating_add(1),
                    };
                    if conv.width.is_some() && limit < needed {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} destination array is too small for the requested field width"
                            ),
                            span,
                            Some("7.24.2.2"),
                        ));
                    }
                }
                _ => {
                    return Err(Diagnostic::error(
                        format!(
                            "{function_name} conversion %{spec} uses an unsupported length modifier",
                            spec = conv.spec
                        ),
                        span,
                    ));
                }
            },
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported {function_name} conversion %{}", conv.spec),
                    span,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check_wscanf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            format_index + 1,
            "a format argument",
            span,
        )?;
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.reject_missing_return_value(value, expr.span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const wchar_t *"),
                args[format_index].span(),
                Some("7.24.2.2"),
            ));
        }
        let format_units = self.read_wide_string_units(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        let directives =
            self.parse_wide_scanf_format(function_name, &format_units, args[format_index].span())?;
        let mut arg_index = format_index + 1;
        for directive in directives {
            let ScanfDirective::Conversion(conv) = directive else {
                continue;
            };
            if conv.suppress {
                continue;
            }
            let Some(expr) = args.get(arg_index) else {
                return Err(Diagnostic::error(
                    format!("{function_name} is missing an output argument"),
                    span,
                ));
            };
            let value = evaluated[arg_index].clone();
            self.reject_indeterminate_library_value(&value, expr.span(), function_name)?;
            self.check_wscanf_target(function_name, &conv, &value, expr.span(), objects)?;
            arg_index += 1;
        }
        Ok(())
    }

    pub(super) fn check_vwscanf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        va_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            va_index + 1,
            "a format and va_list argument",
            span,
        )?;
        for index in 0..=format_index {
            self.reject_missing_return_value(&evaluated[index], args[index].span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const wchar_t *"),
                args[format_index].span(),
                Some("7.24.2.2"),
            ));
        }
        let _ = self.read_wide_string_units(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        self.check_library_argument_value(
            &evaluated[va_index],
            args[va_index].span(),
            function_name,
        )?;
        if evaluated[va_index].ty != CType::VaList {
            return Err(Diagnostic::error(
                format!("{function_name} requires a va_list argument"),
                args[va_index].span(),
            ));
        }
        let handle = u64::try_from(evaluated[va_index].to_int()?).unwrap_or(0);
        if handle == 0 || !self.va_lists.contains_key(&handle) {
            return Err(Diagnostic::ub(
                format!("{function_name} used with an invalid va_list"),
                args[va_index].span(),
                Some("7.15.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_scanf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            format_index + 1,
            "a format argument",
            span,
        )?;
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.reject_missing_return_value(value, expr.span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const char *"),
                args[format_index].span(),
                Some("7.19.6.2"),
            ));
        }
        let format_bytes = self.read_c_string_bytes(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        let directives =
            self.parse_scanf_format(function_name, &format_bytes, args[format_index].span())?;
        let mut arg_index = format_index + 1;
        for directive in directives {
            let ScanfDirective::Conversion(conv) = directive else {
                continue;
            };
            if conv.suppress {
                continue;
            }
            let Some(expr) = args.get(arg_index) else {
                return Err(Diagnostic::error(
                    format!("{function_name} is missing an output argument"),
                    span,
                ));
            };
            let value = evaluated[arg_index].clone();
            self.reject_indeterminate_library_value(&value, expr.span(), function_name)?;
            self.check_wscanf_target(function_name, &conv, &value, expr.span(), objects)?;
            arg_index += 1;
        }
        Ok(())
    }

    pub(super) fn check_vscanf_like_call(
        &mut self,
        function_name: &'static str,
        format_index: usize,
        va_index: usize,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_min_call_args(
            function_name,
            args,
            evaluated,
            va_index + 1,
            "a format and va_list argument",
            span,
        )?;
        for index in 0..=format_index {
            self.reject_missing_return_value(&evaluated[index], args[index].span())?;
        }
        let format_value = evaluated[format_index].clone();
        self.reject_indeterminate_library_value(
            &format_value,
            args[format_index].span(),
            function_name,
        )?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &format_value.ty) {
            return Err(Diagnostic::ub(
                format!("{function_name} format argument must have type const char *"),
                args[format_index].span(),
                Some("7.19.6.8"),
            ));
        }
        let _ = self.read_c_string_bytes(
            format_value.as_pointer(args[format_index].span())?,
            args[format_index].span(),
            objects,
        )?;
        self.check_library_argument_value(
            &evaluated[va_index],
            args[va_index].span(),
            function_name,
        )?;
        if evaluated[va_index].ty != CType::VaList {
            return Err(Diagnostic::error(
                format!("{function_name} requires a va_list argument"),
                args[va_index].span(),
            ));
        }
        let handle = u64::try_from(evaluated[va_index].to_int()?).unwrap_or(0);
        if handle == 0 || !self.va_lists.contains_key(&handle) {
            return Err(Diagnostic::ub(
                format!("{function_name} used with an invalid va_list"),
                args[va_index].span(),
                Some("7.15.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn check_fputwc_like_call(
        &mut self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
        if evaluated[0].ty != self.wchar_type() {
            return Err(Diagnostic::error(
                format!("{function_name} character argument must have type wchar_t"),
                args[0].span(),
            ));
        }
        self.check_stream_call(function_name, &args[1..], &evaluated[1..], span, standard)
    }

    pub(super) fn check_fputws_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fputws", args, evaluated, 2, span)?;
        self.check_wide_string_arg("fputws", &evaluated[0], args[0].span(), objects)?;
        self.check_stream_call("fputws", &args[1..], &evaluated[1..], span, "7.24.3.6")
    }

    pub(super) fn check_fgetws_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fgetws", args, evaluated, 3, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "fgetws")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "fgetws")?;
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "fgetws requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "fgetws count argument must have type int",
                args[1].span(),
            ));
        }
        let count = evaluated[1].to_int()?;
        if count <= 0 {
            return Err(Diagnostic::ub(
                "fgetws count must be positive",
                args[1].span(),
                Some("7.29.3.2"),
            ));
        }
        let total = usize::try_from(count)
            .ok()
            .and_then(|count| count.checked_mul(self.wide_char_byte_width()))
            .ok_or_else(|| {
                Diagnostic::error("fgetws destination size is out of supported range", span)
            })?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ = self.ensure_library_array_destination(
            &destination,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        self.check_stream_call("fgetws", &args[2..], &evaluated[2..], span, "7.24.3.2")
    }

    pub(super) fn check_fwide_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fwide", args, evaluated, 2, span)?;
        self.check_stream_call("fwide", &args[..1], &evaluated[..1], span, "7.24.3.5")?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "fwide")?;
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "fwide mode argument must have type int",
                args[1].span(),
            ));
        }
        Ok(())
    }

    pub(super) fn check_ungetwc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("ungetwc", args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "ungetwc")?;
        if evaluated[0].ty != self.wchar_type() {
            return Err(Diagnostic::error(
                "ungetwc character argument must have type wchar_t",
                args[0].span(),
            ));
        }
        self.check_stream_call("ungetwc", &args[1..], &evaluated[1..], span, "7.24.3.11")
    }

    pub(super) fn check_wcsftime_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcsftime", args, evaluated, 4, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "wcsftime")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "wcsftime")?;
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcsftime requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcsftime maxsize argument must have type unsigned long",
                args[1].span(),
            ));
        }
        let maxsize = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "wcsftime maxsize",
        )?;
        let total = maxsize
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcsftime size is out of supported range", span))?;
        let destination = evaluated[0].as_pointer(args[0].span())?;
        let _ = self.ensure_library_array_destination(
            &destination,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        self.check_wide_string_arg("wcsftime", &evaluated[2], args[2].span(), objects)?;
        let format = self.read_wide_string_units(
            evaluated[2].as_pointer(args[2].span())?,
            args[2].span(),
            objects,
        )?;
        let format = format
            .into_iter()
            .map(|unit| unit as u32)
            .collect::<Vec<_>>();
        self.validate_strftime_conversion_specifiers("wcsftime", &format, args[2].span())?;
        let _ = self.check_tm_pointer_input("wcsftime", &evaluated[3], args[3].span(), objects)?;
        Ok(())
    }

    pub(super) fn check_wcstok_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcstok", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcstok")?;
        }
        let source = evaluated[0].as_pointer(args[0].span())?;
        if !source.is_null()
            && !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "wcstok requires a wchar_t * source or null",
                args[0].span(),
            ));
        }
        self.check_wide_string_arg("wcstok", &evaluated[1], args[1].span(), objects)?;
        if !self.pointer_assignment_compatible(
            &CType::pointer_to(self.wchar_ptr_type()),
            &evaluated[2].ty,
        ) {
            return Err(Diagnostic::error(
                "wcstok requires a wchar_t ** state pointer",
                args[2].span(),
            ));
        }
        let state_pointer = evaluated[2].as_pointer(args[2].span())?;
        let pointer_size = self.type_size_of(&self.wchar_ptr_type()).unwrap_or(8);
        let (state_object, state_start, _) =
            self.byte_region_from_pointer(&state_pointer, pointer_size, args[2].span(), objects)?;
        self.ensure_library_writable_region(
            state_object,
            state_start,
            pointer_size,
            args[2].span(),
            objects,
        )?;

        let active_source = if source.is_null() {
            let state_lvalue = self.pointer_lvalue(
                "wcstok",
                &state_pointer,
                &self.wchar_ptr_type(),
                args[2].span(),
                "7.29.4.5.7",
            )?;
            let saved = self
                .load_lvalue(state_lvalue, args[2].span(), objects)?
                .as_pointer(args[2].span())?;
            let version = self
                .lookup_object(objects, state_object)
                .expect("validated wcstok state object")
                .modification_count;
            let valid = self
                .wcstok_state_provenance
                .get(&(state_object, state_start))
                .is_some_and(|(saved_version, expected)| {
                    *saved_version == version && *expected == saved
                });
            if !valid {
                return Err(Diagnostic::ub(
                    "wcstok null source requires an unmodified state value stored by a previous call for the same wide string",
                    args[0].span(),
                    Some("7.29.4.5.7"),
                ));
            }
            saved
        } else {
            source
        };
        if !active_source.is_null() {
            let units =
                self.read_wide_string_units(active_source.clone(), args[0].span(), objects)?;
            let size = (units.len() + 1)
                .checked_mul(self.wide_char_byte_width())
                .ok_or_else(|| Diagnostic::error("wcstok source size is out of range", span))?;
            let _ = self.ensure_library_array_destination(
                &active_source,
                size,
                self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?;
        }
        Ok(())
    }

    pub(super) fn validate_strftime_conversion_specifiers(
        &self,
        function_name: &str,
        format: &[u32],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut index = 0;
        while index < format.len() {
            if format[index] != u32::from(b'%') {
                index += 1;
                continue;
            }
            index += 1;
            let Some(&next) = format.get(index) else {
                return Err(Diagnostic::ub(
                    format!("{function_name} format ends with an incomplete conversion specifier"),
                    span,
                    Some("7.27.3.5"),
                ));
            };
            let modifier = char::from_u32(next).filter(|ch| matches!(ch, 'E' | 'O'));
            if modifier.is_some() {
                index += 1;
            }
            let Some(specifier) = format.get(index).and_then(|&unit| char::from_u32(unit)) else {
                return Err(Diagnostic::ub(
                    format!("{function_name} format contains an invalid conversion specifier"),
                    span,
                    Some("7.27.3.5"),
                ));
            };
            let valid = match modifier {
                Some('E') => matches!(specifier, 'c' | 'C' | 'x' | 'X' | 'y' | 'Y'),
                Some('O') => matches!(
                    specifier,
                    'd' | 'e' | 'H' | 'I' | 'm' | 'M' | 'S' | 'u' | 'U' | 'V' | 'w' | 'W' | 'y'
                ),
                Some(_) => unreachable!(),
                None => "aAbBcCdDeFgGhHIjmMnprRStTuUVwWxXyYzZ%".contains(specifier),
            };
            if !valid {
                return Err(Diagnostic::ub(
                    format!("{function_name} format contains an invalid conversion specifier"),
                    span,
                    Some("7.27.3.5"),
                ));
            }
            index += 1;
        }
        Ok(())
    }

    pub(super) fn check_math_common_args(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
    ) -> Result<(), Diagnostic> {
        if args.len() != evaluated.len() {
            return Err(Diagnostic::error(
                format!(
                    "{function_name} evaluated {} argument(s) for {} expression(s)",
                    evaluated.len(),
                    args.len()
                ),
                args.first()
                    .map(|expr| expr.span())
                    .unwrap_or_else(|| Span::new(FileId(0), 0, 0)),
            ));
        }
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.reject_missing_return_value(value, expr.span())?;
            if value.indeterminate {
                return Err(Diagnostic::ub(
                    format!("{function_name} argument has an indeterminate value"),
                    expr.span(),
                    Some("7.1.4, WG14 DR 451"),
                )
                .with_note(
                    "library functions exhibit undefined behavior when used on indeterminate values",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check_math_real_arg(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !value.ty.is_floating() {
            return Err(Diagnostic::error(
                format!("{function_name} requires a real floating argument"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn check_math_integer_arg(
        &self,
        function_name: &str,
        value: &TypedValue,
        expected: CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.ty != expected {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type {expected}"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn check_math_pointer_output_arg(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a writable result pointer"),
                span,
                Some("7.1.4"),
            ));
        }
        let Some(pointee_ty) = value.ty.element_type().cloned() else {
            return Err(Diagnostic::error(
                format!("{function_name} requires a pointer result argument"),
                span,
            ));
        };
        let size = self.type_size_of(&pointee_ty).ok_or_else(|| {
            Diagnostic::error(
                format!("{function_name} requires a pointer to a complete object type"),
                span,
            )
        })?;
        let (object_id, start, _) = self.byte_region_from_pointer(&pointer, size, span, objects)?;
        self.ensure_library_writable_region(object_id, start, size, span, objects)
    }

    pub(super) fn check_math_string_arg(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let pointer = value.as_pointer(span)?;
        let _ = self.read_c_string_bytes(pointer, span, objects)?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type const char *"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn check_c_string_arg(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let pointer = value.as_pointer(span)?;
        let _ = self.read_c_string_bytes(pointer, span, objects)?;
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type const char *"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn check_wide_string_arg(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let pointer = value.as_pointer(span)?;
        let _ = self.read_wide_string_units(pointer, span, objects)?;
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type const wchar_t *"),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn parse_wide_scanf_format(
        &self,
        function_name: &str,
        units: &[libc::wchar_t],
        span: Span,
    ) -> Result<Vec<ScanfDirective>, Diagnostic> {
        let mut directives = Vec::new();
        let mut index = 0usize;
        while index < units.len() {
            let unit = units[index];
            if unsafe { iswspace(unit as c_int) } != 0 {
                while index < units.len() && unsafe { iswspace(units[index] as c_int) } != 0 {
                    index += 1;
                }
                directives.push(ScanfDirective::Whitespace);
                continue;
            }
            if unit != '%' as libc::wchar_t {
                directives.push(ScanfDirective::Literal(unit));
                index += 1;
                continue;
            }
            index += 1;
            if index < units.len() && units[index] == '%' as libc::wchar_t {
                // %% is a conversion specification, so it skips leading input
                // whitespace. Ordinary literal characters must not do so.
                directives.push(ScanfDirective::Percent);
                index += 1;
                continue;
            }

            let suppress = if index < units.len() && units[index] == '*' as libc::wchar_t {
                index += 1;
                true
            } else {
                false
            };

            let width_start = index;
            while index < units.len()
                && (units[index] as libc::wchar_t) >= ('0' as libc::wchar_t)
                && (units[index] as libc::wchar_t) <= ('9' as libc::wchar_t)
            {
                index += 1;
            }
            let width = if index > width_start {
                let text = units[width_start..index]
                    .iter()
                    .map(|unit| char::from_u32(*unit as u32).unwrap())
                    .collect::<String>();
                Some(text.parse::<usize>().map_err(|_| {
                    Diagnostic::error(
                        format!("{function_name} field width is out of supported range"),
                        span,
                    )
                })?)
            } else {
                None
            };

            let length = if index + 1 < units.len()
                && units[index] == 'h' as libc::wchar_t
                && units[index + 1] == 'h' as libc::wchar_t
            {
                index += 2;
                ScanfLength::Hh
            } else if index < units.len() && units[index] == 'h' as libc::wchar_t {
                index += 1;
                ScanfLength::H
            } else if index + 1 < units.len()
                && units[index] == 'l' as libc::wchar_t
                && units[index + 1] == 'l' as libc::wchar_t
            {
                index += 2;
                ScanfLength::Ll
            } else if index < units.len() && units[index] == 'l' as libc::wchar_t {
                index += 1;
                ScanfLength::L
            } else if index < units.len() && units[index] == 'j' as libc::wchar_t {
                index += 1;
                ScanfLength::J
            } else if index < units.len() && units[index] == 'z' as libc::wchar_t {
                index += 1;
                ScanfLength::Z
            } else if index < units.len() && units[index] == 't' as libc::wchar_t {
                index += 1;
                ScanfLength::T
            } else if index < units.len() && units[index] == 'L' as libc::wchar_t {
                index += 1;
                ScanfLength::BigL
            } else {
                ScanfLength::None
            };

            let spec = char::from_u32(units.get(index).copied().ok_or_else(|| {
                Diagnostic::error(format!("incomplete {function_name} format specifier"), span)
            })? as u32)
            .ok_or_else(|| {
                Diagnostic::error(
                    format!("unsupported non-ASCII {function_name} format specifier"),
                    span,
                )
            })?;
            index += 1;

            let scanset = if spec == '[' {
                let mut invert = false;
                if index < units.len() && units[index] == '^' as libc::wchar_t {
                    invert = true;
                    index += 1;
                }
                let mut items = Vec::new();
                if index < units.len() && units[index] == ']' as libc::wchar_t {
                    items.push(ScansetItem::Single(']' as libc::wchar_t));
                    index += 1;
                }
                while index < units.len() && units[index] != ']' as libc::wchar_t {
                    let first = units[index];
                    if index + 2 < units.len()
                        && units[index + 1] == '-' as libc::wchar_t
                        && units[index + 2] != ']' as libc::wchar_t
                    {
                        items.push(ScansetItem::Range(first, units[index + 2]));
                        index += 3;
                    } else {
                        items.push(ScansetItem::Single(first));
                        index += 1;
                    }
                }
                if index >= units.len() || units[index] != ']' as libc::wchar_t {
                    return Err(Diagnostic::error(
                        format!("unterminated {function_name} scanset"),
                        span,
                    ));
                }
                index += 1;
                Some(Scanset { invert, items })
            } else {
                None
            };

            directives.push(ScanfDirective::Conversion(ScanfConversion {
                suppress,
                width,
                length,
                spec,
                scanset,
            }));
        }
        Ok(directives)
    }

    pub(super) fn parse_scanf_format(
        &self,
        function_name: &str,
        bytes: &[u8],
        span: Span,
    ) -> Result<Vec<ScanfDirective>, Diagnostic> {
        let units = bytes
            .iter()
            .map(|byte| libc::wchar_t::from(*byte))
            .collect::<Vec<_>>();
        self.parse_wide_scanf_format(function_name, &units, span)
    }

    pub(super) fn scanset_matches(&self, scanset: &Scanset, unit: libc::wchar_t) -> bool {
        let matched = scanset.items.iter().any(|item| match item {
            ScansetItem::Single(value) => *value == unit,
            ScansetItem::Range(start, end) => *start <= unit && unit <= *end,
        });
        if scanset.invert { !matched } else { matched }
    }

    pub(super) fn wide_scan_source_from_stream(
        &self,
        stream_object: ObjectId,
        function_name: &'static str,
        standard: &'static str,
    ) -> WideScanfSource {
        WideScanfSource::Stream {
            stream_object,
            function_name,
            standard,
            pushback: Vec::new(),
            consumed: 0,
        }
    }

    pub(super) fn wide_scan_source_from_units(&self, units: Vec<libc::wchar_t>) -> WideScanfSource {
        WideScanfSource::Buffer {
            units,
            index: 0,
            consumed: 0,
        }
    }

    fn wide_scan_consumed(source: &WideScanfSource) -> usize {
        match source {
            WideScanfSource::Stream { consumed, .. } | WideScanfSource::Buffer { consumed, .. } => {
                *consumed
            }
        }
    }

    fn wide_scan_next(
        &mut self,
        source: &mut WideScanfSource,
        span: Span,
    ) -> Result<Option<libc::wchar_t>, Diagnostic> {
        match source {
            WideScanfSource::Buffer {
                units,
                index,
                consumed,
            } => {
                let Some(unit) = units.get(*index).copied() else {
                    return Ok(None);
                };
                *index += 1;
                *consumed = consumed.saturating_add(1);
                Ok(Some(unit))
            }
            WideScanfSource::Stream {
                stream_object,
                function_name,
                standard,
                pushback,
                consumed,
            } => {
                if let Some(unit) = pushback.pop() {
                    *consumed = consumed.saturating_add(1);
                    return Ok(Some(unit));
                }
                if let Some(ch) =
                    self.read_wide_unit_from_stream(*stream_object, span, function_name, standard)?
                {
                    *consumed = consumed.saturating_add(1);
                    Ok(Some(ch))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn wide_scan_unread(&self, source: &mut WideScanfSource, unit: libc::wchar_t) {
        match source {
            WideScanfSource::Buffer {
                index, consumed, ..
            } => {
                *index = index.saturating_sub(1);
                *consumed = consumed.saturating_sub(1);
            }
            WideScanfSource::Stream {
                pushback, consumed, ..
            } => {
                pushback.push(unit);
                *consumed = consumed.saturating_sub(1);
            }
        }
    }

    pub(super) fn finalize_wide_scan_source(
        &mut self,
        source: &mut WideScanfSource,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let WideScanfSource::Stream {
            stream_object,
            pushback,
            ..
        } = source
        else {
            return Ok(());
        };
        if pushback.is_empty() {
            return Ok(());
        }
        for &unit in pushback.iter() {
            let scalar = char::from_u32(unit as u32).ok_or_else(|| {
                Diagnostic::error("failed to restore unread wide input to stream", span)
            })?;
            let mut encoded = [0u8; 4];
            for &byte in scalar.encode_utf8(&mut encoded).as_bytes().iter().rev() {
                self.unread_byte_to_stream(*stream_object, byte, span, "fwscanf", "7.24.2.2")?;
            }
        }
        if let Some(stream) = self.host_streams.get_mut(stream_object) {
            stream.eof = false;
            stream.error = false;
        }
        pushback.clear();
        Ok(())
    }

    fn wide_scan_skip_ws(
        &mut self,
        source: &mut WideScanfSource,
        span: Span,
    ) -> Result<(), Diagnostic> {
        loop {
            let Some(unit) = self.wide_scan_next(source, span)? else {
                return Ok(());
            };
            if unsafe { iswspace(unit as c_int) } == 0 {
                self.wide_scan_unread(source, unit);
                return Ok(());
            }
        }
    }

    pub(super) fn scan_source_from_stream(
        &self,
        stream_object: ObjectId,
        function_name: &'static str,
        standard: &'static str,
    ) -> ByteScanfSource {
        ByteScanfSource::Stream {
            stream_object,
            function_name,
            standard,
            pushback: Vec::new(),
            consumed: 0,
        }
    }

    pub(super) fn scan_source_from_bytes(&self, bytes: Vec<u8>) -> ByteScanfSource {
        ByteScanfSource::Buffer {
            bytes,
            index: 0,
            consumed: 0,
        }
    }

    pub(super) fn scan_consumed(source: &ByteScanfSource) -> usize {
        match source {
            ByteScanfSource::Stream { consumed, .. } | ByteScanfSource::Buffer { consumed, .. } => {
                *consumed
            }
        }
    }

    pub(super) fn scan_next(
        &mut self,
        source: &mut ByteScanfSource,
        span: Span,
    ) -> Result<Option<u8>, Diagnostic> {
        match source {
            ByteScanfSource::Buffer {
                bytes,
                index,
                consumed,
            } => {
                let Some(byte) = bytes.get(*index).copied() else {
                    return Ok(None);
                };
                *index += 1;
                *consumed = consumed.saturating_add(1);
                Ok(Some(byte))
            }
            ByteScanfSource::Stream {
                stream_object,
                function_name,
                standard,
                pushback,
                consumed,
            } => {
                if let Some(byte) = pushback.pop() {
                    *consumed = consumed.saturating_add(1);
                    return Ok(Some(byte));
                }
                if let Some(ch) =
                    self.read_one_byte_from_stream(*stream_object, span, function_name, standard)?
                {
                    *consumed = consumed.saturating_add(1);
                    Ok(Some(ch))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub(super) fn scan_unread(&self, source: &mut ByteScanfSource, byte: u8) {
        match source {
            ByteScanfSource::Buffer {
                index, consumed, ..
            } => {
                *index = index.saturating_sub(1);
                *consumed = consumed.saturating_sub(1);
            }
            ByteScanfSource::Stream {
                pushback, consumed, ..
            } => {
                pushback.push(byte);
                *consumed = consumed.saturating_sub(1);
            }
        }
    }

    pub(super) fn finalize_scan_source(
        &mut self,
        source: &mut ByteScanfSource,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let ByteScanfSource::Stream {
            stream_object,
            pushback,
            ..
        } = source
        else {
            return Ok(());
        };
        if pushback.is_empty() {
            return Ok(());
        }
        for &byte in pushback.iter() {
            self.unread_byte_to_stream(*stream_object, byte, span, "fscanf", "7.19.6.2")?;
        }
        if let Some(stream) = self.host_streams.get_mut(stream_object) {
            stream.eof = false;
            stream.error = false;
        }
        pushback.clear();
        Ok(())
    }

    pub(super) fn scan_skip_ws(
        &mut self,
        source: &mut ByteScanfSource,
        span: Span,
    ) -> Result<(), Diagnostic> {
        loop {
            let Some(byte) = self.scan_next(source, span)? else {
                return Ok(());
            };
            if unsafe { libc::isspace(c_int::from(byte)) } == 0 {
                self.scan_unread(source, byte);
                return Ok(());
            }
        }
    }

    pub(super) fn scan_collect_token(
        &mut self,
        source: &mut ByteScanfSource,
        width: Option<usize>,
        span: Span,
    ) -> Result<(Vec<u8>, bool), Diagnostic> {
        let mut token = Vec::new();
        let mut saw_eof = false;
        while width.is_none_or(|limit| token.len() < limit) {
            let Some(byte) = self.scan_next(source, span)? else {
                saw_eof = true;
                break;
            };
            if unsafe { libc::isspace(c_int::from(byte)) } != 0 {
                self.scan_unread(source, byte);
                break;
            }
            token.push(byte);
        }
        Ok((token, saw_eof))
    }

    pub(super) fn scan_read_exact(
        &mut self,
        source: &mut ByteScanfSource,
        width: usize,
        span: Span,
    ) -> Result<Option<Vec<u8>>, Diagnostic> {
        let mut bytes = Vec::with_capacity(width);
        for _ in 0..width {
            let Some(byte) = self.scan_next(source, span)? else {
                return Ok(None);
            };
            bytes.push(byte);
        }
        Ok(Some(bytes))
    }

    pub(super) fn scan_token_to_signed_bytes(
        &self,
        token: &[u8],
        base: c_int,
    ) -> Option<(i128, usize, bool)> {
        let mut bytes = token.to_vec();
        bytes.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { strtoll(bytes.as_ptr().cast::<c_char>(), &mut end, base) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE;
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(bytes.as_ptr().cast::<c_char>()) as usize }
        };
        (consumed != 0).then_some((value as i128, consumed, overflow))
    }

    pub(super) fn scan_token_to_unsigned_bytes(
        &self,
        token: &[u8],
        base: c_int,
    ) -> Option<(i128, usize, bool)> {
        let mut bytes = token.to_vec();
        bytes.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { strtoull(bytes.as_ptr().cast::<c_char>(), &mut end, base) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE;
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(bytes.as_ptr().cast::<c_char>()) as usize }
        };
        (consumed != 0).then_some((value as i128, consumed, overflow))
    }

    pub(super) fn scan_token_to_float_bytes(&self, token: &[u8]) -> Option<(f64, usize, bool)> {
        let mut bytes = token.to_vec();
        bytes.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { strtod(bytes.as_ptr().cast::<c_char>(), &mut end) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE && value.is_infinite();
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(bytes.as_ptr().cast::<c_char>()) as usize }
        };
        (consumed != 0).then_some((value, consumed, overflow))
    }

    pub(super) fn scan_numeric_input_item_bytes(
        &self,
        token: &[u8],
        spec: char,
    ) -> Option<(usize, bool)> {
        let host_consumed = match spec {
            'd' => self
                .scan_token_to_signed_bytes(token, 10)
                .map(|(_, consumed, _)| consumed),
            'i' => self
                .scan_token_to_signed_bytes(token, 0)
                .map(|(_, consumed, _)| consumed),
            'o' => self
                .scan_token_to_unsigned_bytes(token, 8)
                .map(|(_, consumed, _)| consumed),
            'u' => self
                .scan_token_to_unsigned_bytes(token, 10)
                .map(|(_, consumed, _)| consumed),
            'x' | 'X' | 'p' => self
                .scan_token_to_unsigned_bytes(token, 16)
                .map(|(_, consumed, _)| consumed),
            _ => self
                .scan_token_to_float_bytes(token)
                .map(|(_, consumed, _)| consumed),
        };
        Self::scan_numeric_input_item_ascii(token, spec)
            .map(|standard| {
                host_consumed
                    .filter(|consumed| *consumed > standard.0)
                    .map_or(standard, |consumed| (consumed, true))
            })
            .or_else(|| host_consumed.map(|consumed| (consumed, true)))
    }

    fn scan_numeric_input_item_ascii(token: &[u8], spec: char) -> Option<(usize, bool)> {
        let mut index = usize::from(matches!(token.first(), Some(b'+' | b'-')));
        let signed_prefix = index != 0;
        if index == token.len() {
            return signed_prefix.then_some((index, false));
        }

        if matches!(spec, 'a' | 'A' | 'e' | 'E' | 'f' | 'F' | 'g' | 'G') {
            let lower = |byte: u8| byte.to_ascii_lowercase();
            if matches!(lower(token[index]), b'i' | b'n') {
                let start = index;
                if lower(token[index]) == b'i' {
                    let expected = b"infinity";
                    while index - start < expected.len()
                        && index < token.len()
                        && lower(token[index]) == expected[index - start]
                    {
                        index += 1;
                    }
                    let matched = index - start;
                    if matched == 0 {
                        return None;
                    }
                    let complete = matched == expected.len()
                        || (matched == 3
                            && (index == token.len() || lower(token[index]) != expected[matched]));
                    return Some((index, complete));
                }

                let expected = b"nan";
                while index - start < expected.len()
                    && index < token.len()
                    && lower(token[index]) == expected[index - start]
                {
                    index += 1;
                }
                let matched = index - start;
                if matched < expected.len() {
                    return Some((index, false));
                }
                if token.get(index) != Some(&b'(') {
                    return Some((index, true));
                }
                index += 1;
                while token
                    .get(index)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    index += 1;
                }
                if token.get(index) == Some(&b')') {
                    return Some((index + 1, true));
                }
                return Some((index, false));
            }

            if token.get(index) == Some(&b'0')
                && token
                    .get(index + 1)
                    .is_some_and(|byte| matches!(byte, b'x' | b'X'))
            {
                index += 2;
                let significand_start = index;
                while token.get(index).is_some_and(u8::is_ascii_hexdigit) {
                    index += 1;
                }
                let mut digits = index - significand_start;
                if token.get(index) == Some(&b'.') {
                    index += 1;
                    let fraction_start = index;
                    while token.get(index).is_some_and(u8::is_ascii_hexdigit) {
                        index += 1;
                    }
                    digits += index - fraction_start;
                }
                if digits == 0 {
                    return Some((index, false));
                }
                if !token
                    .get(index)
                    .is_some_and(|byte| matches!(byte, b'p' | b'P'))
                {
                    return Some((index, true));
                }
                index += 1;
                if token
                    .get(index)
                    .is_some_and(|byte| matches!(byte, b'+' | b'-'))
                {
                    index += 1;
                }
                let exponent_start = index;
                while token.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                return Some((index, index != exponent_start));
            }

            let significand_start = index;
            while token.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            let mut digits = index - significand_start;
            if token.get(index) == Some(&b'.') {
                index += 1;
                let fraction_start = index;
                while token.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                digits += index - fraction_start;
            }
            if digits == 0 {
                return (index != 0).then_some((index, false));
            }
            if token
                .get(index)
                .is_some_and(|byte| matches!(byte, b'e' | b'E'))
            {
                index += 1;
                if token
                    .get(index)
                    .is_some_and(|byte| matches!(byte, b'+' | b'-'))
                {
                    index += 1;
                }
                let exponent_start = index;
                while token.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                return Some((index, index != exponent_start));
            }
            return Some((index, true));
        }

        let digit_matches = |byte: u8, radix: u8| match radix {
            8 => matches!(byte, b'0'..=b'7'),
            10 => byte.is_ascii_digit(),
            16 => byte.is_ascii_hexdigit(),
            _ => false,
        };
        let radix = match spec {
            'o' => 8,
            'x' | 'X' | 'p' => 16,
            'i' if token.get(index) == Some(&b'0') => 8,
            _ => 10,
        };
        if matches!(spec, 'i' | 'x' | 'X' | 'p')
            && token.get(index) == Some(&b'0')
            && token
                .get(index + 1)
                .is_some_and(|byte| matches!(byte, b'x' | b'X'))
        {
            index += 2;
            let digits_start = index;
            while token
                .get(index)
                .is_some_and(|byte| digit_matches(*byte, 16))
            {
                index += 1;
            }
            return Some((index, index != digits_start));
        }
        let digits_start = index;
        while token
            .get(index)
            .is_some_and(|byte| digit_matches(*byte, radix))
        {
            index += 1;
        }
        if index == digits_start {
            return signed_prefix.then_some((index, false));
        }
        Some((index, true))
    }

    pub(super) fn scan_read_multibyte_char(
        &mut self,
        source: &mut ByteScanfSource,
        span: Span,
        state: &mut HostMbState,
    ) -> Result<Option<(libc::wchar_t, Vec<u8>)>, Diagnostic> {
        let mut bytes = Vec::new();
        let max_len = host_mb_cur_max().max(1);
        loop {
            let Some(byte) = self.scan_next(source, span)? else {
                return Ok(None);
            };
            bytes.push(byte);
            let mut next_state = HostMbState {
                opaque: state.opaque,
            };
            let mut unit = 0 as libc::wchar_t;
            let status = unsafe {
                mbrtowc(
                    &mut unit,
                    bytes.as_ptr().cast::<c_char>(),
                    bytes.len(),
                    &mut next_state as *mut HostMbState,
                )
            };
            if status == usize::MAX {
                return Ok(None);
            }
            if status == usize::MAX - 1 {
                if bytes.len() >= max_len {
                    return Ok(None);
                }
                continue;
            }
            *state = next_state;
            return Ok(Some((unit, bytes)));
        }
    }

    fn scanf_integer_pointee_type(
        &self,
        conv: &ScanfConversion,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let signed = matches!(conv.spec, 'd' | 'i' | 'n');
        let ty = match conv.length {
            ScanfLength::None => {
                if signed {
                    CType::Int
                } else {
                    CType::UnsignedInt
                }
            }
            ScanfLength::H => {
                if signed {
                    CType::Short
                } else {
                    CType::UnsignedShort
                }
            }
            ScanfLength::Hh => {
                if signed {
                    CType::SignedChar
                } else {
                    CType::UnsignedChar
                }
            }
            ScanfLength::L => {
                if signed {
                    CType::Long
                } else {
                    CType::UnsignedLong
                }
            }
            ScanfLength::Ll => {
                if signed {
                    CType::LongLong
                } else {
                    CType::UnsignedLongLong
                }
            }
            ScanfLength::J => {
                if signed {
                    CType::Long
                } else {
                    CType::UnsignedLong
                }
            }
            ScanfLength::Z => {
                if signed {
                    CType::Long
                } else {
                    CType::UnsignedLong
                }
            }
            ScanfLength::T => CType::Long,
            ScanfLength::BigL => {
                return Err(Diagnostic::error(
                    "scanf integer conversion does not accept the L length modifier",
                    span,
                ));
            }
        };
        Ok(ty)
    }

    fn scanf_float_pointee_type(
        &self,
        conv: &ScanfConversion,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        match conv.length {
            ScanfLength::None => Ok(CType::Float),
            ScanfLength::L => Ok(CType::Double),
            ScanfLength::BigL => Ok(CType::LongDouble),
            _ => Err(Diagnostic::error(
                "scanf floating conversion uses an unsupported length modifier",
                span,
            )),
        }
    }

    fn wide_scan_output_pointer(
        &self,
        function_name: &str,
        value: &TypedValue,
        expected_pointer_ty: &CType,
        span: Span,
    ) -> Result<PointerValue, Diagnostic> {
        if !self.pointer_assignment_compatible(expected_pointer_ty, &value.ty) {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} argument must have type {}",
                    expected_pointer_ty
                ),
                span,
                Some("7.24.2.2"),
            ));
        }
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a non-null output pointer"),
                span,
                Some("7.24.2.2"),
            ));
        }
        Ok(pointer)
    }

    fn scan_character_output_pointer(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<(PointerValue, CType), Diagnostic> {
        let CType::Pointer(element_ty) = value.ty.unqualified() else {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} character conversion requires a pointer to a writable character type"
                ),
                span,
                Some("7.24.2.2"),
            ));
        };
        if !element_ty.is_character() || element_ty.is_const_qualified() {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} character conversion requires a pointer to a writable character type"
                ),
                span,
                Some("7.24.2.2"),
            ));
        }
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a non-null output pointer"),
                span,
                Some("7.24.2.2"),
            ));
        }
        Ok((pointer, (**element_ty).clone()))
    }

    fn wide_scan_capacity(
        &self,
        pointer: &PointerValue,
        element_ty: &CType,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        let limit = self
            .object_pointer_limit(objects, pointer, element_ty)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "scanf destination does not designate a valid writable array or object",
                    span,
                    Some("7.24.2.2"),
                )
            })?;
        let capacity = usize::try_from(limit.max(0)).map_err(|_| {
            Diagnostic::ub(
                "scanf destination capacity is out of supported range",
                span,
                Some("7.24.2.2"),
            )
        })?;
        if capacity == 0 {
            return Err(Diagnostic::ub(
                "scanf destination does not designate an element of a writable array or object",
                span,
                Some("7.24.2.2"),
            ));
        }
        let size = self
            .type_size_of(element_ty)
            .ok_or_else(|| Diagnostic::error("scanf destination element type has no size", span))?;
        let (object_id, start, _) = self.byte_region_from_pointer(pointer, size, span, objects)?;
        self.ensure_library_writable_region(object_id, start, size, span, objects)?;
        Ok(capacity)
    }

    fn write_scan_bytes(
        &mut self,
        pointer: &PointerValue,
        bytes: &[u8],
        add_nul: bool,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let total = bytes.len() + usize::from(add_nul);
        if total == 0 {
            return Ok(());
        }
        let (object_id, start, _) = self.byte_region_from_pointer(pointer, total, span, objects)?;
        self.ensure_library_writable_region(object_id, start, total, span, objects)?;
        let mut rendered = bytes.to_vec();
        if add_nul {
            rendered.push(0);
        }
        self.overlay_known_bytes_into_object(object_id, start, &rendered, span, objects)
    }

    fn scan_token_to_signed(
        &self,
        token: &[libc::wchar_t],
        base: c_int,
    ) -> Option<(i128, usize, bool)> {
        let mut units = token.to_vec();
        units.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { wcstoll(units.as_ptr(), &mut end, base) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE;
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(units.as_ptr()) as usize }
        };
        (consumed != 0).then_some((value as i128, consumed, overflow))
    }

    fn scan_token_to_unsigned(
        &self,
        token: &[libc::wchar_t],
        base: c_int,
    ) -> Option<(i128, usize, bool)> {
        let mut units = token.to_vec();
        units.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { wcstoull(units.as_ptr(), &mut end, base) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE;
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(units.as_ptr()) as usize }
        };
        (consumed != 0).then_some((value as i128, consumed, overflow))
    }

    fn scan_token_to_float(&self, token: &[libc::wchar_t]) -> Option<(f64, usize, bool)> {
        let mut units = token.to_vec();
        units.push(0);
        let mut end = std::ptr::null_mut();
        let previous_errno = unsafe { *host_errno_ptr() };
        Self::set_host_errno(0);
        let value = unsafe { wcstod(units.as_ptr(), &mut end) };
        let overflow = unsafe { *host_errno_ptr() } == libc::ERANGE && value.is_infinite();
        Self::set_host_errno(previous_errno);
        let consumed = if end.is_null() {
            0
        } else {
            unsafe { end.offset_from(units.as_ptr()) as usize }
        };
        (consumed != 0).then_some((value, consumed, overflow))
    }

    fn wide_scan_numeric_input_item(
        &self,
        token: &[libc::wchar_t],
        spec: char,
    ) -> Option<(usize, bool)> {
        let ascii = token
            .iter()
            .map(|unit| u8::try_from(*unit).unwrap_or(0xff))
            .collect::<Vec<_>>();
        let host_consumed = match spec {
            'd' => self
                .scan_token_to_signed(token, 10)
                .map(|(_, consumed, _)| consumed),
            'i' => self
                .scan_token_to_signed(token, 0)
                .map(|(_, consumed, _)| consumed),
            'o' => self
                .scan_token_to_unsigned(token, 8)
                .map(|(_, consumed, _)| consumed),
            'u' => self
                .scan_token_to_unsigned(token, 10)
                .map(|(_, consumed, _)| consumed),
            'x' | 'X' | 'p' => self
                .scan_token_to_unsigned(token, 16)
                .map(|(_, consumed, _)| consumed),
            _ => self
                .scan_token_to_float(token)
                .map(|(_, consumed, _)| consumed),
        };
        Self::scan_numeric_input_item_ascii(&ascii, spec)
            .map(|standard| {
                host_consumed
                    .filter(|consumed| *consumed > standard.0)
                    .map_or(standard, |consumed| (consumed, true))
            })
            .or_else(|| host_consumed.map(|consumed| (consumed, true)))
    }

    fn wide_scan_collect_token(
        &mut self,
        source: &mut WideScanfSource,
        width: Option<usize>,
        span: Span,
    ) -> Result<(Vec<libc::wchar_t>, bool), Diagnostic> {
        let mut token = Vec::new();
        let mut saw_eof = false;
        while width.is_none_or(|limit| token.len() < limit) {
            let Some(unit) = self.wide_scan_next(source, span)? else {
                saw_eof = true;
                break;
            };
            if unsafe { iswspace(unit as c_int) } != 0 {
                self.wide_scan_unread(source, unit);
                break;
            }
            token.push(unit);
        }
        Ok((token, saw_eof))
    }

    fn wide_scan_read_exact(
        &mut self,
        source: &mut WideScanfSource,
        width: usize,
        span: Span,
    ) -> Result<Option<Vec<libc::wchar_t>>, Diagnostic> {
        let mut units = Vec::with_capacity(width);
        for _ in 0..width {
            let Some(unit) = self.wide_scan_next(source, span)? else {
                return Ok(None);
            };
            units.push(unit);
        }
        Ok(Some(units))
    }

    fn wide_unit_to_multibyte(
        &self,
        unit: libc::wchar_t,
        state: &mut HostMbState,
    ) -> Option<Vec<u8>> {
        let mut buffer = [0u8; 32];
        let written = unsafe {
            wcrtomb(
                buffer.as_mut_ptr().cast::<c_char>(),
                unit,
                state as *mut HostMbState,
            )
        };
        (written != usize::MAX).then(|| buffer[..written].to_vec())
    }

    pub(super) fn store_scan_integer(
        &mut self,
        function_name: &str,
        conv: &ScanfConversion,
        dest: &TypedValue,
        span: Span,
        value: i128,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let pointee = self.scanf_integer_pointee_type(conv, span)?;
        let (minimum, maximum) = pointee
            .integer_bounds()
            .expect("scanf integer destination has integer bounds");
        if !(minimum..=maximum).contains(&value) {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} conversion result is not representable in the receiving object"
                ),
                span,
                Some(Self::scanf_pointer_input_standard(function_name)),
            ));
        }
        let pointer = self.wide_scan_output_pointer(
            function_name,
            dest,
            &CType::pointer_to(pointee.clone()),
            span,
        )?;
        let lvalue = self.pointer_lvalue(function_name, &pointer, &pointee, span, "7.24.2.2")?;
        self.store_lvalue(objects, &lvalue, TypedValue::integer(pointee, value), span)
    }

    pub(super) fn store_scan_float(
        &mut self,
        function_name: &str,
        conv: &ScanfConversion,
        dest: &TypedValue,
        span: Span,
        value: f64,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let pointee = self.scanf_float_pointee_type(conv, span)?;
        if pointee == CType::Float && value.is_finite() && (value as f32).is_infinite() {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} conversion result is not representable in the receiving object"
                ),
                span,
                Some(Self::scanf_pointer_input_standard(function_name)),
            ));
        }
        let pointer = self.wide_scan_output_pointer(
            function_name,
            dest,
            &CType::pointer_to(pointee.clone()),
            span,
        )?;
        let lvalue = self.pointer_lvalue(function_name, &pointer, &pointee, span, "7.24.2.2")?;
        self.store_lvalue(objects, &lvalue, TypedValue::floating(pointee, value), span)
    }

    pub(super) fn store_scan_pointer(
        &mut self,
        function_name: &str,
        dest: &TypedValue,
        span: Span,
        pointer_value: PointerValue,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let expected = CType::pointer_to(self.void_ptr_type());
        let pointer = self.wide_scan_output_pointer(function_name, dest, &expected, span)?;
        let lvalue = self.pointer_lvalue(
            function_name,
            &pointer,
            &self.void_ptr_type(),
            span,
            "7.24.2.2",
        )?;
        self.store_lvalue(
            objects,
            &lvalue,
            self.pointer_value_with_type(self.void_ptr_type(), pointer_value),
            span,
        )
    }

    pub(super) fn store_scan_narrow_buffer(
        &mut self,
        function_name: &str,
        dest: &TypedValue,
        span: Span,
        bytes: &[u8],
        add_nul: bool,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let (pointer, element_ty) =
            self.scan_character_output_pointer(function_name, dest, span)?;
        let capacity = self.wide_scan_capacity(&pointer, &element_ty, span, objects)?;
        let needed = bytes.len() + usize::from(add_nul);
        if needed > capacity {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} destination array is too small for the converted multibyte sequence"
                ),
                span,
                Some("7.24.2.2"),
            ));
        }
        self.write_scan_bytes(&pointer, bytes, add_nul, span, objects)
    }

    pub(super) fn store_scan_wide_buffer(
        &mut self,
        function_name: &str,
        dest: &TypedValue,
        span: Span,
        units: &[libc::wchar_t],
        add_nul: bool,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let pointer =
            self.wide_scan_output_pointer(function_name, dest, &self.wchar_ptr_type(), span)?;
        let capacity = self.wide_scan_capacity(&pointer, &self.wchar_type(), span, objects)?;
        let needed = units.len() + usize::from(add_nul);
        if needed > capacity {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} destination array is too small for the scanned wide string"
                ),
                span,
                Some("7.24.2.2"),
            ));
        }
        if add_nul {
            self.write_wide_string_to_pointer(&pointer, units, span, objects)
        } else {
            self.write_wide_units_to_pointer(&pointer, units, span, objects)
        }
    }

    pub(super) fn pointer_from_scanned_address(&self, address: u64) -> Option<PointerValue> {
        if address == 0 {
            return Some(Self::null_pointer());
        }
        self.decoded_pointers
            .get(&address)?
            .iter()
            .find_map(|candidate| match candidate {
                EncodedPointer::Object { pointer, .. } => Some(pointer.clone()),
                EncodedPointer::Function(_) => None,
            })
    }

    pub(super) fn pointer_value_from_integer(
        &self,
        address: u64,
        target: &CType,
        indeterminate: bool,
    ) -> TypedValue {
        if address == 0 {
            return TypedValue {
                ty: target.clone(),
                data: ValueData::Pointer(Rc::new(Self::null_pointer())),
                restrict_source: None,
                indeterminate,
                missing_return: false,
            };
        }

        let target_is_function = matches!(
            target.unqualified(),
            CType::Pointer(inner) if matches!(inner.unqualified(), CType::Function(..))
        );
        if let Some(encoded) = self.decoded_pointers.get(&address) {
            let data = if target_is_function {
                encoded.iter().find_map(|candidate| match candidate {
                    EncodedPointer::Function(name) => {
                        Some(ValueData::Function(name.clone().into()))
                    }
                    EncodedPointer::Object { .. } => None,
                })
            } else {
                let CType::Pointer(target_inner) = target.unqualified() else {
                    unreachable!("integer conversion target is a pointer")
                };
                self.best_decoded_object_pointer(encoded, target_inner)
                    .map(|pointer| ValueData::Pointer(Rc::new(pointer)))
            };
            if let Some(data) = data {
                return TypedValue {
                    ty: target.clone(),
                    data,
                    restrict_source: None,
                    indeterminate,
                    missing_return: false,
                };
            }
        }

        if !target_is_function {
            for (object, base) in &self.object_base_addresses {
                let Some(root_ty) = self.object_type_registry.get(object) else {
                    continue;
                };
                let Some(size) = self.type_size_of(root_ty).map(|size| size as u64) else {
                    continue;
                };
                let Some(end) = base.checked_add(size) else {
                    continue;
                };
                if address >= *base && address <= end {
                    return TypedValue {
                        ty: target.clone(),
                        data: ValueData::Pointer(Rc::new(PointerValue {
                            object: Some(*object),
                            base_offset: 0,
                            offset: 0,
                            member_path: Rc::new(Vec::new()),
                            designated_root_ty: None,
                            byte_offset_override: Some((address - base) as usize),
                            arithmetic_domain_start: None,
                            object_representation_domain: None,
                        })),
                        restrict_source: None,
                        indeterminate,
                        missing_return: false,
                    };
                }
            }
        }
        if let Ok(byte_offset) = usize::try_from(address) {
            let pointer = PointerValue {
                object: None,
                base_offset: 1,
                offset: 0,
                member_path: Rc::new(Vec::new()),
                designated_root_ty: None,
                byte_offset_override: Some(byte_offset),
                arithmetic_domain_start: None,
                object_representation_domain: None,
            };
            self.opaque_integer_pointer_addresses
                .borrow_mut()
                .insert(address);
            return TypedValue {
                ty: target.clone(),
                data: ValueData::Pointer(Rc::new(pointer)),
                restrict_source: None,
                indeterminate,
                missing_return: false,
            };
        }

        TypedValue::object_representation(
            target.clone(),
            address
                .to_le_bytes()
                .into_iter()
                .map(ByteCell::Known)
                .collect(),
        )
    }

    pub(super) fn opaque_integer_pointer_address(pointer: &PointerValue) -> Option<u64> {
        (pointer.object.is_none()
            && pointer.base_offset == 1
            && pointer.offset == 0
            && pointer.member_path.is_empty())
        .then_some(pointer.byte_offset_override)
        .flatten()
        .and_then(|address| u64::try_from(address).ok())
    }

    pub(super) fn scanf_pointer_input_standard(function_name: &str) -> &'static str {
        if function_name.contains('w') {
            "7.24.2.2"
        } else {
            "7.19.6.2"
        }
    }

    fn eval_wscanf_conversion(
        &mut self,
        function_name: &'static str,
        conv: &ScanfConversion,
        source: &mut WideScanfSource,
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
                        Self::wide_scan_consumed(source) as i128,
                        objects,
                    )?;
                }
                Ok(ScanConversionStatus::NoAssignment)
            }
            'c' => {
                let width = conv.width.unwrap_or(1);
                let Some(units) = self.wide_scan_read_exact(source, width, call_span)? else {
                    return Ok(ScanConversionStatus::InputFailure);
                };
                if let Some(dest) = dest {
                    match conv.length {
                        ScanfLength::None => {
                            let mut state = HostMbState { opaque: [0; 128] };
                            let mut bytes = Vec::new();
                            for unit in units {
                                let Some(chunk) = self.wide_unit_to_multibyte(unit, &mut state)
                                else {
                                    return Ok(ScanConversionStatus::InputFailure);
                                };
                                bytes.extend(chunk);
                            }
                            self.store_scan_narrow_buffer(
                                function_name,
                                dest,
                                dest_span,
                                &bytes,
                                false,
                                objects,
                            )?;
                        }
                        ScanfLength::L => {
                            self.store_scan_wide_buffer(
                                function_name,
                                dest,
                                dest_span,
                                &units,
                                false,
                                objects,
                            )?;
                        }
                        _ => {
                            return Err(Diagnostic::error(
                                format!("{function_name} %c uses an unsupported length modifier"),
                                dest_span,
                            ));
                        }
                    }
                }
                Ok(if conv.suppress {
                    ScanConversionStatus::NoAssignment
                } else {
                    ScanConversionStatus::Assigned
                })
            }
            's' | '[' => {
                if conv.spec == 's' {
                    self.wide_scan_skip_ws(source, call_span)?;
                }
                let mut units = Vec::new();
                let mut hit_eof = false;
                loop {
                    if conv.width.is_some_and(|limit| units.len() >= limit) {
                        break;
                    }
                    let Some(unit) = self.wide_scan_next(source, call_span)? else {
                        hit_eof = true;
                        break;
                    };
                    let matches = if conv.spec == 's' {
                        (unsafe { iswspace(unit as c_int) }) == 0
                    } else {
                        self.scanset_matches(conv.scanset.as_ref().expect("scanset"), unit)
                    };
                    if !matches {
                        self.wide_scan_unread(source, unit);
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
                    match conv.length {
                        ScanfLength::None => {
                            let mut state = HostMbState { opaque: [0; 128] };
                            let mut bytes = Vec::new();
                            for unit in units {
                                let Some(chunk) = self.wide_unit_to_multibyte(unit, &mut state)
                                else {
                                    return Ok(ScanConversionStatus::InputFailure);
                                };
                                bytes.extend(chunk);
                            }
                            self.store_scan_narrow_buffer(
                                function_name,
                                dest,
                                dest_span,
                                &bytes,
                                true,
                                objects,
                            )?;
                        }
                        ScanfLength::L => {
                            self.store_scan_wide_buffer(
                                function_name,
                                dest,
                                dest_span,
                                &units,
                                true,
                                objects,
                            )?;
                        }
                        _ => {
                            return Err(Diagnostic::error(
                                format!(
                                    "{function_name} %{} uses an unsupported length modifier",
                                    conv.spec
                                ),
                                dest_span,
                            ));
                        }
                    }
                }
                Ok(if conv.suppress {
                    ScanConversionStatus::NoAssignment
                } else {
                    ScanConversionStatus::Assigned
                })
            }
            'd' | 'i' | 'o' | 'u' | 'x' | 'X' | 'p' | 'a' | 'A' | 'e' | 'E' | 'f' | 'F' | 'g'
            | 'G' => {
                self.wide_scan_skip_ws(source, call_span)?;
                let (token, hit_eof) =
                    self.wide_scan_collect_token(source, conv.width, call_span)?;
                if token.is_empty() {
                    return Ok(if hit_eof {
                        ScanConversionStatus::InputFailure
                    } else {
                        ScanConversionStatus::MatchingFailure
                    });
                }
                let Some((item_len, complete)) =
                    self.wide_scan_numeric_input_item(&token, conv.spec)
                else {
                    for &unit in token.iter().rev() {
                        self.wide_scan_unread(source, unit);
                    }
                    return Ok(ScanConversionStatus::MatchingFailure);
                };
                for &unit in token[item_len..].iter().rev() {
                    self.wide_scan_unread(source, unit);
                }
                if !complete {
                    return Ok(ScanConversionStatus::MatchingFailure);
                }
                let token = &token[..item_len];
                let parsed =
                    match conv.spec {
                        'd' => self.scan_token_to_signed(token, 10).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        'i' => self.scan_token_to_signed(token, 0).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        'o' => self.scan_token_to_unsigned(token, 8).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        'u' => self.scan_token_to_unsigned(token, 10).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        'x' | 'X' => self.scan_token_to_unsigned(token, 16).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        'p' => self.scan_token_to_unsigned(token, 16).map(
                            |(value, consumed, overflow)| {
                                (TypedValue::int(value), consumed, overflow)
                            },
                        ),
                        _ => self
                            .scan_token_to_float(token)
                            .map(|(value, consumed, overflow)| {
                                (
                                    TypedValue::floating(CType::Double, value),
                                    consumed,
                                    overflow,
                                )
                            }),
                    };
                let Some((parsed, consumed, overflow)) = parsed else {
                    return Ok(ScanConversionStatus::MatchingFailure);
                };
                for &unit in token[consumed..].iter().rev() {
                    self.wide_scan_unread(source, unit);
                }
                if let Some(dest) = dest {
                    if overflow {
                        return Err(Diagnostic::ub(
                            format!(
                                "{function_name} conversion result is not representable in the receiving object"
                            ),
                            dest_span,
                            Some("7.29.2.1"),
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

    pub(super) fn eval_wscanf_like(
        &mut self,
        function_name: &'static str,
        directives: &[ScanfDirective],
        source: &mut WideScanfSource,
        outputs: &[TypedValue],
        output_spans: &[Span],
        call_span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(i32, usize), Diagnostic> {
        let mut progress = ScanProgress::default();
        let mut used_args = 0usize;
        for directive in directives {
            match directive {
                ScanfDirective::Whitespace => {
                    self.wide_scan_skip_ws(source, call_span)?;
                }
                ScanfDirective::Literal(_) | ScanfDirective::Percent => {
                    let expected = match directive {
                        ScanfDirective::Literal(unit) => *unit,
                        _ => {
                            self.wide_scan_skip_ws(source, call_span)?;
                            '%' as libc::wchar_t
                        }
                    };
                    let Some(unit) = self.wide_scan_next(source, call_span)? else {
                        return Ok((progress.input_failure_result(), used_args));
                    };
                    if unit != expected {
                        self.wide_scan_unread(source, unit);
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
                    let status = self.eval_wscanf_conversion(
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
}
