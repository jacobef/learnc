use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn reject_indeterminate_library_value(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &str,
    ) -> Result<(), Diagnostic> {
        if value.indeterminate {
            return Err(Diagnostic::ub(
                format!("{function_name} argument has an indeterminate value"),
                span,
                Some("7.1.4, 7.19.6.1, WG14 DR 451"),
            )
            .with_note(
                "library functions exhibit undefined behavior when used on indeterminate values",
            ));
        }
        Ok(())
    }

    pub(super) fn check_library_argument_value(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &str,
    ) -> Result<(), Diagnostic> {
        self.reject_missing_return_value(value, span)?;
        self.reject_indeterminate_library_value(value, span, function_name)
    }

    fn check_math_unary_real_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_real_arg(function_name, &evaluated[0], args[0].span())
    }

    fn check_math_binary_real_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_real_arg(function_name, &evaluated[0], args[0].span())?;
        self.check_math_real_arg(function_name, &evaluated[1], args[1].span())
    }

    fn check_math_ternary_real_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_real_arg(function_name, &evaluated[0], args[0].span())?;
        self.check_math_real_arg(function_name, &evaluated[1], args[1].span())?;
        self.check_math_real_arg(function_name, &evaluated[2], args[2].span())
    }

    fn check_math_real_int_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        expected: CType,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_real_arg(function_name, &evaluated[0], args[0].span())?;
        self.check_math_integer_arg(function_name, &evaluated[1], expected, args[1].span())
    }

    fn check_math_output_pointer_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        pointer_index: usize,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        for (index, value) in evaluated.iter().enumerate() {
            if index == pointer_index {
                self.check_math_pointer_output_arg(
                    function_name,
                    value,
                    args[index].span(),
                    objects,
                )?;
            } else {
                self.check_math_real_arg(function_name, value, args[index].span())?;
            }
        }
        Ok(())
    }

    fn check_math_nan_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_string_arg(function_name, &evaluated[0], args[0].span(), objects)
    }

    fn check_math_helper_nullary_call(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !args.is_empty() || !evaluated.is_empty() {
            return Err(Diagnostic::error(
                format!("{function_name} requires no arguments"),
                span,
            ));
        }
        Ok(())
    }

    fn check_malloc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("malloc", args, evaluated, 1, span)?;
        let size = &evaluated[0];
        self.check_library_argument_value(size, args[0].span(), "malloc")?;
        if size.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "malloc size argument must have type unsigned long",
                args[0].span(),
            ));
        }
        let size = usize::try_from(size.to_int()?).map_err(|_| {
            Diagnostic::error("malloc size is out of supported range", args[0].span())
        })?;
        self.pending_heap_call = Some(PendingHeapCall::Malloc(size));
        Ok(())
    }

    fn check_aligned_alloc_call(
        &self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("aligned_alloc", args, evaluated, 2, span)?;
        for (index, label) in ["alignment", "size"].into_iter().enumerate() {
            self.check_library_argument_value(
                &evaluated[index],
                args[index].span(),
                "aligned_alloc",
            )?;
            if evaluated[index].ty != CType::UnsignedLong {
                return Err(Diagnostic::error(
                    format!("aligned_alloc {label} must have type unsigned long"),
                    args[index].span(),
                ));
            }
        }
        let alignment = self.checked_usize_from_unsigned_long(
            &evaluated[0],
            args[0].span(),
            "aligned_alloc alignment",
        )?;
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "aligned_alloc size",
        )?;
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(Diagnostic::ub(
                "aligned_alloc alignment is not a supported valid alignment",
                args[0].span(),
                Some("7.22.3.1"),
            ));
        }
        if size % alignment != 0 {
            return Err(Diagnostic::ub(
                "aligned_alloc size is not an integral multiple of its alignment",
                args[1].span(),
                Some("7.22.3.1"),
            ));
        }
        Ok(())
    }

    fn check_abs_call(
        &self,
        function_name: &'static str,
        args: &[Expr],
        evaluated: &[TypedValue],
        expected: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), function_name)?;
        if &value.ty != expected {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type {expected}"),
                args[0].span(),
            ));
        }
        let int_value = value.to_int()?;
        if int_value < 0 {
            let _ =
                self.ensure_integer_ub_range(-int_value, expected, args[0].span(), "7.20.6.1")?;
        }
        Ok(())
    }

    fn char_ptr_ptr_type(&self) -> CType {
        CType::pointer_to(self.char_ptr_type())
    }

    pub(super) fn wchar_type(&self) -> CType {
        CType::Int
    }

    pub(super) fn wchar_ptr_type(&self) -> CType {
        CType::pointer_to(self.wchar_type())
    }

    pub(super) fn const_wchar_ptr_type(&self) -> CType {
        CType::pointer_to(CType::qualified(
            self.wchar_type(),
            crate::types::TypeQualifiers {
                is_atomic: false,
                is_const: true,
                is_restrict: false,
                is_volatile: false,
            },
        ))
    }

    fn check_unary_string_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_math_string_arg(function_name, &evaluated[0], args[0].span(), objects)
    }

    fn check_exit_call(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} requires an int argument"),
                args[0].span(),
            ));
        }
        if function_name == "exit" && self.running_atexit {
            return Err(Diagnostic::ub(
                "calling exit from an atexit handler is undefined behavior",
                args[0].span(),
                Some("7.20.4.3"),
            ));
        }
        Ok(())
    }

    fn check_atexit_call(
        &self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("atexit", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "atexit")?;
        let (return_ty, params, is_variadic) =
            self.call_target_signature(&value.ty, args[0].span())?;
        if is_variadic
            || *return_ty != CType::Void
            || !(params.is_empty() || (params.len() == 1 && params[0] == CType::Void))
        {
            return Err(Diagnostic::error(
                "atexit requires an argument of type void (*)(void)",
                args[0].span(),
            ));
        }
        match value.data {
            ValueData::Function(_) => {}
            ValueData::Pointer(ref pointer) if pointer.is_null() => {
                return Err(Diagnostic::ub(
                    "atexit function pointer is null",
                    args[0].span(),
                    Some("7.20.4.2"),
                ));
            }
            _ => {
                return Err(Diagnostic::error(
                    "atexit requires a callable function pointer",
                    args[0].span(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check_sort_search_common(
        &self,
        function_name: &str,
        nmemb: &TypedValue,
        size: &TypedValue,
        nmemb_span: Span,
        size_span: Span,
    ) -> Result<(usize, usize, usize), Diagnostic> {
        if nmemb.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} requires an unsigned long element count"),
                nmemb_span,
            ));
        }
        if size.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} requires an unsigned long element size"),
                size_span,
            ));
        }
        let nmemb = self.checked_usize_from_unsigned_long(nmemb, nmemb_span, "element count")?;
        let size = self.checked_usize_from_unsigned_long(size, size_span, "element size")?;
        if size == 0 {
            return Err(Diagnostic::ub(
                format!("{function_name} element size must be positive"),
                size_span,
                Some("7.22.5"),
            ));
        }
        let total = nmemb.checked_mul(size).ok_or_else(|| {
            Diagnostic::error(
                format!("{function_name} total byte count is out of supported range"),
                size_span,
            )
        })?;
        Ok((nmemb, size, total))
    }

    fn check_qsort_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("qsort", args, evaluated, 4, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "qsort")?;
        }
        if !self.pointer_assignment_compatible(&self.void_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "qsort requires a void * base pointer",
                args[0].span(),
            ));
        }
        let (nmemb, size, total) = self.check_sort_search_common(
            "qsort",
            &evaluated[1],
            &evaluated[2],
            args[1].span(),
            args[2].span(),
        )?;
        let function_symbol =
            self.comparator_function_symbol("qsort", &evaluated[3], args[3].span())?;
        let base = evaluated[0].as_pointer(args[0].span())?;
        let (object_id, start, _) =
            self.ensure_library_array_destination(&base, total, size, args[0].span(), objects)?;
        let signs = self.check_comparator_total_order(
            "qsort",
            &function_symbol,
            object_id,
            start,
            total,
            &base,
            nmemb,
            size,
            span,
            objects,
        )?;
        let mut order = (0..nmemb).collect::<Vec<_>>();
        order.sort_by_key(|&index| (0..nmemb).filter(|&other| signs[other][index] < 0).count());
        self.pending_qsort_order = Some(order);
        Ok(())
    }

    fn check_bsearch_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("bsearch", args, evaluated, 5, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "bsearch")?;
        }
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "bsearch requires a const void * key pointer",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "bsearch requires a const void * base pointer",
                args[1].span(),
            ));
        }
        let (nmemb, size, total) = self.check_sort_search_common(
            "bsearch",
            &evaluated[2],
            &evaluated[3],
            args[2].span(),
            args[3].span(),
        )?;
        let function_symbol =
            self.comparator_function_symbol("bsearch", &evaluated[4], args[4].span())?;
        let key = evaluated[0].as_pointer(args[0].span())?;
        let (key_object, key_start, _) =
            self.library_array_region(&key, size, size, args[0].span(), objects)?;
        let base = evaluated[1].as_pointer(args[1].span())?;
        let (base_object, base_start, _) =
            self.library_array_region(&base, total, size, args[1].span(), objects)?;
        let homogeneous_elements = evaluated[0]
            .ty
            .element_type()
            .zip(evaluated[1].ty.element_type())
            .is_some_and(|(key_ty, element_ty)| {
                key_ty.unqualified() == element_ty.unqualified()
                    && self.type_size_of(element_ty) == Some(size)
            });
        if homogeneous_elements {
            for index in 0..nmemb.saturating_sub(1) {
                let left = self.pointer_with_byte_offset(&base, index * size, span, objects)?;
                let left_start = base_start + index * size;
                let first = self.bsearch_compare_call(
                    &function_symbol,
                    &left,
                    base_object,
                    left_start,
                    &base,
                    base_object,
                    base_start,
                    total,
                    size,
                    index + 1,
                    span,
                    objects,
                )?;
                let second = self.bsearch_compare_call(
                    &function_symbol,
                    &left,
                    base_object,
                    left_start,
                    &base,
                    base_object,
                    base_start,
                    total,
                    size,
                    index + 1,
                    span,
                    objects,
                )?;
                if first.signum() != second.signum() {
                    return Err(Diagnostic::ub(
                        "bsearch comparison function returned inconsistent results for the same adjacent array elements",
                        span,
                        Some("7.22.5.1"),
                    ));
                }
                if first > 0 {
                    return Err(Diagnostic::ub(
                        "bsearch array is not in ascending order according to its comparison function",
                        args[1].span(),
                        Some("7.22.5.1"),
                    ));
                }
            }
        }
        let mut previous = None;
        let mut found = None;
        for index in 0..nmemb {
            let first = self.bsearch_compare_call(
                &function_symbol,
                &key,
                key_object,
                key_start,
                &base,
                base_object,
                base_start,
                total,
                size,
                index,
                span,
                objects,
            )?;
            let second = self.bsearch_compare_call(
                &function_symbol,
                &key,
                key_object,
                key_start,
                &base,
                base_object,
                base_start,
                total,
                size,
                index,
                span,
                objects,
            )?;
            let sign = first.signum() as i8;
            if sign != second.signum() as i8 {
                return Err(Diagnostic::ub(
                    "bsearch comparison function returned inconsistent results for the same key and array element",
                    span,
                    Some("7.22.5.1"),
                ));
            }
            if previous.is_some_and(|previous| previous < sign) {
                return Err(Diagnostic::ub(
                    "bsearch array is not ordered consistently with its comparison function",
                    args[1].span(),
                    Some("7.22.5.1"),
                ));
            }
            if sign == 0 && found.is_none() {
                found = Some(index);
            }
            previous = Some(sign);
        }
        self.pending_bsearch_result = Some(found);
        Ok(())
    }

    fn check_mblen_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mblen", args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mblen")?;
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "mblen requires a const char * argument",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mblen requires an unsigned long length argument",
                args[1].span(),
            ));
        }
        let limit = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "mblen byte count",
        )?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if !pointer.is_null() {
            let _ = self.read_multibyte_source_buffer(&pointer, limit, args[0].span(), objects)?;
        }
        Ok(())
    }

    fn check_mbtowc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbtowc", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mbtowc")?;
        }
        let out = evaluated[0].as_pointer(args[0].span())?;
        if !out.is_null()
            && !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "mbtowc requires a wchar_t * output pointer or null",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "mbtowc requires a const char * input pointer",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mbtowc requires an unsigned long length argument",
                args[2].span(),
            ));
        }
        if !out.is_null() {
            let _ = self.ensure_library_array_destination(
                &out,
                self.wide_char_byte_width(),
                self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?;
        }
        let limit = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbtowc byte count",
        )?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        if !source.is_null() {
            let _ = self.read_multibyte_source_buffer(&source, limit, args[1].span(), objects)?;
        }
        Ok(())
    }

    fn check_wctomb_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wctomb", args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wctomb")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "wctomb requires a char * destination or null",
                args[0].span(),
            ));
        }
        if !dest.is_null() {
            let _ = self.ensure_library_array_destination(
                &dest,
                host_mb_cur_max().max(1),
                1,
                args[0].span(),
                objects,
            )?;
        }
        let _ = self.wchar_scalar_to_host(&evaluated[1], args[1].span(), "wctomb")?;
        Ok(())
    }

    fn check_mbstowcs_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbstowcs", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mbstowcs")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "mbstowcs requires a wchar_t * destination or null",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "mbstowcs requires a const char * source",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mbstowcs requires an unsigned long element count",
                args[2].span(),
            ));
        }
        let limit = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbstowcs element count",
        )?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        let source_bytes = self.read_c_string_bytes(source.clone(), args[1].span(), objects)?;
        let total = limit
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error(
                    "mbstowcs destination size is out of supported range",
                    args[2].span(),
                )
            })?;
        let (object_id, dest_start, _) = self.ensure_library_array_destination(
            &dest,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        if limit != 0 {
            let source_access = source_bytes.len() + 1;
            let (src_object_id, src_start, _) =
                self.byte_region_from_pointer(&source, source_access, args[1].span(), objects)?;
            if self.ranges_overlap_with_sizes(
                object_id,
                dest_start,
                total,
                src_object_id,
                src_start,
                source_access,
            ) {
                return Err(Diagnostic::ub(
                    "mbstowcs source and destination regions overlap",
                    span,
                    Some("7.20.8.1"),
                ));
            }
        }
        Ok(())
    }

    fn check_wcstombs_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcstombs", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcstombs")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "wcstombs requires a char * destination or null",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcstombs requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcstombs requires an unsigned long byte count",
                args[2].span(),
            ));
        }
        let limit = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "wcstombs byte count",
        )?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        let source_units = self.read_wide_string_units(source.clone(), args[1].span(), objects)?;
        let (object_id, dest_start, _) =
            self.ensure_library_array_destination(&dest, limit, 1, args[0].span(), objects)?;
        if limit != 0 {
            let source_access = (source_units.len() + 1)
                .checked_mul(self.wide_char_byte_width())
                .ok_or_else(|| {
                    Diagnostic::error(
                        "wcstombs source size is out of supported range",
                        args[1].span(),
                    )
                })?;
            let (src_object_id, src_start, _) =
                self.byte_region_from_pointer(&source, source_access, args[1].span(), objects)?;
            if self.ranges_overlap_with_sizes(
                object_id,
                dest_start,
                limit,
                src_object_id,
                src_start,
                source_access,
            ) {
                return Err(Diagnostic::ub(
                    "wcstombs source and destination regions overlap",
                    span,
                    Some("7.20.8.2"),
                ));
            }
        }
        Ok(())
    }

    fn check_mbsinit_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbsinit", args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "mbsinit")?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if pointer.is_null() {
            self.pending_heap_call = Some(PendingHeapCall::Free(None));
            return Ok(());
        }
        let expected = self.host_function_parameter_type("mbsinit", span.file, 0, span)?;
        if !self.pointer_assignment_compatible(&expected, &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "mbsinit requires a const mbstate_t * argument or null",
                args[0].span(),
            ));
        }
        self.check_mbstate_for_current_locale("mbsinit", &pointer, false, args[0].span(), objects)?;
        Ok(())
    }

    fn check_mbrlen_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbrlen", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mbrlen")?;
        }
        let source = evaluated[0].as_pointer(args[0].span())?;
        if !source.is_null()
            && !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "mbrlen requires a const char * source or null",
                args[0].span(),
            ));
        }
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mbrlen requires an unsigned long length argument",
                args[1].span(),
            ));
        }
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "mbrlen byte count",
        )?;
        if !source.is_null() {
            let _ = self.library_array_region(&source, n, 1, args[0].span(), objects)?;
        }
        let state = evaluated[2].as_pointer(args[2].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type("mbrlen", span.file, 2, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[2].ty) {
                return Err(Diagnostic::error(
                    "mbrlen requires an mbstate_t * state argument or null",
                    args[2].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale("mbrlen", &state, true, args[2].span(), objects)?;
        Ok(())
    }

    fn check_mbrtowc_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbrtowc", args, evaluated, 4, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mbrtowc")?;
        }
        let out = evaluated[0].as_pointer(args[0].span())?;
        if !out.is_null()
            && !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "mbrtowc requires a wchar_t * output pointer or null",
                args[0].span(),
            ));
        }
        let source = evaluated[1].as_pointer(args[1].span())?;
        if !source.is_null()
            && !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[1].ty)
        {
            return Err(Diagnostic::error(
                "mbrtowc requires a const char * source or null",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mbrtowc requires an unsigned long length argument",
                args[2].span(),
            ));
        }
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbrtowc byte count",
        )?;
        if !source.is_null() {
            let _ = self.library_array_region(&source, n, 1, args[1].span(), objects)?;
        }
        if !out.is_null() {
            let _ = self.ensure_library_array_destination(
                &out,
                self.wide_char_byte_width(),
                self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?;
        }
        let state = evaluated[3].as_pointer(args[3].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type("mbrtowc", span.file, 3, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[3].ty) {
                return Err(Diagnostic::error(
                    "mbrtowc requires an mbstate_t * state argument or null",
                    args[3].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale("mbrtowc", &state, true, args[3].span(), objects)?;
        Ok(())
    }

    fn check_wcrtomb_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcrtomb", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcrtomb")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "wcrtomb requires a char * destination or null",
                args[0].span(),
            ));
        }
        let _ = self.wchar_scalar_to_host(&evaluated[1], args[1].span(), "wcrtomb")?;
        if !dest.is_null() {
            let bytes = host_mb_cur_max().max(1);
            let _ =
                self.ensure_library_array_destination(&dest, bytes, 1, args[0].span(), objects)?;
        }
        let state = evaluated[2].as_pointer(args[2].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type("wcrtomb", span.file, 2, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[2].ty) {
                return Err(Diagnostic::error(
                    "wcrtomb requires an mbstate_t * state argument or null",
                    args[2].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale("wcrtomb", &state, true, args[2].span(), objects)?;
        Ok(())
    }

    fn check_mbrtoc_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 4, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), function_name)?;
        }
        let out = evaluated[0].as_pointer(args[0].span())?;
        let expected_out = self.host_function_parameter_type(function_name, span.file, 0, span)?;
        if !out.is_null() && !self.pointer_assignment_compatible(&expected_out, &evaluated[0].ty) {
            return Err(Diagnostic::error(
                format!("{function_name} output pointer has incompatible type"),
                args[0].span(),
            ));
        }
        if !out.is_null() {
            let output_ty = expected_out.element_type().expect("pointer parameter");
            let size = self
                .type_size_of(output_ty)
                .expect("complete character type");
            let _ =
                self.ensure_library_array_destination(&out, size, size, args[0].span(), objects)?;
        }
        let source = evaluated[1].as_pointer(args[1].span())?;
        if !source.is_null()
            && !self.pointer_assignment_compatible(&self.const_char_ptr_type(), &evaluated[1].ty)
        {
            return Err(Diagnostic::error(
                format!("{function_name} requires a const char * source"),
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} length must have type unsigned long"),
                args[2].span(),
            ));
        }
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "multibyte byte count",
        )?;
        if !source.is_null() {
            let _ = self.library_array_region(&source, n, 1, args[1].span(), objects)?;
        }
        let state = evaluated[3].as_pointer(args[3].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type(function_name, span.file, 3, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[3].ty) {
                return Err(Diagnostic::error(
                    format!("{function_name} state pointer has incompatible type"),
                    args[3].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale(
            function_name,
            &state,
            true,
            args[3].span(),
            objects,
        )?;
        Ok(())
    }

    fn check_c_rtomb_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), function_name)?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                format!("{function_name} requires a char * destination"),
                args[0].span(),
            ));
        }
        let expected_scalar =
            self.host_function_parameter_type(function_name, span.file, 1, span)?;
        if evaluated[1].ty != expected_scalar {
            return Err(Diagnostic::error(
                format!("{function_name} character argument has incompatible type"),
                args[1].span(),
            ));
        }
        if !dest.is_null() {
            let _ = self.ensure_library_array_destination(
                &dest,
                host_mb_cur_max().max(1),
                1,
                args[0].span(),
                objects,
            )?;
        }
        let state = evaluated[2].as_pointer(args[2].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type(function_name, span.file, 2, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[2].ty) {
                return Err(Diagnostic::error(
                    format!("{function_name} state pointer has incompatible type"),
                    args[2].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale(
            function_name,
            &state,
            true,
            args[2].span(),
            objects,
        )?;
        Ok(())
    }

    fn check_wcsto_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let expects_base = matches!(function_name, "wcstol" | "wcstoll" | "wcstoul" | "wcstoull");
        let expected_args = if expects_base { 3 } else { 2 };
        self.require_exact_call_args(function_name, args, evaluated, expected_args, span)?;
        self.check_wide_string_arg(function_name, &evaluated[0], args[0].span(), objects)?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), function_name)?;
        if !self.pointer_assignment_compatible(
            &CType::pointer_to(self.wchar_ptr_type()),
            &evaluated[1].ty,
        ) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wchar_t ** end-pointer argument"),
                args[1].span(),
            ));
        }
        let end_pointer = evaluated[1].as_pointer(args[1].span())?;
        if !end_pointer.is_null() {
            let size = self.type_size_of(&self.wchar_ptr_type()).unwrap_or(8);
            let (object_id, start, _) =
                self.byte_region_from_pointer(&end_pointer, size, args[1].span(), objects)?;
            self.ensure_library_writable_region(object_id, start, size, args[1].span(), objects)?;
        }
        if expects_base {
            self.check_library_argument_value(&evaluated[2], args[2].span(), function_name)?;
            if evaluated[2].ty != CType::Int {
                return Err(Diagnostic::error(
                    format!("{function_name} requires an int base argument"),
                    args[2].span(),
                ));
            }
            let base = evaluated[2].to_int()?;
            if base != 0 && !(2..=36).contains(&base) {
                return Err(Diagnostic::ub(
                    format!("{function_name} base must be 0 or between 2 and 36"),
                    args[2].span(),
                    Some("7.24.4.1"),
                ));
            }
        }
        Ok(())
    }

    fn check_mbsrtowcs_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("mbsrtowcs", args, evaluated, 4, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "mbsrtowcs")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "mbsrtowcs requires a wchar_t * destination or null",
                args[0].span(),
            ));
        }
        let srcpp_ty = self.host_function_parameter_type("mbsrtowcs", span.file, 1, span)?;
        if !self.pointer_assignment_compatible(&srcpp_ty, &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "mbsrtowcs requires a const char ** source-pointer argument",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "mbsrtowcs requires an unsigned long length argument",
                args[2].span(),
            ));
        }
        let len = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbsrtowcs element count",
        )?;
        let total = len
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error(
                    "mbsrtowcs destination size is out of supported range",
                    args[2].span(),
                )
            })?;
        let dest_region = if dest.is_null() {
            None
        } else {
            Some(self.ensure_library_array_destination(
                &dest,
                total,
                self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?)
        };
        let srcpp = evaluated[1].as_pointer(args[1].span())?;
        let pointer_size = self
            .type_size_of(
                srcpp_ty
                    .element_type()
                    .expect("source pointer-to-pointer parameter"),
            )
            .unwrap_or(8);
        let (srcpp_object, srcpp_start, _) =
            self.byte_region_from_pointer(&srcpp, pointer_size, args[1].span(), objects)?;
        self.ensure_library_writable_region(
            srcpp_object,
            srcpp_start,
            pointer_size,
            args[1].span(),
            objects,
        )?;
        let src_lvalue = self.pointer_lvalue(
            "mbsrtowcs",
            &srcpp,
            srcpp_ty.element_type().expect("pointer parameter"),
            args[1].span(),
            "7.24.6.4.1",
        )?;
        let current = self.load_lvalue(src_lvalue, args[1].span(), objects)?;
        let current_ptr = current.as_pointer(args[1].span())?;
        if !current_ptr.is_null() {
            let source_bytes =
                self.read_c_string_bytes(current_ptr.clone(), args[1].span(), objects)?;
            if let Some((object_id, dest_start, _)) = dest_region {
                let source_access = source_bytes.len() + 1;
                let (src_object_id, src_start, _) = self.byte_region_from_pointer(
                    &current_ptr,
                    source_access,
                    args[1].span(),
                    objects,
                )?;
                if self.ranges_overlap_with_sizes(
                    object_id,
                    dest_start,
                    total,
                    src_object_id,
                    src_start,
                    source_access,
                ) {
                    return Err(Diagnostic::ub(
                        "mbsrtowcs source and destination regions overlap",
                        span,
                        Some("7.24.6.4.1"),
                    ));
                }
            }
        }
        let state = evaluated[3].as_pointer(args[3].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type("mbsrtowcs", span.file, 3, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[3].ty) {
                return Err(Diagnostic::error(
                    "mbsrtowcs requires an mbstate_t * state argument or null",
                    args[3].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale("mbsrtowcs", &state, true, args[3].span(), objects)?;
        Ok(())
    }

    fn check_wcsrtombs_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcsrtombs", args, evaluated, 4, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcsrtombs")?;
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        if !dest.is_null()
            && !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty)
        {
            return Err(Diagnostic::error(
                "wcsrtombs requires a char * destination or null",
                args[0].span(),
            ));
        }
        let srcpp_ty = self.host_function_parameter_type("wcsrtombs", span.file, 1, span)?;
        if !self.pointer_assignment_compatible(&srcpp_ty, &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcsrtombs requires a const wchar_t ** source-pointer argument",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcsrtombs requires an unsigned long length argument",
                args[2].span(),
            ));
        }
        let len = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "wcsrtombs byte count",
        )?;
        let dest_region = if dest.is_null() {
            None
        } else {
            Some(self.ensure_library_array_destination(&dest, len, 1, args[0].span(), objects)?)
        };
        let srcpp = evaluated[1].as_pointer(args[1].span())?;
        let pointer_size = self
            .type_size_of(
                srcpp_ty
                    .element_type()
                    .expect("source pointer-to-pointer parameter"),
            )
            .unwrap_or(8);
        let (srcpp_object, srcpp_start, _) =
            self.byte_region_from_pointer(&srcpp, pointer_size, args[1].span(), objects)?;
        self.ensure_library_writable_region(
            srcpp_object,
            srcpp_start,
            pointer_size,
            args[1].span(),
            objects,
        )?;
        let src_lvalue = self.pointer_lvalue(
            "wcsrtombs",
            &srcpp,
            srcpp_ty.element_type().expect("pointer parameter"),
            args[1].span(),
            "7.24.6.4.2",
        )?;
        let current = self.load_lvalue(src_lvalue, args[1].span(), objects)?;
        let current_ptr = current.as_pointer(args[1].span())?;
        if !current_ptr.is_null() {
            let source_units =
                self.read_wide_string_units(current_ptr.clone(), args[1].span(), objects)?;
            if let Some((object_id, dest_start, _)) = dest_region {
                let source_access = (source_units.len() + 1)
                    .checked_mul(self.wide_char_byte_width())
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "wcsrtombs source size is out of supported range",
                            args[1].span(),
                        )
                    })?;
                let (src_object_id, src_start, _) = self.byte_region_from_pointer(
                    &current_ptr,
                    source_access,
                    args[1].span(),
                    objects,
                )?;
                if self.ranges_overlap_with_sizes(
                    object_id,
                    dest_start,
                    len,
                    src_object_id,
                    src_start,
                    source_access,
                ) {
                    return Err(Diagnostic::ub(
                        "wcsrtombs source and destination regions overlap",
                        span,
                        Some("7.24.6.4.2"),
                    ));
                }
            }
        }
        let state = evaluated[3].as_pointer(args[3].span())?;
        if !state.is_null() {
            let expected = self.host_function_parameter_type("wcsrtombs", span.file, 3, span)?;
            if !self.pointer_assignment_compatible(&expected, &evaluated[3].ty) {
                return Err(Diagnostic::error(
                    "wcsrtombs requires an mbstate_t * state argument or null",
                    args[3].span(),
                ));
            }
        }
        self.check_mbstate_for_current_locale("wcsrtombs", &state, true, args[3].span(), objects)?;
        Ok(())
    }

    fn check_wcscpy_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcscpy", args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcscpy")?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcscpy requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcscpy requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        let src_units = self.read_wide_string_units(src.clone(), args[1].span(), objects)?;
        let copy_size = (src_units.len() + 1)
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcscpy copy size is out of supported range", span))?;
        let (dest_object, dest_start, _) =
            self.byte_region_from_pointer(&dest, copy_size, args[0].span(), objects)?;
        self.ensure_library_writable_region(
            dest_object,
            dest_start,
            copy_size,
            args[0].span(),
            objects,
        )?;
        let (src_object, src_start, _) =
            self.byte_region_from_pointer(&src, copy_size, args[1].span(), objects)?;
        if self.ranges_overlap(dest_object, dest_start, src_object, src_start, copy_size) {
            return Err(Diagnostic::ub(
                "wcscpy source and destination regions overlap",
                span,
                Some("7.24.4.1"),
            ));
        }
        Ok(())
    }

    fn check_wcsncpy_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcsncpy", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcsncpy")?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcsncpy requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcsncpy requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcsncpy requires an unsigned long count argument",
                args[2].span(),
            ));
        }
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsncpy count")?;
        let total = count
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcsncpy size is out of supported range", span))?;
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        let width = self.wide_char_byte_width();
        let (dest_object, dest_start, _) =
            self.ensure_library_array_destination(&dest, total, width, args[0].span(), objects)?;
        let (src_object, src_start, _) =
            self.library_array_region(&src, total, width, args[1].span(), objects)?;
        if self.ranges_overlap(dest_object, dest_start, src_object, src_start, total) {
            return Err(Diagnostic::ub(
                "wcsncpy source and destination regions overlap",
                span,
                Some("7.24.4.2"),
            ));
        }
        Ok(())
    }

    fn check_wide_memory_copy_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), function_name)?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wchar_t * destination"),
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a const wchar_t * source"),
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} requires an unsigned long count argument"),
                args[2].span(),
            ));
        }
        let count = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            &format!("{function_name} count"),
        )?;
        let total = count
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error(
                    format!("{function_name} size is out of supported range"),
                    span,
                )
            })?;
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        let width = self.wide_char_byte_width();
        let (dest_object, dest_start, _) =
            self.ensure_library_array_destination(&dest, total, width, args[0].span(), objects)?;
        let (src_object, src_start, _) =
            self.library_array_region(&src, total, width, args[1].span(), objects)?;
        if function_name == "wmemcpy"
            && self.ranges_overlap(dest_object, dest_start, src_object, src_start, total)
        {
            return Err(Diagnostic::ub(
                "wmemcpy source and destination regions overlap",
                span,
                Some("7.24.4.4"),
            ));
        }
        Ok(())
    }

    fn check_wcscat_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcscat", args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcscat")?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcscat requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcscat requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        let dest_units = self.read_wide_string_units(dest.clone(), args[0].span(), objects)?;
        let src_units = self.read_wide_string_units(src.clone(), args[1].span(), objects)?;
        let total = (dest_units.len() + src_units.len() + 1)
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcscat size is out of supported range", span))?;
        let (dest_object, dest_start, _) = self.ensure_library_array_destination(
            &dest,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        let src_access = (src_units.len() + 1)
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error("wcscat source size is out of supported range", span)
            })?;
        let (src_object, src_start, _) =
            self.byte_region_from_pointer(&src, src_access, args[1].span(), objects)?;
        if self.ranges_overlap_with_sizes(
            dest_object,
            dest_start + dest_units.len() * self.wide_char_byte_width(),
            src_access,
            src_object,
            src_start,
            src_access,
        ) {
            return Err(Diagnostic::ub(
                "wcscat source and destination regions overlap",
                span,
                Some("7.24.4.3"),
            ));
        }
        Ok(())
    }

    fn check_wcsncat_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcsncat", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcsncat")?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcsncat requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcsncat requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcsncat requires an unsigned long count argument",
                args[2].span(),
            ));
        }
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsncat count")?;
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        let bounded_source_size = count
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcsncat source size is out of range", span))?;
        let _ = self.library_array_region(
            &src,
            bounded_source_size,
            self.wide_char_byte_width(),
            args[1].span(),
            objects,
        )?;
        let dest_units = self.read_wide_string_units(dest.clone(), args[0].span(), objects)?;
        let src_buffer =
            self.read_bounded_wide_source(src.clone(), count, args[1].span(), objects)?;
        let nul_pos = src_buffer.iter().position(|&unit| unit == 0);
        let copy_len = nul_pos.unwrap_or(count);
        let src_access_units = nul_pos.map_or(count, |pos| pos + 1);
        let total = (dest_units.len() + copy_len + 1)
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcsncat size is out of supported range", span))?;
        let (dest_object, dest_start, _) =
            self.byte_region_from_pointer(&dest, total, args[0].span(), objects)?;
        self.ensure_library_writable_region(
            dest_object,
            dest_start,
            total,
            args[0].span(),
            objects,
        )?;
        let src_access = src_access_units
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error("wcsncat source size is out of supported range", span)
            })?;
        let (src_object, src_start, _) =
            self.byte_region_from_pointer(&src, src_access, args[1].span(), objects)?;
        if self.ranges_overlap_with_sizes(
            dest_object,
            dest_start + dest_units.len() * self.wide_char_byte_width(),
            src_access,
            src_object,
            src_start,
            src_access,
        ) {
            return Err(Diagnostic::ub(
                "wcsncat source and destination regions overlap",
                span,
                Some("7.24.4.3"),
            ));
        }
        Ok(())
    }

    fn check_wcsxfrm_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("wcsxfrm", args, evaluated, 3, span)?;
        for (expr, value) in args.iter().zip(evaluated) {
            self.check_library_argument_value(value, expr.span(), "wcsxfrm")?;
        }
        if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "wcsxfrm requires a wchar_t * destination",
                args[0].span(),
            ));
        }
        if !self.pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                "wcsxfrm requires a const wchar_t * source",
                args[1].span(),
            ));
        }
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "wcsxfrm requires an unsigned long count argument",
                args[2].span(),
            ));
        }
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsxfrm count")?;
        let total = count
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wcsxfrm size is out of supported range", span))?;
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let src = evaluated[1].as_pointer(args[1].span())?;
        if count == 0 && dest.is_null() {
            self.read_wide_string_units(src, args[1].span(), objects)?;
            return Ok(());
        }
        let (dest_object, dest_start, _) = self.ensure_library_array_destination(
            &dest,
            total,
            self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        let src_units = self.read_wide_string_units(src.clone(), args[1].span(), objects)?;
        let src_access = (src_units.len() + 1)
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| {
                Diagnostic::error("wcsxfrm source size is out of supported range", span)
            })?;
        let (src_object, src_start, _) =
            self.byte_region_from_pointer(&src, src_access, args[1].span(), objects)?;
        if self.ranges_overlap_with_sizes(
            dest_object,
            dest_start,
            total,
            src_object,
            src_start,
            src_access,
        ) {
            return Err(Diagnostic::ub(
                "wcsxfrm source and destination regions overlap",
                span,
                Some("7.24.4.6"),
            ));
        }
        Ok(())
    }

    fn check_basic_wide_string_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if args.len() != evaluated.len() {
            return Err(Diagnostic::error(
                format!("{function_name} received an invalid argument list"),
                span,
            ));
        }
        match args.len() {
            1 => {
                self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
                self.check_wide_string_arg(function_name, &evaluated[0], args[0].span(), objects)
            }
            2 => {
                for (expr, value) in args.iter().zip(evaluated) {
                    self.check_library_argument_value(value, expr.span(), function_name)?;
                }
                self.check_wide_string_arg(function_name, &evaluated[0], args[0].span(), objects)?;
                self.check_wide_string_arg(function_name, &evaluated[1], args[1].span(), objects)
            }
            _ => Err(Diagnostic::error(
                format!("{function_name} received an invalid argument count"),
                span,
            )),
        }
    }

    fn check_wide_string_n_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 3, span)?;
        for index in 0..2 {
            self.check_library_argument_value(
                &evaluated[index],
                args[index].span(),
                function_name,
            )?;
            if !self
                .pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[index].ty)
            {
                return Err(Diagnostic::error(
                    format!("{function_name} requires const wchar_t * input pointers"),
                    args[index].span(),
                ));
            }
        }
        self.check_library_argument_value(&evaluated[2], args[2].span(), function_name)?;
        if evaluated[2].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} requires an unsigned long count argument"),
                args[2].span(),
            ));
        }
        let count = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            &format!("{function_name} count"),
        )?;
        let total = count
            .checked_mul(self.wide_char_byte_width())
            .ok_or_else(|| Diagnostic::error("wide memory access is out of range", span))?;
        for index in 0..2 {
            let pointer = evaluated[index].as_pointer(args[index].span())?;
            let _ = self.library_array_region(
                &pointer,
                total,
                self.wide_char_byte_width(),
                args[index].span(),
                objects,
            )?;
        }
        Ok(())
    }

    fn check_wide_char_search_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        self.check_basic_wide_string_call(
            function_name,
            &args[..1],
            &evaluated[..1],
            span,
            objects,
        )?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), function_name)?;
        if evaluated[1].ty != self.wchar_type() {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wchar_t character argument"),
                args[1].span(),
            ));
        }
        Ok(())
    }

    fn check_wint_arg(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} requires a wint_t argument"),
                span,
            ));
        }
        Ok(())
    }

    fn check_unary_wint_call(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
        self.check_wint_arg(function_name, &evaluated[0], args[0].span())
    }

    fn check_wctrans_lookup_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_unary_string_call("wctrans", args, evaluated, span, objects)
    }

    fn check_wctype_lookup_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_unary_string_call("wctype", args, evaluated, span, objects)
    }

    fn check_iswctype_call(
        &self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("iswctype", args, evaluated, 2, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "iswctype")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "iswctype")?;
        self.check_wint_arg("iswctype", &evaluated[0], args[0].span())?;
        if evaluated[1].ty != CType::UnsignedInt {
            return Err(Diagnostic::error(
                "iswctype requires a wctype_t descriptor",
                args[1].span(),
            ));
        }
        let _ = self.validated_wctype_descriptor(evaluated[1].to_int()?, args[1].span())?;
        Ok(())
    }

    fn check_towctrans_call(
        &self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("towctrans", args, evaluated, 2, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "towctrans")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "towctrans")?;
        self.check_wint_arg("towctrans", &evaluated[0], args[0].span())?;
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "towctrans requires a wctrans_t descriptor",
                args[1].span(),
            ));
        }
        let _ = self.validated_wctrans_descriptor(evaluated[1].to_int()?, args[1].span())?;
        Ok(())
    }

    fn check_assert_fail_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("__codex_assert_fail", args, evaluated, 4, span)?;
        self.check_math_string_arg(
            "__codex_assert_fail",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        self.check_math_string_arg(
            "__codex_assert_fail",
            &evaluated[1],
            args[1].span(),
            objects,
        )?;
        self.check_library_argument_value(&evaluated[2], args[2].span(), "__codex_assert_fail")?;
        if evaluated[2].ty != CType::Int {
            return Err(Diagnostic::error(
                "__codex_assert_fail requires an int line number",
                args[2].span(),
            ));
        }
        self.check_math_string_arg(
            "__codex_assert_fail",
            &evaluated[3],
            args[3].span(),
            objects,
        )?;
        Ok(())
    }

    fn check_ctype_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), function_name)?;
        if value.ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} requires an int argument"),
                args[0].span(),
            ));
        }
        let ch = value.to_int()?;
        if ch != HOST_EOF as i128 && !(0..=u8::MAX as i128).contains(&ch) {
            return Err(Diagnostic::ub(
                format!("{function_name} requires EOF or a value representable as unsigned char"),
                args[0].span(),
                Some("7.4"),
            ));
        }
        Ok(())
    }

    fn check_srand_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("srand", args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "srand")?;
        if evaluated[0].ty != CType::UnsignedInt {
            return Err(Diagnostic::error(
                "srand requires an unsigned int argument",
                args[0].span(),
            ));
        }
        Ok(())
    }

    fn check_binary_integer_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        expected: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
        for (value, expr) in evaluated.iter().zip(args.iter()) {
            self.check_library_argument_value(value, expr.span(), function_name)?;
            if &value.ty != expected {
                return Err(Diagnostic::error(
                    format!("{function_name} requires arguments of type {expected}"),
                    expr.span(),
                ));
            }
        }
        let numer = evaluated[0].to_int()?;
        let denom = evaluated[1].to_int()?;
        let _ = self.integer_div_for_type(numer, denom, expected, args[1].span())?;
        let _ = self.integer_rem_for_type(numer, denom, expected, args[1].span())?;
        Ok(())
    }

    fn check_strto_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let expects_base = matches!(
            function_name,
            "strtol" | "strtoul" | "strtoll" | "strtoull" | "strtoimax" | "strtoumax"
        );
        let expected_args = if expects_base { 3 } else { 2 };
        self.require_exact_call_args(function_name, args, evaluated, expected_args, span)?;
        self.check_math_string_arg(function_name, &evaluated[0], args[0].span(), objects)?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), function_name)?;
        if !self.pointer_assignment_compatible(&self.char_ptr_ptr_type(), &evaluated[1].ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a char ** end-pointer argument"),
                args[1].span(),
            ));
        }
        let end_pointer = evaluated[1].as_pointer(args[1].span())?;
        if !end_pointer.is_null() {
            let size = self.type_size_of(&self.char_ptr_type()).unwrap_or(8);
            let (object_id, start, _) =
                self.byte_region_from_pointer(&end_pointer, size, args[1].span(), objects)?;
            self.ensure_library_writable_region(object_id, start, size, args[1].span(), objects)?;
        }
        if expects_base {
            self.check_library_argument_value(&evaluated[2], args[2].span(), function_name)?;
            if evaluated[2].ty != CType::Int {
                return Err(Diagnostic::error(
                    format!("{function_name} requires an int base argument"),
                    args[2].span(),
                ));
            }
            let base = evaluated[2].to_int()?;
            if base != 0 && !(2..=36).contains(&base) {
                return Err(Diagnostic::ub(
                    format!("{function_name} base must be 0 or between 2 and 36"),
                    args[2].span(),
                    Some("7.20.1.4"),
                ));
            }
        }
        Ok(())
    }

    fn check_system_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args("system", args, evaluated)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "system")?;
        if let ValueData::Pointer(pointer) = &value.data
            && pointer.is_null()
        {
            return Ok(());
        }
        self.check_math_string_arg("system", value, args[0].span(), objects)
    }

    fn check_setlocale_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("setlocale", args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "setlocale")?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                "setlocale requires an int category argument",
                args[0].span(),
            ));
        }
        let category = evaluated[0].to_int()?;
        if !matches!(category, 0..=5) {
            return Err(Diagnostic::ub(
                "setlocale category is not one of the required LC_* macros",
                args[0].span(),
                Some("7.11.1.1"),
            ));
        }
        self.check_library_argument_value(&evaluated[1], args[1].span(), "setlocale")?;
        let locale_pointer = evaluated[1].as_pointer(args[1].span())?;
        if !locale_pointer.is_null() {
            self.check_math_string_arg("setlocale", &evaluated[1], args[1].span(), objects)?;
        }
        Ok(())
    }

    fn check_signal_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("signal", args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "signal")?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                "signal requires an int signal number",
                args[0].span(),
            ));
        }
        let signal = evaluated[0].to_int()?;
        if !matches!(signal, 2 | 4 | 6 | 8 | 11 | 15) {
            return Err(Diagnostic::ub(
                "signal number is not one of the required ISO C signal macros",
                args[0].span(),
                Some("7.14"),
            ));
        }
        self.check_library_argument_value(&evaluated[1], args[1].span(), "signal")?;
        let handler_ty = self.host_function_return_type("signal", span.file, span)?;
        if evaluated[1].ty != handler_ty {
            return Err(Diagnostic::error(
                "signal requires a compatible signal handler pointer",
                args[1].span(),
            ));
        }
        Ok(())
    }

    fn check_raise_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("raise", args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "raise")?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                "raise requires an int signal number",
                args[0].span(),
            ));
        }
        let signal = evaluated[0].to_int()?;
        if !matches!(signal, 2 | 4 | 6 | 8 | 11 | 15) {
            return Err(Diagnostic::ub(
                "raise signal number is not one of the required ISO C signal macros",
                args[0].span(),
                Some("7.14.2.1"),
            ));
        }
        Ok(())
    }

    fn check_signal_sentinel_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), function_name)?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} requires an int signal number"),
                args[0].span(),
            ));
        }
        Ok(())
    }

    fn check_difftime_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("difftime", args, evaluated, 2, span)?;
        for (expr, value) in args.iter().zip(evaluated.iter()) {
            self.check_library_argument_value(value, expr.span(), "difftime")?;
            if value.ty != CType::Long {
                return Err(Diagnostic::error(
                    "difftime requires arguments of type time_t",
                    expr.span(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check_tm_pointer_input(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let tm_ty = self.tm_type_for_call(span)?;
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a valid pointer to struct tm"),
                span,
                Some("7.23"),
            ));
        }
        let Some(pointee_ty) = value.ty.element_type().cloned() else {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type struct tm*"),
                span,
            ));
        };
        if pointee_ty.unqualified() != tm_ty.unqualified() {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type struct tm*"),
                span,
            ));
        }
        let size = self
            .type_size_of(&tm_ty)
            .ok_or_else(|| Diagnostic::error("struct tm type is incomplete", span))?;
        let _ = self.byte_region_from_pointer(&pointer, size, span, objects)?;
        Ok(tm_ty)
    }

    fn check_time_t_pointer_input(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
        allow_null: bool,
        writable: bool,
    ) -> Result<(), Diagnostic> {
        self.check_library_argument_value(value, span, function_name)?;
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            if allow_null {
                return Ok(());
            }
            return Err(Diagnostic::ub(
                format!("{function_name} requires a valid time_t pointer"),
                span,
                Some("7.23"),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(function_name, value, span, objects)?;
        let expected = if writable {
            CType::pointer_to(CType::Long)
        } else {
            CType::pointer_to(CType::qualified(
                CType::Long,
                crate::types::TypeQualifiers {
                    is_atomic: false,
                    is_const: true,
                    is_restrict: false,
                    is_volatile: false,
                },
            ))
        };
        if !self.pointer_assignment_compatible(&expected, &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires an argument of type {expected}"),
                span,
            ));
        }
        let (object_id, start, _) = self.byte_region_from_pointer(&pointer, 8, span, objects)?;
        if writable {
            self.ensure_library_writable_region(object_id, start, 8, span, objects)?;
        }
        Ok(())
    }

    fn check_mktime_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args("mktime", args, evaluated)?;
        let tm_ty =
            self.check_tm_pointer_input("mktime", &evaluated[0], args[0].span(), objects)?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let size = self.type_size_of(&tm_ty).unwrap();
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, size, args[0].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, size, args[0].span(), objects)
    }

    fn check_time_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("time", args, evaluated, 1, span)?;
        self.check_time_t_pointer_input("time", &evaluated[0], args[0].span(), objects, true, true)
    }

    fn check_timespec_get_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("timespec_get", args, evaluated, 2, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "timespec_get")?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let pointee = evaluated[0].ty.element_type().cloned().ok_or_else(|| {
            Diagnostic::error(
                "timespec_get requires a struct timespec pointer",
                args[0].span(),
            )
        })?;
        let record = self.record_type(&pointee).ok_or_else(|| {
            Diagnostic::error(
                "timespec_get requires a struct timespec pointer",
                args[0].span(),
            )
        })?;
        if record.tag.as_deref() != Some("timespec") {
            return Err(Diagnostic::error(
                "timespec_get requires a struct timespec pointer",
                args[0].span(),
            ));
        }
        let size = self
            .type_size_of(&pointee)
            .ok_or_else(|| Diagnostic::error("struct timespec is incomplete", args[0].span()))?;
        let (object, start, _) =
            self.byte_region_from_pointer(&pointer, size, args[0].span(), objects)?;
        self.ensure_library_writable_region(object, start, size, args[0].span(), objects)?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "timespec_get")?;
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "timespec_get base argument must have type int",
                args[1].span(),
            ));
        }
        Ok(())
    }

    fn check_asctime_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args("asctime", args, evaluated)?;
        let _ = self.check_tm_pointer_input("asctime", &evaluated[0], args[0].span(), objects)?;
        let tm =
            self.load_host_tm_from_pointer("asctime", &evaluated[0], args[0].span(), objects)?;
        let year = i64::from(tm.tm_year) + 1900;
        let valid = (0..=60).contains(&tm.tm_sec)
            && (0..=59).contains(&tm.tm_min)
            && (0..=23).contains(&tm.tm_hour)
            && (1..=31).contains(&tm.tm_mday)
            && (0..=11).contains(&tm.tm_mon)
            && (1000..=9999).contains(&year)
            && (0..=6).contains(&tm.tm_wday)
            && (0..=365).contains(&tm.tm_yday);
        if !valid {
            return Err(Diagnostic::ub(
                "asctime requires struct tm members in their normal ranges and a year from 1000 through 9999",
                args[0].span(),
                Some("7.27.3.1"),
            ));
        }
        Ok(())
    }

    fn check_ctime_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_time_t_pointer_input(
            function_name,
            &evaluated[0],
            args[0].span(),
            objects,
            false,
            false,
        )
    }

    fn check_strftime_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("strftime", args, evaluated, 4, span)?;
        self.check_library_argument_value(&evaluated[0], args[0].span(), "strftime")?;
        self.check_library_argument_value(&evaluated[1], args[1].span(), "strftime")?;
        if evaluated[1].ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                "strftime requires a size_t maxsize argument",
                args[1].span(),
            ));
        }
        let maxsize = usize::try_from(evaluated[1].to_int()?).map_err(|_| {
            Diagnostic::error("strftime maxsize is out of supported range", args[1].span())
        })?;
        if !self.pointer_assignment_compatible(&self.char_ptr_type(), &evaluated[0].ty) {
            return Err(Diagnostic::error(
                "strftime requires a char * destination",
                args[0].span(),
            ));
        }
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let _ =
            self.ensure_library_array_destination(&dest, maxsize, 1, args[0].span(), objects)?;
        self.check_math_string_arg("strftime", &evaluated[2], args[2].span(), objects)?;
        let format = self.read_c_string_bytes(
            evaluated[2].as_pointer(args[2].span())?,
            args[2].span(),
            objects,
        )?;
        let format = format.into_iter().map(u32::from).collect::<Vec<_>>();
        self.validate_strftime_conversion_specifiers("strftime", &format, args[2].span())?;
        let _ = self.check_tm_pointer_input("strftime", &evaluated[3], args[3].span(), objects)?;
        Ok(())
    }

    fn check_fenv_mask_value(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.ty != CType::Int {
            return Err(Diagnostic::error(
                format!("{function_name} requires an int exception-mask argument"),
                span,
            ));
        }
        let mask = value.to_int()?;
        if mask & !i128::from(HOST_FE_ALL_EXCEPT) != 0 {
            return Err(Diagnostic::ub(
                format!("{function_name} exception mask contains unsupported bits"),
                span,
                Some("7.6"),
            ));
        }
        Ok(())
    }

    fn check_fenv_mask_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args(function_name, args, evaluated)?;
        self.check_fenv_mask_value(function_name, &evaluated[0], args[0].span())
    }

    fn check_fegetexceptflag_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fegetexceptflag", args, evaluated, 2, span)?;
        self.check_math_common_args("fegetexceptflag", args, evaluated)?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                "fegetexceptflag requires a writable fexcept_t pointer",
                args[0].span(),
                Some("7.6.2.2"),
            ));
        }
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 2, args[0].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, 2, args[0].span(), objects)?;
        self.check_fenv_mask_value("fegetexceptflag", &evaluated[1], args[1].span())
    }

    fn check_fesetexceptflag_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("fesetexceptflag", args, evaluated, 2, span)?;
        self.check_math_common_args("fesetexceptflag", args, evaluated)?;
        self.reject_nonvolatile_library_pointer_access(
            "fesetexceptflag",
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                "fesetexceptflag requires a valid fexcept_t pointer",
                args[0].span(),
                Some("7.6.2.4"),
            ));
        }
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 2, args[0].span(), objects)?;
        self.check_fenv_mask_value("fesetexceptflag", &evaluated[1], args[1].span())?;
        let requested_mask = evaluated[1].to_int()?;
        let current_version = self
            .lookup_object(objects, object_id)
            .expect("validated fexcept_t object")
            .modification_count;
        let valid = self
            .fexcept_provenance
            .get(&(object_id, start))
            .is_some_and(|&(version, saved_mask)| {
                version == current_version && requested_mask & !saved_mask == 0
            });
        if !valid {
            return Err(Diagnostic::ub(
                "fesetexceptflag requires an unmodified fexcept_t value produced by fegetexceptflag for all requested exceptions",
                args[0].span(),
                Some("7.6.2.4"),
            ));
        }
        Ok(())
    }

    fn check_fesetround_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        _span: Span,
    ) -> Result<(), Diagnostic> {
        self.check_math_common_args("fesetround", args, evaluated)?;
        if evaluated[0].ty != CType::Int {
            return Err(Diagnostic::error(
                "fesetround requires an int argument",
                args[0].span(),
            ));
        }
        Ok(())
    }

    fn check_fenv_env_output_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_math_common_args(function_name, args, evaluated)?;
        self.reject_nonvolatile_library_pointer_access(
            function_name,
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a writable fenv_t pointer"),
                args[0].span(),
                Some("7.6.4"),
            ));
        }
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 16, args[0].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, 16, args[0].span(), objects)
    }

    fn check_fenv_env_input_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
        self.check_math_common_args(function_name, args, evaluated)?;
        self.reject_nonvolatile_library_pointer_access(
            function_name,
            &evaluated[0],
            args[0].span(),
            objects,
        )?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!("{function_name} requires FE_DFL_ENV or a saved fenv_t object"),
                args[0].span(),
                Some("7.6.4"),
            ));
        }
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 16, args[0].span(), objects)?;
        if self.fe_dfl_env_binding == Some(object_id) && start == 0 {
            return Ok(());
        }
        let current_version = self
            .lookup_object(objects, object_id)
            .expect("validated fenv_t object")
            .modification_count;
        if self.fenv_provenance.get(&(object_id, start)).copied() != Some(current_version) {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} requires an unmodified fenv_t object produced by fegetenv or feholdexcept"
                ),
                args[0].span(),
                Some("7.6.4"),
            ));
        }
        Ok(())
    }

    fn check_free_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("free", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "free")?;
        if value.ty != CType::pointer_to(CType::Void) {
            return Err(Diagnostic::error(
                "free requires a void * argument",
                args[0].span(),
            ));
        }
        let pointer = value.as_pointer(args[0].span())?;
        if pointer.is_null() {
            self.pending_heap_call = Some(PendingHeapCall::Free(None));
            return Ok(());
        }
        let points_to_allocation_start = pointer.byte_offset_override == Some(0)
            || (pointer.base_offset == 0 && pointer.offset == 0 && pointer.member_path.is_empty());
        if !points_to_allocation_start || pointer.object.is_none() {
            return Err(Diagnostic::ub(
                "free requires a pointer value returned by malloc or a null pointer",
                args[0].span(),
                Some("7.20.3.2"),
            ));
        }
        let object_id = pointer.object.expect("checked above");
        let object = self.lookup_object(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "free requires a pointer value returned by malloc or a null pointer",
                args[0].span(),
                Some("7.20.3.2"),
            )
        })?;
        if object.storage_duration != StorageDuration::Dynamic {
            return Err(Diagnostic::ub(
                "free requires a pointer value returned by malloc or a null pointer",
                args[0].span(),
                Some("7.20.3.2"),
            ));
        }
        if !object.alive {
            return Err(Diagnostic::ub(
                "double free or free of an already ended allocation",
                args[0].span(),
                Some("7.20.3.2"),
            ));
        }
        self.pending_heap_call = Some(PendingHeapCall::Free(Some(object_id)));
        Ok(())
    }

    fn check_memory_copy_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args(function_name, args, evaluated, 3, span)?;
        let dest = &evaluated[0];
        let src = &evaluated[1];
        let count = &evaluated[2];
        self.reject_missing_return_value(dest, args[0].span())?;
        self.reject_missing_return_value(src, args[1].span())?;
        self.reject_missing_return_value(count, args[2].span())?;
        self.reject_indeterminate_library_value(dest, args[0].span(), function_name)?;
        self.reject_indeterminate_library_value(src, args[1].span(), function_name)?;
        self.reject_indeterminate_library_value(count, args[2].span(), function_name)?;
        if !self.pointer_assignment_compatible(&self.void_ptr_type(), &dest.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a void * destination"),
                args[0].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            function_name,
            dest,
            args[0].span(),
            objects,
        )?;
        if !self.pointer_assignment_compatible(&self.const_void_ptr_type(), &src.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a const void * source"),
                args[1].span(),
            ));
        }
        self.reject_nonvolatile_library_pointer_access(
            function_name,
            src,
            args[1].span(),
            objects,
        )?;
        if count.ty != CType::UnsignedLong {
            return Err(Diagnostic::error(
                format!("{function_name} requires an unsigned long byte count"),
                args[2].span(),
            ));
        }
        let size = self.checked_usize_from_unsigned_long(
            count,
            args[2].span(),
            &format!("{function_name} byte count"),
        )?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        let (dest_object_id, dest_start, _) = self.ensure_library_untyped_byte_destination(
            &dest_pointer,
            size,
            args[0].span(),
            objects,
        )?;
        let (src_object_id, src_start, _) =
            self.library_untyped_byte_region(&src_pointer, size, args[1].span(), objects)?;
        if function_name == "memcpy"
            && size != 0
            && dest_object_id == src_object_id
            && dest_start < src_start.saturating_add(size)
            && src_start < dest_start.saturating_add(size)
        {
            return Err(Diagnostic::ub(
                "memcpy source and destination regions overlap",
                span,
                Some("7.21.2.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn void_ptr_type(&self) -> CType {
        CType::pointer_to(CType::Void)
    }

    pub(super) fn const_void_ptr_type(&self) -> CType {
        CType::pointer_to(CType::qualified(
            CType::Void,
            crate::types::TypeQualifiers {
                is_atomic: false,
                is_const: true,
                is_restrict: false,
                is_volatile: false,
            },
        ))
    }

    pub(super) fn char_ptr_type(&self) -> CType {
        CType::pointer_to(CType::Char)
    }

    pub(super) fn const_char_ptr_type(&self) -> CType {
        CType::pointer_to(CType::qualified(
            CType::Char,
            crate::types::TypeQualifiers {
                is_atomic: false,
                is_const: true,
                is_restrict: false,
                is_volatile: false,
            },
        ))
    }

    fn allocate_runtime_c_string(
        &mut self,
        text: &str,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<PointerValue, Diagnostic> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        let char_ty = CType::Char;
        let array_ty = CType::array_of(char_ty.clone(), bytes.len());
        let mut elements = Vec::with_capacity(bytes.len());
        for byte in bytes {
            elements.push(self.stored_value_from_typed_value(
                TypedValue::integer(char_ty.clone(), byte as i128),
                &char_ty,
                span,
            )?);
        }
        let value = TypedValue::aggregate(array_ty.clone(), StoredValue::Array(elements));
        let object = self.allocate_object(
            objects,
            array_ty,
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        self.initialize_object_value(objects, object, value, span)?;
        Ok(PointerValue::at_object(object))
    }

    pub(super) fn build_main_arguments(
        &mut self,
        main: &FunctionDef,
        objects: &mut ObjectFrames,
    ) -> Result<Vec<TypedValue>, Diagnostic> {
        if main.return_type != CType::Int {
            return Err(Diagnostic::error(
                "unsupported main signature: main must return int",
                main.return_type_span,
            ));
        }
        if main.is_variadic {
            return Err(Diagnostic::error(
                "unsupported main signature: main cannot be variadic",
                main.span,
            ));
        }
        let fixed_param_count = if main.params.len() == 1 && main.params[0].ty == CType::Void {
            0
        } else {
            main.params.len()
        };
        if fixed_param_count == 0 {
            return Ok(Vec::new());
        }

        match fixed_param_count {
            1 => {
                return Err(Diagnostic::error(
                    "unsupported main signature: this definition gives main 1 parameter",
                    main.params[0].span,
                ));
            }
            2 => {
                if main.params[0].ty != CType::Int {
                    return Err(Diagnostic::error(
                        "unsupported main signature: first parameter must have type int",
                        main.params[0].span,
                    ));
                }
            }
            count => {
                let span = main
                    .params
                    .first()
                    .zip(main.params.last())
                    .map(|(first, last)| first.span.merge(last.span))
                    .unwrap_or(main.span);
                return Err(Diagnostic::error(
                    format!(
                        "unsupported main signature: this definition gives main {count} parameters"
                    ),
                    span,
                ));
            }
        }

        let argv_strings = ["program"];
        let argc = argv_strings.len() as i128;
        let argv_element_ty = self.char_ptr_type();
        let argv_pointer_ty = CType::pointer_to(argv_element_ty.clone());

        let mut argv_elements = Vec::with_capacity(argv_strings.len() + 1);
        for arg in argv_strings {
            let pointer = self.allocate_runtime_c_string(arg, main.span, objects)?;
            argv_elements.push(self.stored_value_from_typed_value(
                self.pointer_value_with_type(argv_element_ty.clone(), pointer),
                &argv_element_ty,
                main.span,
            )?);
        }
        argv_elements.push(self.stored_value_from_typed_value(
            self.pointer_value_with_type(argv_element_ty.clone(), Self::null_pointer()),
            &argv_element_ty,
            main.span,
        )?);
        let argv_array_ty = CType::array_of(argv_element_ty, argv_elements.len());
        let argv_object = self.allocate_object(
            objects,
            argv_array_ty.clone(),
            StorageDuration::Static,
            main.span,
            false,
            false,
        )?;
        self.initialize_object_value(
            objects,
            argv_object,
            TypedValue::aggregate(argv_array_ty, StoredValue::Array(argv_elements)),
            main.span,
        )?;
        let argv_value = self.pointer_value_with_type(
            argv_pointer_ty.clone(),
            PointerValue::at_object(argv_object),
        );

        if !self.pointer_assignment_compatible(&main.params[1].ty, &argv_value.ty) {
            return Err(Diagnostic::error(
                "unsupported main signature: second parameter must have type char ** or equivalent",
                main.params[1].span,
            ));
        }
        Ok(vec![TypedValue::int(argc), argv_value])
    }

    pub(super) fn stream_pointer_diag(
        &self,
        function_name: &'static str,
        span: Span,
        standard: &'static str,
    ) -> Diagnostic {
        Diagnostic::ub(
            format!("{function_name} requires a valid FILE * designating an open stream"),
            span,
            Some(standard),
        )
    }

    pub(super) fn stream_object_from_value(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<Option<ObjectId>, Diagnostic> {
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Ok(None);
        }
        let points_to_allocation_start = pointer.byte_offset_override == Some(0)
            || (pointer.base_offset == 0 && pointer.offset == 0 && pointer.member_path.is_empty());
        if !points_to_allocation_start || pointer.object.is_none() {
            return Err(self.stream_pointer_diag(function_name, span, standard));
        }
        let object = pointer.object.expect("checked above");
        if !self.host_streams.contains_key(&object) {
            return Err(self.stream_pointer_diag(function_name, span, standard));
        }
        Ok(Some(object))
    }

    pub(super) fn require_open_stream(
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
        if !stream.open {
            return Err(self.stream_pointer_diag(function_name, span, standard));
        }
        if self.stream_buffer_lifetime_has_ended(stream) {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} uses a stream whose supplied buffer's lifetime has ended"
                ),
                span,
                Some("7.19.5.5-6"),
            )
            .with_note("an array supplied to setbuf or setvbuf must remain alive until the stream is closed"));
        }
        Ok(())
    }

    pub(super) fn set_host_errno(errno: c_int) {
        unsafe {
            *host_errno_ptr() = errno;
        }
    }

    fn stream_length(&self, stream_object: ObjectId) -> usize {
        let Some(stream) = self.host_streams.get(&stream_object) else {
            return 0;
        };
        match stream.backing {
            StreamBacking::Capture(_) => stream.captured.len(),
            StreamBacking::File { file_id } => self
                .virtual_filesystem
                .files
                .get(&file_id)
                .map_or(0, Vec::len),
        }
    }

    pub(super) fn orient_stream_if_unoriented(&mut self, stream_object: ObjectId, orientation: i8) {
        if let Some(stream) = self.host_streams.get_mut(&stream_object)
            && stream.orientation == 0
        {
            stream.orientation = orientation;
        }
    }

    pub(super) fn read_one_byte_from_stream(
        &mut self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<Option<u8>, Diagnostic> {
        self.require_open_stream(stream_object, span, function_name, standard)?;
        self.orient_stream_if_unoriented(stream_object, -1);
        let (backing, position) = {
            let stream = self.host_streams.get_mut(&stream_object).unwrap();
            if let Some(byte) = stream.pushback.pop() {
                stream.eof = false;
                stream.error = false;
                self.mark_stream_input(stream_object, false);
                return Ok(Some(byte));
            }
            (stream.backing, stream.position)
        };
        let mut byte = match backing {
            StreamBacking::Capture(_) => self
                .host_streams
                .get(&stream_object)
                .and_then(|stream| stream.captured.get(position))
                .copied(),
            StreamBacking::File { file_id } => self
                .virtual_filesystem
                .files
                .get(&file_id)
                .and_then(|bytes| bytes.get(position))
                .copied(),
        };
        if byte.is_none()
            && backing == StreamBacking::Capture(CaptureStream::Stdin)
            && self
                .host_streams
                .get(&stream_object)
                .is_some_and(|stream| !stream.input_closed)
            && let Some(stream_io) = self.run_options.native_stream_io.clone()
        {
            let mut buffer = [0u8; 4096];
            let count = stream_io.read_stdin(&mut buffer).map_err(|error| {
                Diagnostic::error(format!("native stdin read failed: {error}"), span)
            })?;
            let stream = self.host_streams.get_mut(&stream_object).unwrap();
            if count == 0 {
                stream.input_closed = true;
            } else {
                stream.captured.extend_from_slice(&buffer[..count]);
                byte = stream.captured.get(position).copied();
            }
        }
        if byte.is_none()
            && backing == StreamBacking::Capture(CaptureStream::Stdin)
            && self
                .host_streams
                .get(&stream_object)
                .is_some_and(|stream| !stream.input_closed)
        {
            return Err(Diagnostic::blocked(function_name, span));
        }
        let hit_eof = byte.is_none();
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            if byte.is_some() {
                stream.position = stream.position.saturating_add(1);
            }
            stream.eof = hit_eof;
            stream.error = false;
        }
        self.mark_stream_input(stream_object, hit_eof);
        Ok(byte)
    }

    pub(super) fn unread_byte_to_stream(
        &mut self,
        stream_object: ObjectId,
        byte: u8,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.require_open_stream(stream_object, span, function_name, standard)?;
        let stream = self.host_streams.get_mut(&stream_object).unwrap();
        stream.pushback.push(byte);
        stream.eof = false;
        stream.error = false;
        Ok(())
    }

    pub(super) fn write_known_bytes_to_stream(
        &mut self,
        stream_object: ObjectId,
        bytes: &[u8],
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<usize, Diagnostic> {
        self.require_open_stream(stream_object, span, function_name, standard)?;
        self.orient_stream_if_unoriented(stream_object, -1);
        let (backing, mut position, append) = {
            let stream = self.host_streams.get(&stream_object).unwrap();
            (stream.backing, stream.position, stream.append)
        };
        match backing {
            StreamBacking::Capture(capture) => {
                let stream = self.host_streams.get_mut(&stream_object).unwrap();
                stream.captured.extend_from_slice(bytes);
                stream.position = stream.captured.len();
                stream.eof = false;
                stream.error = false;
                if let Some(stream_io) = self.run_options.native_stream_io.clone() {
                    let write = match capture {
                        CaptureStream::Stdout => stream_io.write_stdout(bytes),
                        CaptureStream::Stderr => stream_io.write_stderr(bytes),
                        CaptureStream::Stdin => Ok(()),
                    };
                    write.map_err(|error| {
                        Diagnostic::error(
                            format!(
                                "native {} write failed: {error}",
                                match capture {
                                    CaptureStream::Stdout => "stdout",
                                    CaptureStream::Stderr => "stderr",
                                    CaptureStream::Stdin => "stdin",
                                }
                            ),
                            span,
                        )
                    })?;
                }
                self.mark_stream_output(stream_object);
                return Ok(bytes.len());
            }
            StreamBacking::File { file_id } => {
                let contents = self.virtual_filesystem.files.entry(file_id).or_default();
                if append {
                    position = contents.len();
                }
                if position > contents.len() {
                    contents.resize(position, 0);
                }
                let end = position.saturating_add(bytes.len());
                if end > contents.len() {
                    contents.resize(end, 0);
                }
                contents[position..end].copy_from_slice(bytes);
                if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                    stream.position = end;
                    stream.eof = false;
                    stream.error = false;
                }
            }
        }
        self.mark_stream_output(stream_object);
        Ok(bytes.len())
    }

    pub(super) fn read_bytes_from_stream(
        &mut self,
        stream_object: ObjectId,
        size: usize,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = Vec::with_capacity(size);
        while bytes.len() < size {
            match self.read_one_byte_from_stream(stream_object, span, function_name, standard)? {
                Some(byte) => bytes.push(byte),
                None => break,
            }
        }
        Ok(bytes)
    }

    pub(super) fn read_entire_c_string_from_stream(
        &mut self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut out = Vec::new();
        while let Some(ch) =
            self.read_one_byte_from_stream(stream_object, span, function_name, standard)?
        {
            if ch == b'\n' {
                break;
            }
            out.push(ch);
        }
        Ok(out)
    }

    pub(super) fn write_wide_unit_to_stream(
        &mut self,
        stream_object: ObjectId,
        unit: libc::wchar_t,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.orient_stream_if_unoriented(stream_object, 1);
        let scalar = char::from_u32(unit as u32).ok_or_else(|| {
            Diagnostic::error(
                format!("{function_name} was given an invalid wide character"),
                span,
            )
        })?;
        let mut encoded = [0u8; 4];
        let bytes = scalar.encode_utf8(&mut encoded).as_bytes();
        self.write_known_bytes_to_stream(stream_object, bytes, span, function_name, standard)?;
        Ok(())
    }

    pub(super) fn read_wide_unit_from_stream(
        &mut self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<Option<libc::wchar_t>, Diagnostic> {
        self.orient_stream_if_unoriented(stream_object, 1);
        let Some(first) =
            self.read_one_byte_from_stream(stream_object, span, function_name, standard)?
        else {
            return Ok(None);
        };
        let width = match first {
            0x00..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => {
                if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                    stream.error = true;
                }
                return Ok(None);
            }
        };
        let mut bytes = vec![first];
        while bytes.len() < width {
            let Some(byte) =
                self.read_one_byte_from_stream(stream_object, span, function_name, standard)?
            else {
                if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                    stream.error = true;
                }
                return Ok(None);
            };
            bytes.push(byte);
        }
        let scalar = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| text.chars().next());
        match scalar {
            Some(value) => Ok(Some(value as libc::wchar_t)),
            None => {
                if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                    stream.error = true;
                }
                Ok(None)
            }
        }
    }

    pub(super) fn seek_stream(
        &mut self,
        stream_object: ObjectId,
        offset: i128,
        origin: c_int,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<c_int, Diagnostic> {
        self.require_open_stream(stream_object, span, function_name, standard)?;
        let base = match origin {
            value if value == libc::SEEK_SET => 0i128,
            value if value == libc::SEEK_CUR => {
                self.host_streams.get(&stream_object).map_or(0, |stream| {
                    stream.position.saturating_sub(stream.pushback.len())
                }) as i128
            }
            value if value == libc::SEEK_END => self.stream_length(stream_object) as i128,
            _ => {
                Self::set_host_errno(libc::EINVAL);
                if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                    stream.error = true;
                }
                return Ok(-1);
            }
        };
        let Some(position) = base
            .checked_add(offset)
            .and_then(|value| usize::try_from(value).ok())
        else {
            Self::set_host_errno(libc::EINVAL);
            if let Some(stream) = self.host_streams.get_mut(&stream_object) {
                stream.error = true;
            }
            return Ok(-1);
        };
        if let Some(stream) = self.host_streams.get_mut(&stream_object) {
            stream.position = position;
            stream.pushback.clear();
            stream.eof = false;
            stream.error = false;
        }
        self.clear_stream_last_operation(stream_object);
        Ok(0)
    }

    pub(super) fn tell_stream(
        &self,
        stream_object: ObjectId,
        span: Span,
        function_name: &'static str,
        standard: &'static str,
    ) -> Result<usize, Diagnostic> {
        self.require_open_stream(stream_object, span, function_name, standard)?;
        let stream = self.host_streams.get(&stream_object).unwrap();
        Ok(stream.position.saturating_sub(stream.pushback.len()))
    }

    pub(super) fn write_c_string_to_pointer(
        &mut self,
        pointer: &PointerValue,
        bytes: &[u8],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let total = bytes.len() + 1;
        let (object_id, start, _) = self.byte_region_from_pointer(pointer, total, span, objects)?;
        self.ensure_library_writable_region(object_id, start, total, span, objects)?;
        let mut copied = bytes.to_vec();
        copied.push(0);
        self.overlay_known_bytes_into_object(object_id, start, &copied, span, objects)
    }

    pub(super) fn write_wide_string_to_pointer(
        &mut self,
        pointer: &PointerValue,
        units: &[libc::wchar_t],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let mut copied = units.to_vec();
        copied.push(0);
        self.write_wide_units_to_pointer(pointer, &copied, span, objects)
    }

    pub(super) fn multibyte_bytes_to_wide_units(
        &self,
        bytes: &[u8],
        span: Span,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        let source = CString::new(bytes).map_err(|_| {
            Diagnostic::error("string argument contains an interior nul byte", span)
        })?;
        let mut source_ptr = source.as_ptr();
        let mut units = vec![0 as libc::wchar_t; bytes.len().saturating_add(1).max(1)];
        let written = unsafe {
            mbsrtowcs(
                units.as_mut_ptr(),
                &mut source_ptr,
                units.len(),
                std::ptr::null_mut(),
            )
        };
        if written == usize::MAX {
            return Err(Diagnostic::error(
                "multibyte-to-wide conversion failed in the current locale",
                span,
            ));
        }
        units.truncate(written);
        Ok(units)
    }

    pub(super) fn read_pointer_bytes(
        &mut self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<u8>, Diagnostic> {
        let (object_id, start, _) = self.byte_region_from_pointer(pointer, size, span, objects)?;
        let snapshot = self
            .lookup_object(objects, object_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        let bytes = self.serialize_stored_value(&snapshot.ty, &snapshot.value, span)?;
        self.known_bytes(&bytes[start..start + size], span)
    }

    pub(super) fn tmpnam_buffer_pointer(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<PointerValue, Diagnostic> {
        if let Some(object) = self.tmpnam_binding {
            return Ok(PointerValue::at_object(object));
        }
        let object = self.allocate_object(
            objects,
            CType::array_of(CType::Char, HOST_L_TMPNAM),
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        let zero_value = self.zero_stored_value(&CType::array_of(CType::Char, HOST_L_TMPNAM));
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.address_taken = true;
            state.value = zero_value;
        }
        self.tmpnam_binding = Some(object);
        Ok(PointerValue::at_object(object))
    }

    pub(super) fn standard_stream_object(
        &self,
        capture: CaptureStream,
        span: Span,
    ) -> Result<ObjectId, Diagnostic> {
        self.host_capture_streams
            .get(&capture)
            .copied()
            .ok_or_else(|| Diagnostic::error("host stdio stream was not initialized", span))
    }

    pub(super) fn check_host_library_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        frame: &Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.pending_heap_call = None;
        let result = match function_name {
            "remove" => self.check_remove_call(args, evaluated, span, objects),
            "rename" => self.check_rename_call(args, evaluated, span, objects),
            "tmpfile" => self.check_tmpfile_call(args, evaluated, span),
            "tmpnam" => self.check_tmpnam_call(args, evaluated, span, objects),
            "fclose" => self.check_fclose_call(args, evaluated, span),
            "fflush" => self.check_fflush_call(args, evaluated, span),
            "fopen" => self.check_fopen_call(args, evaluated, span, objects),
            "freopen" => self.check_freopen_call(args, evaluated, span, objects),
            "setbuf" => self.check_setbuf_call(args, evaluated, span, objects),
            "setvbuf" => self.check_setvbuf_call(args, evaluated, span, objects),
            "fprintf" => self.check_fprintf_call(args, evaluated, span, objects),
            "fscanf" => {
                if args.is_empty() || evaluated.is_empty() {
                    return Err(Diagnostic::error(
                        "fscanf requires a stream and format argument",
                        span,
                    ));
                }
                self.check_stream_call("fscanf", &args[..1], &evaluated[..1], span, "7.19.6.2")?;
                self.check_scanf_like_call("fscanf", 1, args, evaluated, span, objects)
            }
            "vfprintf" => {
                self.check_stream_call("vfprintf", &args[..1], &evaluated[..1], span, "7.19.6.8")?;
                self.check_vprintf_like_call("vfprintf", 1, 2, args, evaluated, span, objects)
            }
            "vfscanf" => {
                if args.is_empty() || evaluated.is_empty() {
                    return Err(Diagnostic::error(
                        "vfscanf requires a stream, format, and va_list argument",
                        span,
                    ));
                }
                self.check_stream_call("vfscanf", &args[..1], &evaluated[..1], span, "7.19.6.8")?;
                self.check_vscanf_like_call("vfscanf", 1, 2, args, evaluated, span, objects)
            }
            "fgetc" => self.check_fgetc_like_call("fgetc", args, evaluated, span, "7.19.7.1"),
            "fgets" => self.check_fgets_call(args, evaluated, span, objects),
            "fputc" => self.check_fputc_like_call("fputc", args, evaluated, span, "7.19.7.3"),
            "fputs" => self.check_fputs_call(args, evaluated, span, objects),
            "getc" => self.check_fgetc_like_call("getc", args, evaluated, span, "7.19.7.5"),
            "getchar" => self.check_tmpfile_call(args, evaluated, span),
            "gets" => self.check_gets_call(args, evaluated, span, objects),
            "putc" => self.check_fputc_like_call("putc", args, evaluated, span, "7.19.7.6"),
            "putchar" => self.check_putchar_call(args, evaluated, span),
            "printf" => self.check_printf_call(args, evaluated, span, objects),
            "scanf" => self.check_scanf_like_call("scanf", 0, args, evaluated, span, objects),
            "vprintf" => {
                self.check_vprintf_like_call("vprintf", 0, 1, args, evaluated, span, objects)
            }
            "vscanf" => self.check_vscanf_like_call("vscanf", 0, 1, args, evaluated, span, objects),
            "snprintf" => self.check_snprintf_call(args, evaluated, span, objects),
            "sprintf" => self.check_sprintf_call(args, evaluated, span, objects),
            "sscanf" => {
                if args.len() < 2 || evaluated.len() < 2 {
                    return Err(Diagnostic::error(
                        "sscanf requires a source string and format argument",
                        span,
                    ));
                }
                self.check_c_string_arg("sscanf", &evaluated[0], args[0].span(), objects)?;
                self.check_scanf_like_call("sscanf", 1, args, evaluated, span, objects)
            }
            "vsnprintf" => self.check_vsnprintf_call(args, evaluated, span, objects),
            "vsprintf" => self.check_vsprintf_call(args, evaluated, span, objects),
            "vsscanf" => {
                if args.len() < 3 || evaluated.len() < 3 {
                    return Err(Diagnostic::error(
                        "vsscanf requires a source string, format, and va_list argument",
                        span,
                    ));
                }
                self.check_c_string_arg("vsscanf", &evaluated[0], args[0].span(), objects)?;
                self.check_vscanf_like_call("vsscanf", 1, 2, args, evaluated, span, objects)
            }
            "puts" => self.check_puts_call(args, evaluated, span, objects),
            "ungetc" => self.check_ungetc_call(args, evaluated, span),
            "fread" => self.check_fread_call(args, evaluated, span, objects),
            "fwrite" => self.check_fwrite_call(args, evaluated, span, objects),
            "fgetpos" => self.check_fgetpos_call(args, evaluated, span, objects),
            "fseek" => self.check_fseek_call(args, evaluated, span),
            "fsetpos" => self.check_fsetpos_call(args, evaluated, span, objects),
            "ftell" => self.check_ftell_like_call("ftell", args, evaluated, span, "7.19.9.3"),
            "rewind" => self.check_clearerr_like_call("rewind", args, evaluated, span, "7.19.9.5"),
            "clearerr" => {
                self.check_clearerr_like_call("clearerr", args, evaluated, span, "7.19.10.1")
            }
            "feof" => self.check_ftell_like_call("feof", args, evaluated, span, "7.19.10.2"),
            "ferror" => self.check_ftell_like_call("ferror", args, evaluated, span, "7.19.10.1"),
            "perror" => self.check_perror_call(args, evaluated, span, objects),
            "__codex_assert_fail" => self.check_assert_fail_call(args, evaluated, span, objects),
            "__codex_errno_location" => {
                self.check_math_helper_nullary_call(function_name, args, evaluated, span)
            }
            "isalnum" | "isalpha" | "isblank" | "iscntrl" | "isdigit" | "isgraph" | "islower"
            | "isprint" | "ispunct" | "isspace" | "isupper" | "isxdigit" | "tolower"
            | "toupper" => self.check_ctype_call(function_name, args, evaluated, span),
            "iswalnum" | "iswalpha" | "iswblank" | "iswcntrl" | "iswdigit" | "iswgraph"
            | "iswlower" | "iswprint" | "iswpunct" | "iswspace" | "iswupper" | "iswxdigit"
            | "towlower" | "towupper" => {
                self.check_unary_wint_call(function_name, args, evaluated, span)
            }
            "iswctype" => self.check_iswctype_call(args, evaluated, span),
            "towctrans" => self.check_towctrans_call(args, evaluated, span),
            "wctrans" => self.check_wctrans_lookup_call(args, evaluated, span, objects),
            "wctype" => self.check_wctype_lookup_call(args, evaluated, span, objects),
            "abs" => self.check_abs_call("abs", args, evaluated, &CType::Int, span),
            "labs" => self.check_abs_call("labs", args, evaluated, &CType::Long, span),
            "llabs" => self.check_abs_call("llabs", args, evaluated, &CType::LongLong, span),
            "__codex_mb_cur_max" => {
                self.check_math_helper_nullary_call("__codex_mb_cur_max", args, evaluated, span)
            }
            "atof" => self.check_unary_string_call("atof", args, evaluated, span, objects),
            "atoi" | "atol" | "atoll" => {
                self.check_unary_string_call(function_name, args, evaluated, span, objects)
            }
            "btowc" => self.check_ctype_call("btowc", args, evaluated, span),
            "wctob" => self.check_unary_wint_call("wctob", args, evaluated, span),
            "mbsinit" => self.check_mbsinit_call(args, evaluated, span, objects),
            "mblen" => self.check_mblen_call(args, evaluated, span, objects),
            "mbrlen" => self.check_mbrlen_call(args, evaluated, span, objects),
            "mbrtowc" => self.check_mbrtowc_call(args, evaluated, span, objects),
            "mbtowc" => self.check_mbtowc_call(args, evaluated, span, objects),
            "wcrtomb" => self.check_wcrtomb_call(args, evaluated, span, objects),
            "mbrtoc16" | "mbrtoc32" => {
                self.check_mbrtoc_call(function_name, args, evaluated, span, objects)
            }
            "c16rtomb" | "c32rtomb" => {
                self.check_c_rtomb_call(function_name, args, evaluated, span, objects)
            }
            "wctomb" => self.check_wctomb_call(args, evaluated, span, objects),
            "mbstowcs" => self.check_mbstowcs_call(args, evaluated, span, objects),
            "mbsrtowcs" => self.check_mbsrtowcs_call(args, evaluated, span, objects),
            "wcstombs" => self.check_wcstombs_call(args, evaluated, span, objects),
            "wcsrtombs" => self.check_wcsrtombs_call(args, evaluated, span, objects),
            "fwprintf" => {
                self.check_stream_call("fwprintf", &args[..1], &evaluated[..1], span, "7.24.2.1")?;
                self.check_wprintf_like_call("fwprintf", 1, args, evaluated, span, objects)
            }
            "fwscanf" => {
                if args.is_empty() || evaluated.is_empty() {
                    return Err(Diagnostic::error(
                        "fwscanf requires a stream and format argument",
                        span,
                    ));
                }
                self.check_stream_call("fwscanf", &args[..1], &evaluated[..1], span, "7.24.2.2")?;
                self.check_wscanf_like_call("fwscanf", 1, args, evaluated, span, objects)
            }
            "swprintf" => {
                if args.len() < 3 || evaluated.len() < 3 {
                    return Err(Diagnostic::error(
                        "swprintf requires destination, size, and format arguments",
                        span,
                    ));
                }
                self.check_bounded_wide_printf_destination(
                    "swprintf", args, evaluated, span, objects,
                )?;
                self.check_wprintf_like_call("swprintf", 2, args, evaluated, span, objects)
            }
            "swscanf" => {
                if args.len() < 2 || evaluated.len() < 2 {
                    return Err(Diagnostic::error(
                        "swscanf requires a source string and format argument",
                        span,
                    ));
                }
                self.check_wide_string_arg("swscanf", &evaluated[0], args[0].span(), objects)?;
                self.check_wscanf_like_call("swscanf", 1, args, evaluated, span, objects)
            }
            "vfwprintf" => {
                self.check_stream_call("vfwprintf", &args[..1], &evaluated[..1], span, "7.24.2.5")?;
                self.check_vwprintf_like_call("vfwprintf", 1, 2, args, evaluated, span, objects)
            }
            "vfwscanf" => {
                if args.is_empty() || evaluated.is_empty() {
                    return Err(Diagnostic::error(
                        "vfwscanf requires a stream, format, and va_list argument",
                        span,
                    ));
                }
                self.check_stream_call("vfwscanf", &args[..1], &evaluated[..1], span, "7.24.2.6")?;
                self.check_vwscanf_like_call("vfwscanf", 1, 2, args, evaluated, span, objects)
            }
            "vswprintf" => {
                if args.len() != 4 || evaluated.len() != 4 {
                    return Err(Diagnostic::error(
                        "vswprintf requires destination, size, format, and va_list arguments",
                        span,
                    ));
                }
                self.check_bounded_wide_printf_destination(
                    "vswprintf",
                    args,
                    evaluated,
                    span,
                    objects,
                )?;
                self.check_vwprintf_like_call("vswprintf", 2, 3, args, evaluated, span, objects)
            }
            "vswscanf" => {
                if args.len() < 3 || evaluated.len() < 3 {
                    return Err(Diagnostic::error(
                        "vswscanf requires a source string, format, and va_list argument",
                        span,
                    ));
                }
                self.check_wide_string_arg("vswscanf", &evaluated[0], args[0].span(), objects)?;
                self.check_vwscanf_like_call("vswscanf", 1, 2, args, evaluated, span, objects)
            }
            "vwprintf" => {
                self.check_vwprintf_like_call("vwprintf", 0, 1, args, evaluated, span, objects)
            }
            "vwscanf" => {
                self.check_vwscanf_like_call("vwscanf", 0, 1, args, evaluated, span, objects)
            }
            "wprintf" => self.check_wprintf_like_call("wprintf", 0, args, evaluated, span, objects),
            "wscanf" => self.check_wscanf_like_call("wscanf", 0, args, evaluated, span, objects),
            "fgetwc" => self.check_fgetc_like_call("fgetwc", args, evaluated, span, "7.24.3.1"),
            "fgetws" => self.check_fgetws_call(args, evaluated, span, objects),
            "fputwc" => self.check_fputwc_like_call("fputwc", args, evaluated, span, "7.24.3.7"),
            "fputws" => self.check_fputws_call(args, evaluated, span, objects),
            "fwide" => self.check_fwide_call(args, evaluated, span),
            "getwc" => self.check_fgetc_like_call("getwc", args, evaluated, span, "7.24.3.8"),
            "getwchar" => self.check_tmpfile_call(args, evaluated, span),
            "putwc" => self.check_fputwc_like_call("putwc", args, evaluated, span, "7.24.3.9"),
            "putwchar" => {
                self.require_exact_call_args("putwchar", args, evaluated, 1, span)?;
                self.check_library_argument_value(&evaluated[0], args[0].span(), "putwchar")?;
                if evaluated[0].ty != self.wchar_type() {
                    return Err(Diagnostic::error(
                        "putwchar character argument must have type wchar_t",
                        args[0].span(),
                    ));
                }
                Ok(())
            }
            "ungetwc" => self.check_ungetwc_call(args, evaluated, span),
            "wcstof" | "wcstod" | "wcstold" | "wcstol" | "wcstoll" | "wcstoul" | "wcstoull" => {
                self.check_wcsto_call(function_name, args, evaluated, span, objects)
            }
            "wcscpy" => self.check_wcscpy_call(args, evaluated, span, objects),
            "wcsncpy" => self.check_wcsncpy_call(args, evaluated, span, objects),
            "wmemcpy" | "wmemmove" => {
                self.check_wide_memory_copy_call(function_name, args, evaluated, span, objects)
            }
            "wcscat" => self.check_wcscat_call(args, evaluated, span, objects),
            "wcsncat" => self.check_wcsncat_call(args, evaluated, span, objects),
            "wcslen" => self.check_basic_wide_string_call("wcslen", args, evaluated, span, objects),
            "wcscmp" => self.check_basic_wide_string_call("wcscmp", args, evaluated, span, objects),
            "wcscoll" => {
                self.check_basic_wide_string_call("wcscoll", args, evaluated, span, objects)
            }
            "wcsncmp" => self.check_wide_string_n_call("wcsncmp", args, evaluated, span, objects),
            "wcsxfrm" => self.check_wcsxfrm_call(args, evaluated, span, objects),
            "wcschr" => self.check_wide_char_search_call("wcschr", args, evaluated, span, objects),
            "wcsrchr" => {
                self.check_wide_char_search_call("wcsrchr", args, evaluated, span, objects)
            }
            "wcsspn" => self.check_basic_wide_string_call("wcsspn", args, evaluated, span, objects),
            "wcscspn" => {
                self.check_basic_wide_string_call("wcscspn", args, evaluated, span, objects)
            }
            "wcspbrk" => {
                self.check_basic_wide_string_call("wcspbrk", args, evaluated, span, objects)
            }
            "wcsstr" => self.check_basic_wide_string_call("wcsstr", args, evaluated, span, objects),
            "wmemchr" => {
                self.require_exact_call_args("wmemchr", args, evaluated, 3, span)?;
                self.check_library_argument_value(&evaluated[0], args[0].span(), "wmemchr")?;
                if !self
                    .pointer_assignment_compatible(&self.const_wchar_ptr_type(), &evaluated[0].ty)
                {
                    return Err(Diagnostic::error(
                        "wmemchr requires a const wchar_t * input pointer",
                        args[0].span(),
                    ));
                }
                self.check_library_argument_value(&evaluated[1], args[1].span(), "wmemchr")?;
                if evaluated[1].ty != self.wchar_type() {
                    return Err(Diagnostic::error(
                        "wmemchr requires a wchar_t character argument",
                        args[1].span(),
                    ));
                }
                self.check_library_argument_value(&evaluated[2], args[2].span(), "wmemchr")?;
                if evaluated[2].ty != CType::UnsignedLong {
                    return Err(Diagnostic::error(
                        "wmemchr requires an unsigned long count argument",
                        args[2].span(),
                    ));
                }
                let count = self.checked_usize_from_unsigned_long(
                    &evaluated[2],
                    args[2].span(),
                    "wmemchr count",
                )?;
                let total = count
                    .checked_mul(self.wide_char_byte_width())
                    .ok_or_else(|| Diagnostic::error("wmemchr size is out of range", span))?;
                let pointer = evaluated[0].as_pointer(args[0].span())?;
                let _ = self.library_array_region(
                    &pointer,
                    total,
                    self.wide_char_byte_width(),
                    args[0].span(),
                    objects,
                )?;
                Ok(())
            }
            "wmemcmp" => self.check_wide_string_n_call("wmemcmp", args, evaluated, span, objects),
            "wmemset" => {
                self.require_exact_call_args("wmemset", args, evaluated, 3, span)?;
                self.check_library_argument_value(&evaluated[0], args[0].span(), "wmemset")?;
                if !self.pointer_assignment_compatible(&self.wchar_ptr_type(), &evaluated[0].ty) {
                    return Err(Diagnostic::error(
                        "wmemset requires a wchar_t * destination",
                        args[0].span(),
                    ));
                }
                self.check_library_argument_value(&evaluated[1], args[1].span(), "wmemset")?;
                if evaluated[1].ty != self.wchar_type() {
                    return Err(Diagnostic::error(
                        "wmemset requires a wchar_t fill argument",
                        args[1].span(),
                    ));
                }
                self.check_library_argument_value(&evaluated[2], args[2].span(), "wmemset")?;
                if evaluated[2].ty != CType::UnsignedLong {
                    return Err(Diagnostic::error(
                        "wmemset requires an unsigned long count argument",
                        args[2].span(),
                    ));
                }
                let count = self.checked_usize_from_unsigned_long(
                    &evaluated[2],
                    args[2].span(),
                    "wmemset count",
                )?;
                let total = count
                    .checked_mul(self.wide_char_byte_width())
                    .ok_or_else(|| Diagnostic::error("wmemset size is out of range", span))?;
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
            "wcstok" => self.check_wcstok_call(args, evaluated, span, objects),
            "wcsftime" => self.check_wcsftime_call(args, evaluated, span, objects),
            "strtof" | "strtod" | "strtold" | "strtol" | "strtoul" | "strtoll" | "strtoull" => {
                self.check_strto_call(function_name, args, evaluated, span, objects)
            }
            "rand" => self.check_math_helper_nullary_call(function_name, args, evaluated, span),
            "srand" => self.check_srand_call(args, evaluated, span),
            "div" => self.check_binary_integer_call("div", args, evaluated, &CType::Int, span),
            "ldiv" => self.check_binary_integer_call("ldiv", args, evaluated, &CType::Long, span),
            "lldiv" => {
                self.check_binary_integer_call("lldiv", args, evaluated, &CType::LongLong, span)
            }
            "imaxabs" => self.check_abs_call("imaxabs", args, evaluated, &CType::Long, span),
            "imaxdiv" => {
                self.check_binary_integer_call("imaxdiv", args, evaluated, &CType::Long, span)
            }
            "strtoimax" => self.check_strto_call("strtoimax", args, evaluated, span, objects),
            "strtoumax" => self.check_strto_call("strtoumax", args, evaluated, span, objects),
            "malloc" => self.check_malloc_call(args, evaluated, span),
            "aligned_alloc" => self.check_aligned_alloc_call(args, evaluated, span),
            "calloc" => self.check_calloc_call(args, evaluated, span),
            "realloc" => self.check_realloc_call(args, evaluated, span, objects),
            "free" => self.check_free_call(args, evaluated, span, objects),
            "abort" => self.check_math_helper_nullary_call("abort", args, evaluated, span),
            "atexit" | "at_quick_exit" => self.check_atexit_call(args, evaluated, span),
            "exit" | "quick_exit" | "_Exit" => {
                self.check_exit_call(function_name, args, evaluated, span)
            }
            "bsearch" => self.check_bsearch_call(args, evaluated, span, objects),
            "qsort" => self.check_qsort_call(args, evaluated, span, objects),
            "getenv" => self.check_unary_string_call("getenv", args, evaluated, span, objects),
            "system" => self.check_system_call(args, evaluated, span, objects),
            "setlocale" => self.check_setlocale_call(args, evaluated, span, objects),
            "localeconv" => {
                self.check_math_helper_nullary_call("localeconv", args, evaluated, span)
            }
            "signal" => self.check_signal_call(args, evaluated, span),
            "raise" => self.check_raise_call(args, evaluated, span),
            "__codex_sig_dfl" | "__codex_sig_ign" | "__codex_sig_err" => {
                self.check_signal_sentinel_call(function_name, args, evaluated, span)
            }
            "clock" => self.check_math_helper_nullary_call("clock", args, evaluated, span),
            "difftime" => self.check_difftime_call(args, evaluated, span),
            "mktime" => self.check_mktime_call(args, evaluated, span, objects),
            "time" => self.check_time_call(args, evaluated, span, objects),
            "asctime" => self.check_asctime_call(args, evaluated, span, objects),
            "ctime" => self.check_ctime_call("ctime", args, evaluated, span, objects),
            "gmtime" | "localtime" => {
                self.check_ctime_call(function_name, args, evaluated, span, objects)
            }
            "strftime" => self.check_strftime_call(args, evaluated, span, objects),
            "timespec_get" => self.check_timespec_get_call(args, evaluated, span, objects),
            "feclearexcept" | "feraiseexcept" | "fetestexcept" => {
                self.check_fenv_mask_call(function_name, args, evaluated, span)
            }
            "fegetexceptflag" => self.check_fegetexceptflag_call(args, evaluated, span, objects),
            "fesetexceptflag" => self.check_fesetexceptflag_call(args, evaluated, span, objects),
            "fegetround" => {
                self.check_math_helper_nullary_call("fegetround", args, evaluated, span)
            }
            "fesetround" => self.check_fesetround_call(args, evaluated, span),
            "fegetenv" | "feholdexcept" => {
                self.check_fenv_env_output_call(function_name, args, evaluated, span, objects)
            }
            "fesetenv" | "feupdateenv" => {
                self.check_fenv_env_input_call(function_name, args, evaluated, span, objects)
            }
            "__codex_fe_dfl_env" => {
                self.check_math_helper_nullary_call("__codex_fe_dfl_env", args, evaluated, span)
            }
            "memcpy" | "memmove" => {
                self.check_memory_copy_call(function_name, args, evaluated, span, objects)
            }
            "memset" => self.check_memset_call(args, evaluated, span, objects),
            "memchr" => self.check_memchr_call(args, evaluated, span, objects),
            "memcmp" => self.check_memcmp_call(args, evaluated, span, objects),
            "__codex_setjmp" => self.check_setjmp_call(args, evaluated, span, frame, objects),
            "longjmp" => self.check_longjmp_call(args, evaluated, span, objects),
            "strlen" => self.check_strlen_call(args, evaluated, span, objects),
            "strcmp" | "strcoll" | "strspn" | "strcspn" | "strpbrk" | "strstr" => {
                self.check_binary_c_string_call(function_name, args, evaluated, span, objects)
            }
            "strncmp" => self.check_strncmp_call(args, evaluated, span, objects),
            "strcpy" => self.check_strcpy_call(args, evaluated, span, objects),
            "strncpy" => self.check_strncpy_call(args, evaluated, span, objects),
            "strcat" => self.check_strcat_call(args, evaluated, span, objects),
            "strncat" => self.check_strncat_call(args, evaluated, span, objects),
            "strxfrm" => self.check_strxfrm_call(args, evaluated, span, objects),
            "strerror" => self.check_strerror_call(args, evaluated, span),
            "strdup" => self.check_strdup_call(args, evaluated, span, objects),
            "strchr" | "strrchr" => self.check_c_string_character_search_call(
                function_name,
                args,
                evaluated,
                span,
                objects,
            ),
            "strtok" => self.check_strtok_call(args, evaluated, span, objects),
            _ if Self::is_host_complex_function(function_name) => {
                self.check_complex_call(function_name, args, evaluated, span)
            }
            "__codex_math_errhandling"
            | "__codex_huge_val"
            | "__codex_huge_valf"
            | "__codex_huge_vall" => {
                self.check_math_helper_nullary_call(function_name, args, evaluated, span)
            }
            "__codex_fpclassify"
            | "__codex_fpclassifyf"
            | "__codex_fpclassifyl"
            | "__codex_isfinite"
            | "__codex_isinf"
            | "__codex_isnan"
            | "__codex_isnormal"
            | "__codex_isnormalf"
            | "__codex_isnormall"
            | "__codex_signbit" => self.check_math_unary_real_call(function_name, args, evaluated),
            "__codex_isgreater"
            | "__codex_isgreaterequal"
            | "__codex_isless"
            | "__codex_islessequal"
            | "__codex_islessgreater"
            | "__codex_isunordered" => {
                self.check_math_binary_real_call(function_name, args, evaluated)
            }
            "acos" | "acosf" | "acosl" | "asin" | "asinf" | "asinl" | "atan" | "atanf"
            | "atanl" | "cos" | "cosf" | "cosl" | "sin" | "sinf" | "sinl" | "tan" | "tanf"
            | "tanl" | "acosh" | "acoshf" | "acoshl" | "asinh" | "asinhf" | "asinhl" | "atanh"
            | "atanhf" | "atanhl" | "cosh" | "coshf" | "coshl" | "sinh" | "sinhf" | "sinhl"
            | "tanh" | "tanhf" | "tanhl" | "exp" | "expf" | "expl" | "log" | "logf" | "logl"
            | "log10" | "log10f" | "log10l" | "exp2" | "exp2f" | "exp2l" | "expm1" | "expm1f"
            | "expm1l" | "log1p" | "log1pf" | "log1pl" | "log2" | "log2f" | "log2l" | "logb"
            | "logbf" | "logbl" | "sqrt" | "sqrtf" | "sqrtl" | "cbrt" | "cbrtf" | "cbrtl"
            | "erf" | "erff" | "erfl" | "erfc" | "erfcf" | "erfcl" | "tgamma" | "tgammaf"
            | "tgammal" | "lgamma" | "lgammaf" | "lgammal" | "fabs" | "fabsf" | "fabsl"
            | "ceil" | "ceilf" | "ceill" | "floor" | "floorf" | "floorl" | "trunc" | "truncf"
            | "truncl" | "round" | "roundf" | "roundl" | "rint" | "rintf" | "rintl"
            | "nearbyint" | "nearbyintf" | "nearbyintl" | "ilogb" | "ilogbf" | "ilogbl"
            | "lround" | "lroundf" | "lroundl" | "lrint" | "lrintf" | "lrintl" | "llround"
            | "llroundf" | "llroundl" | "llrint" | "llrintf" | "llrintl" => {
                self.check_math_unary_real_call(function_name, args, evaluated)
            }
            "atan2" | "atan2f" | "atan2l" | "pow" | "powf" | "powl" | "hypot" | "hypotf"
            | "hypotl" | "fmod" | "fmodf" | "fmodl" | "remainder" | "remainderf" | "remainderl"
            | "copysign" | "copysignf" | "copysignl" | "nextafter" | "nextafterf"
            | "nextafterl" | "nexttoward" | "nexttowardf" | "nexttowardl" | "fdim" | "fdimf"
            | "fdiml" | "fmax" | "fmaxf" | "fmaxl" | "fmin" | "fminf" | "fminl" => {
                self.check_math_binary_real_call(function_name, args, evaluated)
            }
            "fma" | "fmaf" | "fmal" => {
                self.check_math_ternary_real_call(function_name, args, evaluated)
            }
            "ldexp" | "ldexpf" | "ldexpl" | "scalbn" | "scalbnf" | "scalbnl" => {
                self.check_math_real_int_call(function_name, args, evaluated, CType::Int)
            }
            "scalbln" | "scalblnf" | "scalblnl" => {
                self.check_math_real_int_call(function_name, args, evaluated, CType::Long)
            }
            "frexp" | "frexpf" | "frexpl" | "modf" | "modff" | "modfl" => {
                self.check_math_output_pointer_call(function_name, args, evaluated, 1, objects)
            }
            "remquo" | "remquof" | "remquol" => {
                self.check_math_output_pointer_call(function_name, args, evaluated, 2, objects)
            }
            "nan" | "nanf" | "nanl" => {
                self.check_math_nan_call(function_name, args, evaluated, objects)
            }
            _ => Err(Diagnostic::error(
                format!("unsupported host library function {}", function_name),
                span,
            )),
        };
        result?;
        self.check_stdio_preconditions(function_name, args, evaluated, span)
    }

    pub(super) fn eval_host_library_call(
        &mut self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if function_name != "__codex_errno_location" {
            self.sync_host_errno_to_host(objects)?;
        }
        let _host_runtime_guard = DepthGuard::enter(&mut self.host_library_runtime_depth);
        let result = match function_name {
            "remove" => self.eval_remove_call(evaluated, args, span, objects),
            "rename" => self.eval_rename_call(evaluated, args, span, objects),
            "tmpfile" => self.eval_tmpfile_call(span, objects),
            "tmpnam" => self.eval_tmpnam_call(evaluated, args, span, objects),
            "fclose" => self.eval_fclose_call(evaluated, args, span, objects),
            "fflush" => self.eval_fflush_call(evaluated, args, span),
            "fopen" => self.eval_fopen_call(evaluated, args, span, objects),
            "freopen" => self.eval_freopen_call(evaluated, args, span, objects),
            "setbuf" => self.eval_setbuf_call(evaluated, args, span),
            "setvbuf" => self.eval_setvbuf_call(evaluated, args, span),
            "fprintf" => self.eval_fprintf_call(args, evaluated, span, objects),
            "fscanf" => self.eval_fscanf_call(args, evaluated, span, objects),
            "vfprintf" => self.eval_vfprintf_call(args, evaluated, span, objects),
            "vfscanf" => self.eval_vfscanf_call(args, evaluated, span, objects),
            "fgetc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[0], args[0].span(), "fgetc", "7.19.7.1")?
                    .expect("checked by caller");
                self.eval_fgetc_like_call(stream, args[0].span(), "fgetc", "7.19.7.1")
            }
            "fgets" => self.eval_fgets_call(evaluated, args, span, objects),
            "fputc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[1], args[1].span(), "fputc", "7.19.7.3")?
                    .expect("checked by caller");
                self.eval_fputc_like_call(
                    evaluated[0].to_int()? as c_int,
                    stream,
                    args[1].span(),
                    "fputc",
                    "7.19.7.3",
                )
            }
            "fputs" => self.eval_fputs_call(evaluated, args, span, objects),
            "getc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[0], args[0].span(), "getc", "7.19.7.5")?
                    .expect("checked by caller");
                self.eval_fgetc_like_call(stream, args[0].span(), "getc", "7.19.7.5")
            }
            "getchar" => {
                let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
                self.eval_fgetc_like_call(stream, span, "getchar", "7.19.7.5")
            }
            "gets" => self.eval_gets_call(evaluated, args, span, objects),
            "putc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[1], args[1].span(), "putc", "7.19.7.6")?
                    .expect("checked by caller");
                self.eval_fputc_like_call(
                    evaluated[0].to_int()? as c_int,
                    stream,
                    args[1].span(),
                    "putc",
                    "7.19.7.6",
                )
            }
            "putchar" => {
                let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
                self.eval_fputc_like_call(
                    evaluated[0].to_int()? as c_int,
                    stream,
                    span,
                    "putchar",
                    "7.19.7.6",
                )
            }
            "printf" => self.eval_printf_call(args, evaluated, span, objects),
            "scanf" => self.eval_scanf_call(args, evaluated, span, objects),
            "vprintf" => self.eval_vprintf_call(args, evaluated, span, objects),
            "vscanf" => self.eval_vscanf_call(args, evaluated, span, objects),
            "snprintf" => self.eval_snprintf_call(args, evaluated, span, objects),
            "sprintf" => self.eval_sprintf_call(args, evaluated, span, objects),
            "sscanf" => self.eval_sscanf_call(args, evaluated, span, objects),
            "vsnprintf" => self.eval_vsnprintf_call(args, evaluated, span, objects),
            "vsprintf" => self.eval_vsprintf_call(args, evaluated, span, objects),
            "vsscanf" => self.eval_vsscanf_call(args, evaluated, span, objects),
            "puts" => self.eval_puts_call(args, evaluated, span, objects),
            "ungetc" => self.eval_ungetc_call(evaluated, args, span),
            "fread" => self.eval_fread_call(evaluated, args, span, objects),
            "fwrite" => self.eval_fwrite_call(evaluated, args, span, objects),
            "fgetpos" => self.eval_fgetpos_call(evaluated, args, span, objects),
            "fseek" => self.eval_fseek_call(evaluated, args, span),
            "fsetpos" => self.eval_fsetpos_call(evaluated, args, span, objects),
            "ftell" => self.eval_ftell_call(evaluated, args, span),
            "rewind" => self.eval_rewind_call(evaluated, args, span),
            "clearerr" => self.eval_clearerr_call(evaluated, args, span),
            "feof" => self.eval_feof_call(evaluated, args, span),
            "ferror" => self.eval_ferror_call(evaluated, args, span),
            "perror" => self.eval_perror_call(evaluated, args, span, objects),
            "__codex_assert_fail" => self.eval_assert_fail_call(evaluated, args, span, objects),
            "__codex_errno_location" => self.eval_errno_location_call(span, objects),
            "isalnum" | "isalpha" | "isblank" | "iscntrl" | "isdigit" | "isgraph" | "islower"
            | "isprint" | "ispunct" | "isspace" | "isupper" | "isxdigit" | "tolower"
            | "toupper" => self.eval_ctype_call(function_name, evaluated, span),
            "iswalnum" | "iswalpha" | "iswblank" | "iswcntrl" | "iswdigit" | "iswgraph"
            | "iswlower" | "iswprint" | "iswpunct" | "iswspace" | "iswupper" | "iswxdigit"
            | "iswctype" | "towlower" | "towupper" | "towctrans" | "wctrans" | "wctype" => {
                self.eval_wctype_call(function_name, evaluated, args, span, objects)
            }
            "abs" => {
                let value = evaluated[0].to_int()?;
                let result = if value < 0 {
                    self.ensure_integer_ub_range(-value, &CType::Int, span, "7.20.6.1")?
                } else {
                    value
                };
                Ok(TypedValue::integer(CType::Int, result))
            }
            "labs" => {
                let value = evaluated[0].to_int()?;
                let result = if value < 0 {
                    self.ensure_integer_ub_range(-value, &CType::Long, span, "7.20.6.1")?
                } else {
                    value
                };
                Ok(TypedValue::integer(CType::Long, result))
            }
            "llabs" => {
                let value = evaluated[0].to_int()?;
                let result = if value < 0 {
                    self.ensure_integer_ub_range(-value, &CType::LongLong, span, "7.20.6.1")?
                } else {
                    value
                };
                Ok(TypedValue::integer(CType::LongLong, result))
            }
            "__codex_mb_cur_max" => Ok(TypedValue::integer(
                CType::UnsignedLong,
                host_mb_cur_max() as i128,
            )),
            "atof" => self.eval_atof_call(evaluated, args, span, objects),
            "atoi" | "atol" | "atoll" => {
                self.eval_atoi_like_call(function_name, evaluated, args, span, objects)
            }
            "btowc" => Ok(TypedValue::int(unsafe {
                btowc(evaluated[0].to_int()? as c_int) as i128
            })),
            "wctob" => Ok(TypedValue::int(unsafe {
                wctob(evaluated[0].to_int()? as c_int) as i128
            })),
            "mbsinit" => self.eval_mbsinit_call(evaluated, args, span, objects),
            "mblen" => self.eval_mblen_call(evaluated, args, span, objects),
            "mbrlen" => self.eval_mbrlen_call(evaluated, args, span, objects),
            "mbrtowc" => self.eval_mbrtowc_call(evaluated, args, span, objects),
            "mbtowc" => self.eval_mbtowc_call(evaluated, args, span, objects),
            "wcrtomb" => self.eval_wcrtomb_call(evaluated, args, span, objects),
            "mbrtoc16" | "mbrtoc32" => {
                self.eval_mbrtoc_call(function_name, evaluated, args, span, objects)
            }
            "c16rtomb" | "c32rtomb" => {
                self.eval_c_rtomb_call(function_name, evaluated, args, span, objects)
            }
            "wctomb" => self.eval_wctomb_call(evaluated, args, span, objects),
            "mbstowcs" => self.eval_mbstowcs_call(evaluated, args, span, objects),
            "mbsrtowcs" => self.eval_mbsrtowcs_call(evaluated, args, span, objects),
            "wcstombs" => self.eval_wcstombs_call(evaluated, args, span, objects),
            "wcsrtombs" => self.eval_wcsrtombs_call(evaluated, args, span, objects),
            "fwprintf" => self.eval_fwprintf_call(args, evaluated, span, objects),
            "fwscanf" => self.eval_fwscanf_call(args, evaluated, span, objects),
            "swprintf" => self.eval_swprintf_call(args, evaluated, span, objects),
            "swscanf" => self.eval_swscanf_call(args, evaluated, span, objects),
            "vfwprintf" => self.eval_vfwprintf_call(args, evaluated, span, objects),
            "vfwscanf" => self.eval_vfwscanf_call(args, evaluated, span, objects),
            "vswprintf" => self.eval_vswprintf_call(args, evaluated, span, objects),
            "vswscanf" => self.eval_vswscanf_call(args, evaluated, span, objects),
            "vwprintf" => self.eval_vwprintf_call(args, evaluated, span, objects),
            "vwscanf" => self.eval_vwscanf_call(args, evaluated, span, objects),
            "wprintf" => self.eval_wprintf_call(args, evaluated, span, objects),
            "wscanf" => self.eval_wscanf_call(args, evaluated, span, objects),
            "fgetwc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[0], args[0].span(), "fgetwc", "7.24.3.1")?
                    .expect("checked by caller");
                self.eval_fgetwc_like_call(stream, args[0].span(), "fgetwc", "7.24.3.1")
            }
            "fgetws" => self.eval_fgetws_call(evaluated, args, span, objects),
            "fputwc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[1], args[1].span(), "fputwc", "7.24.3.7")?
                    .expect("checked by caller");
                self.eval_fputwc_like_call(
                    self.wchar_scalar_to_host(&evaluated[0], args[0].span(), "fputwc")?,
                    stream,
                    args[1].span(),
                    "fputwc",
                    "7.24.3.7",
                )
            }
            "fputws" => self.eval_fputws_call(evaluated, args, span, objects),
            "fwide" => self.eval_fwide_call(evaluated, args, span),
            "getwc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[0], args[0].span(), "getwc", "7.24.3.8")?
                    .expect("checked by caller");
                self.eval_fgetwc_like_call(stream, args[0].span(), "getwc", "7.24.3.8")
            }
            "getwchar" => {
                let stream = self.standard_stream_object(CaptureStream::Stdin, span)?;
                self.eval_fgetwc_like_call(stream, span, "getwchar", "7.24.3.8")
            }
            "putwc" => {
                let stream = self
                    .stream_object_from_value(&evaluated[1], args[1].span(), "putwc", "7.24.3.9")?
                    .expect("checked by caller");
                self.eval_fputwc_like_call(
                    self.wchar_scalar_to_host(&evaluated[0], args[0].span(), "putwc")?,
                    stream,
                    args[1].span(),
                    "putwc",
                    "7.24.3.9",
                )
            }
            "putwchar" => {
                let stream = self.standard_stream_object(CaptureStream::Stdout, span)?;
                self.eval_fputwc_like_call(
                    self.wchar_scalar_to_host(&evaluated[0], args[0].span(), "putwchar")?,
                    stream,
                    span,
                    "putwchar",
                    "7.24.3.10",
                )
            }
            "ungetwc" => self.eval_ungetwc_call(evaluated, args, span),
            "wcstof" | "wcstod" | "wcstold" | "wcstol" | "wcstoll" | "wcstoul" | "wcstoull" => {
                self.eval_wcsto_call(function_name, evaluated, args, span, objects)
            }
            "wcslen" => self.eval_wcslen_call(evaluated, args, objects),
            "wcscmp" => self.eval_wcscmp_call(evaluated, args, objects),
            "wcsncmp" => self.eval_wcsncmp_call(evaluated, args, objects),
            "wcschr" => self.eval_wcschr_call(evaluated, args, span, objects),
            "wcsrchr" => self.eval_wcsrchr_call(evaluated, args, span, objects),
            "wcsspn" => self.eval_wcsspn_call(evaluated, args, objects),
            "wcscspn" => self.eval_wcscspn_call(evaluated, args, objects),
            "wcspbrk" => self.eval_wcspbrk_call(evaluated, args, span, objects),
            "wcsstr" => self.eval_wcsstr_call(evaluated, args, span, objects),
            "wmemchr" => self.eval_wmemchr_call(evaluated, args, span, objects),
            "wmemcmp" => self.eval_wmemcmp_call(evaluated, args, objects),
            "wmemset" => self.eval_wmemset_call(evaluated, args, objects),
            "wcstok" => self.eval_wcstok_call(evaluated, args, span, objects),
            "wcscpy" => self.eval_wcscpy_call(evaluated, args, objects),
            "wcsncpy" => self.eval_wcsncpy_call(evaluated, args, objects),
            "wmemcpy" | "wmemmove" => {
                self.eval_wide_memory_copy_call(function_name, evaluated, args, objects)
            }
            "wcscat" => self.eval_wcscat_call(evaluated, args, objects),
            "wcsncat" => self.eval_wcsncat_call(evaluated, args, objects),
            "wcscoll" => self.eval_wcscoll_call(evaluated, args, span, objects),
            "wcsxfrm" => self.eval_wcsxfrm_call(evaluated, args, objects),
            "wcsftime" => self.eval_wcsftime_call(evaluated, args, span, objects),
            "strtof" | "strtod" | "strtold" | "strtol" | "strtoul" | "strtoll" | "strtoull"
            | "strtoimax" | "strtoumax" => {
                self.eval_strto_call(function_name, evaluated, args, span, objects)
            }
            "rand" => {
                self.rand_state = self
                    .rand_state
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(12_345)
                    & 0x7fff_ffff;
                Ok(TypedValue::int(i128::from(self.rand_state)))
            }
            "srand" => {
                self.rand_state = evaluated[0].to_int()? as u32;
                Ok(TypedValue::void())
            }
            "div" | "ldiv" | "lldiv" | "imaxdiv" => {
                self.eval_div_call(function_name, evaluated, span)
            }
            "imaxabs" => {
                let value = evaluated[0].to_int()?;
                let result = if value < 0 {
                    self.ensure_integer_ub_range(-value, &CType::Long, span, "7.8.2.1")?
                } else {
                    value
                };
                Ok(TypedValue::integer(CType::Long, result))
            }
            "malloc" => self.eval_malloc_call(span, objects),
            "aligned_alloc" => self.eval_aligned_alloc_call(evaluated, span, objects),
            "calloc" => self.eval_calloc_call(span, objects),
            "realloc" => self.eval_realloc_call(span, objects),
            "free" => self.eval_free_call(span, objects),
            "abort" => self.eval_abort_call(span),
            "atexit" => self.eval_atexit_call(evaluated, span),
            "at_quick_exit" => self.eval_at_quick_exit_call(evaluated, span),
            "exit" => self.eval_exit_call(evaluated, span, true, false),
            "quick_exit" => self.eval_exit_call(evaluated, span, false, true),
            "_Exit" => self.eval_exit_call(evaluated, span, false, false),
            "bsearch" => self.eval_bsearch_call(evaluated, args, span, frame, objects),
            "qsort" => self.eval_qsort_call(evaluated, args, span, frame, objects),
            "getenv" => self.eval_getenv_call(evaluated, args, span, objects),
            "system" => self.eval_system_call(evaluated, args, span, objects),
            "setlocale" => self.eval_setlocale_call(evaluated, args, span, objects),
            "localeconv" => self.eval_localeconv_call(span, objects),
            "signal" => self.eval_signal_call(evaluated, span),
            "raise" => self.eval_raise_call(evaluated, span, objects),
            "__codex_sig_dfl" | "__codex_sig_ign" | "__codex_sig_err" => Ok(TypedValue::void()),
            "clock" => Ok(TypedValue::integer(CType::UnsignedLong, unsafe {
                clock() as i128
            })),
            "difftime" => Ok(TypedValue::floating(CType::Double, unsafe {
                difftime(
                    evaluated[0].to_int()? as libc::time_t,
                    evaluated[1].to_int()? as libc::time_t,
                )
            })),
            "mktime" => self.eval_mktime_call(evaluated, args, span, objects),
            "time" => self.eval_time_call(evaluated, args, span, objects),
            "asctime" => self.eval_asctime_call(evaluated, args, span, objects),
            "ctime" => self.eval_ctime_call(evaluated, args, span, objects),
            "gmtime" => self.eval_gmtime_like_call("gmtime", evaluated, args, span, objects),
            "localtime" => self.eval_gmtime_like_call("localtime", evaluated, args, span, objects),
            "strftime" => self.eval_strftime_call(evaluated, args, span, objects),
            "timespec_get" => self.eval_timespec_get_call(evaluated, args, span, objects),
            "feclearexcept" => {
                Ok(self
                    .eval_fenv_scalar_call("feclearexcept", Some(evaluated[0].to_int()? as c_int)))
            }
            "fegetexceptflag" => self.eval_fegetexceptflag_call(evaluated, args, span, objects),
            "feraiseexcept" => {
                Ok(self
                    .eval_fenv_scalar_call("feraiseexcept", Some(evaluated[0].to_int()? as c_int)))
            }
            "fesetexceptflag" => self.eval_fesetexceptflag_call(evaluated, args, span, objects),
            "fetestexcept" => {
                Ok(self
                    .eval_fenv_scalar_call("fetestexcept", Some(evaluated[0].to_int()? as c_int)))
            }
            "fegetround" => Ok(self.eval_fenv_scalar_call("fegetround", None)),
            "fesetround" => {
                Ok(self.eval_fenv_scalar_call("fesetround", Some(evaluated[0].to_int()? as c_int)))
            }
            "fegetenv" => self.eval_fegetenv_like_call("fegetenv", evaluated, args, span, objects),
            "feholdexcept" => {
                self.eval_fegetenv_like_call("feholdexcept", evaluated, args, span, objects)
            }
            "fesetenv" => self.eval_fesetenv_like_call("fesetenv", evaluated, args, span, objects),
            "feupdateenv" => {
                self.eval_fesetenv_like_call("feupdateenv", evaluated, args, span, objects)
            }
            "__codex_fe_dfl_env" => self.eval_fe_dfl_env_call(span, objects),
            "memcpy" | "memmove" => {
                self.eval_memory_copy_call(function_name, evaluated, args, frame, objects)
            }
            "memset" => self.eval_memset_call(evaluated, args, span, objects),
            "memchr" => self.eval_memchr_call(evaluated, args, span, objects),
            "memcmp" => self.eval_memcmp_call(evaluated, args, span, objects),
            "__codex_setjmp" => self.eval_setjmp_call(evaluated, span, frame, objects),
            "longjmp" => self.eval_longjmp_call(evaluated, span, objects),
            "strlen" => self.eval_strlen_call(evaluated, args, span, objects),
            "strcmp" => self.eval_strcmp_call(evaluated, args, span, objects),
            "strncmp" => self.eval_strncmp_call(evaluated, args, span, objects),
            "strcoll" => self.eval_strcoll_call(evaluated, args, span, objects),
            "strcpy" => self.eval_strcpy_call(evaluated, args, span, objects),
            "strncpy" => self.eval_strncpy_call(evaluated, args, span, objects),
            "strcat" => self.eval_strcat_call(evaluated, args, span, objects),
            "strncat" => self.eval_strncat_call(evaluated, args, span, objects),
            "strxfrm" => self.eval_strxfrm_call(evaluated, args, span, objects),
            "strerror" => self.eval_strerror_call(evaluated, args, span),
            "strdup" => self.eval_strdup_call(evaluated, args, span, objects),
            "strchr" => self.eval_strchr_call(evaluated, args, span, objects),
            "strrchr" => self.eval_strrchr_call(evaluated, args, span, objects),
            "strspn" => self.eval_strspn_call(evaluated, args, span, objects),
            "strcspn" => self.eval_strcspn_call(evaluated, args, span, objects),
            "strpbrk" => self.eval_strpbrk_call(evaluated, args, span, objects),
            "strstr" => self.eval_strstr_call(evaluated, args, span, objects),
            "strtok" => self.eval_strtok_call(evaluated, args, span, objects),
            _ if Self::is_host_complex_function(function_name) => {
                self.eval_complex_call(function_name, evaluated, span)
            }
            "__codex_math_errhandling"
            | "__codex_huge_val"
            | "__codex_huge_valf"
            | "__codex_huge_vall" => self.eval_math_helper_nullary_call(function_name, span),
            "__codex_fpclassify"
            | "__codex_fpclassifyf"
            | "__codex_fpclassifyl"
            | "__codex_isfinite"
            | "__codex_isinf"
            | "__codex_isnan"
            | "__codex_isnormal"
            | "__codex_isnormalf"
            | "__codex_isnormall"
            | "__codex_signbit" => self.eval_math_unary_helper_call(function_name, evaluated, span),
            "__codex_isgreater"
            | "__codex_isgreaterequal"
            | "__codex_isless"
            | "__codex_islessequal"
            | "__codex_islessgreater"
            | "__codex_isunordered" => {
                self.eval_math_binary_helper_call(function_name, evaluated, span)
            }
            "acos" | "acosf" | "acosl" | "asin" | "asinf" | "asinl" | "atan" | "atanf"
            | "atanl" | "cos" | "cosf" | "cosl" | "sin" | "sinf" | "sinl" | "tan" | "tanf"
            | "tanl" | "acosh" | "acoshf" | "acoshl" | "asinh" | "asinhf" | "asinhl" | "atanh"
            | "atanhf" | "atanhl" | "cosh" | "coshf" | "coshl" | "sinh" | "sinhf" | "sinhl"
            | "tanh" | "tanhf" | "tanhl" | "exp" | "expf" | "expl" | "log" | "logf" | "logl"
            | "log10" | "log10f" | "log10l" | "exp2" | "exp2f" | "exp2l" | "expm1" | "expm1f"
            | "expm1l" | "log1p" | "log1pf" | "log1pl" | "log2" | "log2f" | "log2l" | "logb"
            | "logbf" | "logbl" | "sqrt" | "sqrtf" | "sqrtl" | "cbrt" | "cbrtf" | "cbrtl"
            | "erf" | "erff" | "erfl" | "erfc" | "erfcf" | "erfcl" | "tgamma" | "tgammaf"
            | "tgammal" | "lgamma" | "lgammaf" | "lgammal" | "fabs" | "fabsf" | "fabsl"
            | "ceil" | "ceilf" | "ceill" | "floor" | "floorf" | "floorl" | "trunc" | "truncf"
            | "truncl" | "round" | "roundf" | "roundl" | "rint" | "rintf" | "rintl"
            | "nearbyint" | "nearbyintf" | "nearbyintl" => {
                self.eval_unary_real_math_call(function_name, evaluated, span, objects)
            }
            "atan2" | "atan2f" | "atan2l" | "pow" | "powf" | "powl" | "hypot" | "hypotf"
            | "hypotl" | "fmod" | "fmodf" | "fmodl" | "remainder" | "remainderf" | "remainderl"
            | "copysign" | "copysignf" | "copysignl" | "nextafter" | "nextafterf"
            | "nextafterl" | "fdim" | "fdimf" | "fdiml" | "fmax" | "fmaxf" | "fmaxl" | "fmin"
            | "fminf" | "fminl" => self.eval_binary_real_math_call(function_name, evaluated, span),
            "nexttoward" | "nexttowardf" | "nexttowardl" => {
                self.eval_nexttoward_math_call(function_name, evaluated, span)
            }
            "fma" | "fmaf" | "fmal" => {
                self.eval_ternary_real_math_call(function_name, evaluated, span)
            }
            "ldexp" | "ldexpf" | "ldexpl" | "scalbn" | "scalbnf" | "scalbnl" => {
                self.eval_real_int_math_call(function_name, evaluated, span)
            }
            "scalbln" | "scalblnf" | "scalblnl" => {
                self.eval_real_long_math_call(function_name, evaluated, span)
            }
            "ilogb" | "ilogbf" | "ilogbl" | "lround" | "lroundf" | "lroundl" | "lrint"
            | "lrintf" | "lrintl" | "llround" | "llroundf" | "llroundl" | "llrint" | "llrintf"
            | "llrintl" => self.eval_unary_int_math_result_call(function_name, evaluated, span),
            "frexp" | "frexpf" | "frexpl" => {
                self.eval_frexp_math_call(function_name, evaluated, args, span, objects)
            }
            "modf" | "modff" | "modfl" => {
                self.eval_modf_math_call(function_name, evaluated, args, span, objects)
            }
            "remquo" | "remquof" | "remquol" => {
                self.eval_remquo_math_call(function_name, evaluated, args, span, objects)
            }
            "nan" | "nanf" | "nanl" => {
                self.eval_nan_math_call(function_name, evaluated, args, span, objects)
            }
            _ => Err(Diagnostic::error(
                format!("unsupported host library function {}", function_name),
                span,
            )),
        };
        if function_name != "__codex_errno_location" {
            self.sync_host_errno_from_host(objects, span)?;
        }
        result
    }

    pub(super) fn checked_usize_from_unsigned_long(
        &self,
        value: &TypedValue,
        span: Span,
        what: &str,
    ) -> Result<usize, Diagnostic> {
        usize::try_from(value.to_int()?)
            .map_err(|_| Diagnostic::error(format!("{what} is out of supported range"), span))
    }

    fn unsigned_long_pointer_type(&self) -> CType {
        CType::pointer_to(CType::UnsignedLong)
    }

    fn setjmp_buffer_pointer(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
    ) -> Result<PointerValue, Diagnostic> {
        if !self.pointer_assignment_compatible(&self.unsigned_long_pointer_type(), &value.ty) {
            return Err(Diagnostic::error(
                format!("{function_name} requires a jmp_buf argument"),
                span,
            ));
        }
        value.as_pointer(span)
    }

    pub(super) fn pointer_lvalue(
        &self,
        function_name: &str,
        pointer: &PointerValue,
        pointed_ty: &CType,
        span: Span,
        standard: &'static str,
    ) -> Result<LValue, Diagnostic> {
        let Some(object) = pointer.object else {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a valid pointer argument"),
                span,
                Some(standard),
            ));
        };
        Ok(LValue {
            object,
            ty: pointed_ty.clone(),
            base_offset: pointer.base_offset,
            offset: pointer.offset,
            member_path: pointer.member_path.clone(),
            designated_root_ty: pointer.designated_root_ty.clone(),
            byte_offset_override: pointer.byte_offset_override,
            arithmetic_domain_start: pointer.arithmetic_domain_start,
            object_representation_domain: pointer.object_representation_domain,
            bit_field_width: None,
            restrict_source: None,
        })
    }

    fn read_unsigned_long_at_pointer(
        &mut self,
        function_name: &str,
        pointer: &PointerValue,
        span: Span,
        objects: &ObjectFrames,
        standard: &'static str,
    ) -> Result<u64, Diagnostic> {
        let lvalue =
            self.pointer_lvalue(function_name, pointer, &CType::UnsignedLong, span, standard)?;
        let value = self.load_lvalue(lvalue, span, objects)?;
        u64::try_from(value.to_int()?).map_err(|_| {
            Diagnostic::ub(
                format!("{function_name} environment has an invalid representation"),
                span,
                Some(standard),
            )
        })
    }

    fn write_unsigned_long_at_pointer(
        &mut self,
        function_name: &str,
        pointer: &PointerValue,
        value: u64,
        span: Span,
        objects: &mut ObjectFrames,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let lvalue =
            self.pointer_lvalue(function_name, pointer, &CType::UnsignedLong, span, standard)?;
        self.store_lvalue(
            objects,
            &lvalue,
            TypedValue::integer(CType::UnsignedLong, value as i128),
            span,
        )
    }

    fn check_setjmp_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("__codex_setjmp", args, evaluated, 1, span)?;
        let value = &evaluated[0];
        self.check_library_argument_value(value, args[0].span(), "setjmp")?;
        let pointer = self.setjmp_buffer_pointer("setjmp", value, args[0].span())?;
        let size = self.type_size_of(&CType::UnsignedLong).unwrap_or(8);
        let (object_id, start, _) =
            self.library_array_region(&pointer, size, size, args[0].span(), objects)?;
        self.ensure_library_writable_region(object_id, start, size, args[0].span(), objects)?;
        let Some(frame_id) = self.current_frame_id() else {
            return Err(Diagnostic::error(
                "internal error: setjmp requires an active function invocation",
                span,
            ));
        };
        let context = self
            .active_setjmp_contexts
            .iter()
            .rev()
            .find(|context| context.frame_id == frame_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "setjmp must appear only as an entire expression statement or controlling expression",
                    span,
                    Some("7.13.1.1"),
                )
            })?;
        if !self.setjmp_call_allowed_in_context(&context, span, frame, objects)? {
            return Err(Diagnostic::ub(
                "setjmp must appear only as an entire expression statement or controlling expression",
                span,
                Some("7.13.1.1"),
            ));
        }
        Ok(())
    }

    fn check_longjmp_call(
        &mut self,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.require_exact_call_args("longjmp", args, evaluated, 2, span)?;
        self.reject_missing_return_value(&evaluated[0], args[0].span())?;
        self.reject_missing_return_value(&evaluated[1], args[1].span())?;
        self.reject_indeterminate_library_value(&evaluated[0], args[0].span(), "longjmp")?;
        self.reject_indeterminate_library_value(&evaluated[1], args[1].span(), "longjmp")?;
        let pointer = self.setjmp_buffer_pointer("longjmp", &evaluated[0], args[0].span())?;
        let size = self.type_size_of(&CType::UnsignedLong).unwrap_or(8);
        let (object_id, start, _) =
            self.library_array_region(&pointer, size, size, args[0].span(), objects)?;
        if evaluated[1].ty != CType::Int {
            return Err(Diagnostic::error(
                "longjmp requires an int second argument",
                args[1].span(),
            ));
        }
        let handle = self.read_unsigned_long_at_pointer(
            "longjmp",
            &pointer,
            args[0].span(),
            objects,
            "7.13.2.1",
        )?;
        let saved_bytes = self
            .setjmp_provenance
            .get(&(object_id, start))
            .filter(|(saved_handle, _)| *saved_handle == handle)
            .map(|(_, bytes)| bytes.clone());
        let current_bytes =
            self.array_region_snapshot(object_id, start, size, args[0].span(), objects)?;
        if saved_bytes.as_deref() != Some(current_bytes.as_slice()) {
            return Err(Diagnostic::ub(
                "longjmp requires an unmodified jmp_buf value produced by setjmp in the same object",
                args[0].span(),
                Some("7.13.2.1"),
            ));
        }
        let env = self.setjmp_envs.get(&handle).ok_or_else(|| {
            Diagnostic::ub(
                "longjmp was called with an invalid or uninitialized jmp_buf",
                span,
                Some("7.13.2.1"),
            )
        })?;
        if !self.current_frame_ids.contains(&env.frame_id) {
            return Err(Diagnostic::ub(
                "longjmp was called after the function containing the corresponding setjmp had already returned",
                span,
                Some("7.13.2.1"),
            ));
        }
        Ok(())
    }

    fn eval_setjmp_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
        frame: &Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let Some(frame_id) = self.current_frame_id() else {
            return Err(Diagnostic::error(
                "internal error: setjmp requires an active function invocation",
                span,
            ));
        };
        let context = self
            .active_setjmp_contexts
            .iter()
            .rev()
            .find(|context| context.frame_id == frame_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "setjmp must appear only as an entire expression statement or controlling expression",
                    span,
                    Some("7.13.1.1"),
                )
            })?;
        if !self.setjmp_call_allowed_in_context(&context, span, frame, objects)? {
            return Err(Diagnostic::ub(
                "setjmp must appear only as an entire expression statement or controlling expression",
                span,
                Some("7.13.1.1"),
            ));
        }
        let pointer = self.setjmp_buffer_pointer("setjmp", &evaluated[0], span)?;

        if let Some(pending) = self.pending_longjmp_return
            && pending.frame_id == frame_id
        {
            self.pending_longjmp_return = None;
            return Ok(TypedValue::int(if pending.value == 0 {
                1
            } else {
                pending.value
            }));
        }

        let handle = self.next_setjmp_handle;
        self.next_setjmp_handle += 1;
        self.write_unsigned_long_at_pointer("setjmp", &pointer, handle, span, objects, "7.13.1.1")?;
        let env = self.capture_setjmp_environment(frame, objects, &context)?;
        self.setjmp_envs.insert(handle, env);
        self.live_setjmp_frames.insert(frame_id);
        let size = self.type_size_of(&CType::UnsignedLong).unwrap_or(8);
        let (object_id, start, _) =
            self.library_array_region(&pointer, size, size, span, objects)?;
        let bytes = self.array_region_snapshot(object_id, start, size, span, objects)?;
        self.setjmp_provenance
            .insert((object_id, start), (handle, bytes));
        Ok(TypedValue::int(0))
    }

    fn eval_longjmp_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = self.setjmp_buffer_pointer("longjmp", &evaluated[0], span)?;
        let handle =
            self.read_unsigned_long_at_pointer("longjmp", &pointer, span, objects, "7.13.2.1")?;
        let env = self.setjmp_envs.get(&handle).cloned().ok_or_else(|| {
            Diagnostic::ub(
                "longjmp was called with an invalid or uninitialized jmp_buf",
                span,
                Some("7.13.2.1"),
            )
        })?;
        if !self.current_frame_ids.contains(&env.frame_id) {
            return Err(Diagnostic::ub(
                "longjmp was called after the function containing the corresponding setjmp had already returned",
                span,
                Some("7.13.2.1"),
            ));
        }
        let value = evaluated[1].to_int()?;
        panic_any(LongjmpSignal {
            handle,
            target_frame_id: env.frame_id,
            value: if value == 0 { 1 } else { value },
        });
    }
}
