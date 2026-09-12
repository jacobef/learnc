use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn c_string_argument(
        &mut self,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<CString, Diagnostic> {
        CString::new(self.read_c_string_bytes(value.as_pointer(span)?, span, objects)?)
            .map_err(|_| Diagnostic::error("string argument contains an interior nul byte", span))
    }

    pub(super) fn wide_c_string_argument(
        &mut self,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<libc::wchar_t>, Diagnostic> {
        let mut units = self.read_wide_string_units(value.as_pointer(span)?, span, objects)?;
        units.push(0);
        Ok(units)
    }

    fn intern_host_c_string_pointer(
        &mut self,
        ty: CType,
        ptr: *mut c_char,
        pinned_ids: &mut Vec<ObjectId>,
    ) -> TypedValue {
        let pointer = if ptr.is_null() {
            Self::null_pointer()
        } else {
            let pointer = self.intern_readonly_c_bytes(unsafe { CStr::from_ptr(ptr) }.to_bytes());
            if let Some(id) = pointer.object {
                pinned_ids.push(id);
            }
            pointer
        };
        self.pointer_value_with_type(ty, pointer)
    }

    fn write_strto_end_pointer(
        &mut self,
        function_name: &str,
        endptr_arg: &TypedValue,
        endptr_span: Span,
        source_pointer: &PointerValue,
        offset: usize,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let endptr_pointer = endptr_arg.as_pointer(endptr_span)?;
        if endptr_pointer.is_null() {
            return Ok(());
        }
        let result_pointer =
            self.pointer_with_byte_offset(source_pointer, offset, endptr_span, objects)?;
        let lvalue = self.pointer_lvalue(
            function_name,
            &endptr_pointer,
            &self.char_ptr_type(),
            endptr_span,
            "7.20.1",
        )?;
        self.store_lvalue(
            objects,
            &lvalue,
            self.pointer_value_with_type(self.char_ptr_type(), result_pointer),
            endptr_span,
        )
    }

    fn write_wcsto_end_pointer(
        &mut self,
        function_name: &str,
        endptr_arg: &TypedValue,
        endptr_span: Span,
        source_pointer: &PointerValue,
        unit_offset: usize,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let endptr_pointer = endptr_arg.as_pointer(endptr_span)?;
        if endptr_pointer.is_null() {
            return Ok(());
        }
        let result_pointer = self.pointer_with_byte_offset(
            source_pointer,
            unit_offset
                .checked_mul(self.wide_char_byte_width())
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "wide end-pointer offset is out of supported range",
                        endptr_span,
                        Some("7.24.4.1"),
                    )
                })?,
            endptr_span,
            objects,
        )?;
        let lvalue = self.pointer_lvalue(
            function_name,
            &endptr_pointer,
            &self.wchar_ptr_type(),
            endptr_span,
            "7.24.4.1",
        )?;
        self.store_lvalue(
            objects,
            &lvalue,
            self.pointer_value_with_type(self.wchar_ptr_type(), result_pointer),
            endptr_span,
        )
    }

    pub(super) fn eval_atof_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let result = unsafe { atof(input.as_ptr()) } as f64;
        Ok(TypedValue::floating(
            self.host_function_return_type("atof", span.file, span)?,
            result,
        ))
    }

    pub(super) fn eval_atoi_like_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let result_ty = self.host_function_return_type(function_name, span.file, span)?;
        let value = match function_name {
            "atoi" => unsafe { atoi(input.as_ptr()) as i128 },
            "atol" => unsafe { atol(input.as_ptr()) as i128 },
            "atoll" => unsafe { atoll(input.as_ptr()) as i128 },
            _ => unreachable!("checked by caller"),
        };
        let value = self.ensure_integer_ub_range(value, &result_ty, args[0].span(), "7.20.1.3")?;
        Ok(TypedValue::integer(result_ty, value))
    }

    pub(super) fn eval_strto_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let input = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let mut endptr: *mut c_char = std::ptr::null_mut();
        let result_ty = self.host_function_return_type(function_name, span.file, span)?;
        let result = match function_name {
            "strtof" => {
                let value = unsafe { strtof(input.as_ptr(), &mut endptr) } as f64;
                TypedValue::floating(result_ty, value)
            }
            "strtod" => {
                let value = unsafe { strtod(input.as_ptr(), &mut endptr) };
                TypedValue::floating(result_ty, value)
            }
            "strtold" => {
                let value = unsafe { strtold(input.as_ptr(), &mut endptr) } as f64;
                TypedValue::floating(result_ty, value)
            }
            "strtol" => {
                let value =
                    unsafe { strtol(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int) }
                        as i128;
                TypedValue::integer(result_ty, value)
            }
            "strtoul" => {
                let value = unsafe {
                    strtoul(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
                } as i128;
                TypedValue::integer(result_ty, value)
            }
            "strtoll" => {
                let value = unsafe {
                    strtoll(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
                } as i128;
                TypedValue::integer(result_ty, value)
            }
            "strtoull" => {
                let value = unsafe {
                    strtoull(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
                } as i128;
                TypedValue::integer(result_ty, value)
            }
            "strtoimax" => {
                let value =
                    unsafe { strtol(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int) }
                        as i128;
                TypedValue::integer(result_ty, value)
            }
            "strtoumax" => {
                let value = unsafe {
                    strtoul(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
                } as i128;
                TypedValue::integer(result_ty, value)
            }
            _ => unreachable!("checked by caller"),
        };
        let offset = if endptr.is_null() {
            0
        } else {
            unsafe { endptr.offset_from(input.as_ptr()) as usize }
        };
        self.write_strto_end_pointer(
            function_name,
            &evaluated[1],
            args[1].span(),
            &source_pointer,
            offset,
            objects,
        )?;
        Ok(result)
    }

    pub(super) fn eval_wcslen_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let units = self.read_wide_string_units(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        Ok(TypedValue::integer(
            CType::UnsignedLong,
            units.len() as i128,
        ))
    }

    pub(super) fn eval_wcscmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = self.read_wide_string_units(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let rhs = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        Ok(TypedValue::int(match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }))
    }

    pub(super) fn eval_wcsncmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsncmp count")?;
        let lhs = self.read_bounded_wide_source(
            evaluated[0].as_pointer(args[0].span())?,
            count,
            args[0].span(),
            objects,
        )?;
        let rhs = self.read_bounded_wide_source(
            evaluated[1].as_pointer(args[1].span())?,
            count,
            args[1].span(),
            objects,
        )?;
        Ok(TypedValue::int(match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }))
    }

    pub(super) fn eval_wcschr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let mut units = self.read_wide_string_units(pointer.clone(), args[0].span(), objects)?;
        units.push(0);
        let needle = libc::wchar_t::try_from(evaluated[1].to_int()?).map_err(|_| {
            Diagnostic::ub(
                "wcschr character argument is not representable as wchar_t",
                args[1].span(),
                Some("7.24.4.2"),
            )
        })?;
        let return_ty = self.host_function_return_type("wcschr", span.file, span)?;
        let Some(index) = units.iter().position(|&unit| unit == needle) else {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        };
        let result_pointer = self.pointer_with_byte_offset(
            &pointer,
            index * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_wcsrchr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let mut units = self.read_wide_string_units(pointer.clone(), args[0].span(), objects)?;
        units.push(0);
        let needle = libc::wchar_t::try_from(evaluated[1].to_int()?).map_err(|_| {
            Diagnostic::ub(
                "wcsrchr character argument is not representable as wchar_t",
                args[1].span(),
                Some("7.24.4.5"),
            )
        })?;
        let return_ty = self.host_function_return_type("wcsrchr", span.file, span)?;
        let Some(index) = units.iter().rposition(|&unit| unit == needle) else {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        };
        let result_pointer = self.pointer_with_byte_offset(
            &pointer,
            index * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_wcsspn_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source = self.read_wide_string_units(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let accept = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let len = source
            .iter()
            .take_while(|unit| accept.contains(unit))
            .count();
        Ok(TypedValue::integer(CType::UnsignedLong, len as i128))
    }

    pub(super) fn eval_wcscspn_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source = self.read_wide_string_units(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let reject = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let len = source
            .iter()
            .take_while(|unit| !reject.contains(unit))
            .count();
        Ok(TypedValue::integer(CType::UnsignedLong, len as i128))
    }

    pub(super) fn eval_wcspbrk_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let source = self.read_wide_string_units(pointer.clone(), args[0].span(), objects)?;
        let accept = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let return_ty = self.host_function_return_type("wcspbrk", span.file, span)?;
        let Some(index) = source.iter().position(|unit| accept.contains(unit)) else {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        };
        let result_pointer = self.pointer_with_byte_offset(
            &pointer,
            index * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_wcsstr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let haystack = self.read_wide_string_units(pointer.clone(), args[0].span(), objects)?;
        let needle = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let return_ty = self.host_function_return_type("wcsstr", span.file, span)?;
        if needle.is_empty() {
            return Ok(self.pointer_value_with_type(return_ty, pointer));
        }
        let Some(index) = haystack
            .windows(needle.len())
            .position(|window| window == needle.as_slice())
        else {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        };
        let result_pointer = self.pointer_with_byte_offset(
            &pointer,
            index * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_wmemchr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wmemchr count")?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let units = self.read_exact_wide_units(pointer.clone(), count, args[0].span(), objects)?;
        let needle = libc::wchar_t::try_from(evaluated[1].to_int()?).map_err(|_| {
            Diagnostic::ub(
                "wmemchr character argument is not representable as wchar_t",
                args[1].span(),
                Some("7.24.4.10"),
            )
        })?;
        let return_ty = self.host_function_return_type("wmemchr", span.file, span)?;
        let Some(index) = units.iter().position(|&unit| unit == needle) else {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        };
        let result_pointer = self.pointer_with_byte_offset(
            &pointer,
            index * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_wmemcmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wmemcmp count")?;
        let lhs = self.read_exact_wide_units(
            evaluated[0].as_pointer(args[0].span())?,
            count,
            args[0].span(),
            objects,
        )?;
        let rhs = self.read_exact_wide_units(
            evaluated[1].as_pointer(args[1].span())?,
            count,
            args[1].span(),
            objects,
        )?;
        Ok(TypedValue::int(match lhs.cmp(&rhs) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }))
    }

    pub(super) fn eval_wmemset_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wmemset count")?;
        let dest = evaluated[0].clone();
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let fill = libc::wchar_t::try_from(evaluated[1].to_int()?).map_err(|_| {
            Diagnostic::ub(
                "wmemset fill argument is not representable as wchar_t",
                args[1].span(),
                Some("7.24.4.6"),
            )
        })?;
        let units = vec![fill; count];
        self.write_wide_units_to_pointer(&dest_pointer, &units, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wcstok_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        self.sequence_point();
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let delim = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let state_pointer = evaluated[2].as_pointer(args[2].span())?;
        let state_lvalue = self.pointer_lvalue(
            "wcstok",
            &state_pointer,
            &CType::pointer_to(self.wchar_type()),
            args[2].span(),
            "7.24.5.8",
        )?;
        let current = if source_pointer.is_null() {
            let saved = self.load_lvalue(state_lvalue.clone(), args[2].span(), objects)?;
            saved.as_pointer(args[2].span())?
        } else {
            source_pointer
        };
        self.sequence_point();
        let return_ty = self.host_function_return_type("wcstok", span.file, span)?;
        if current.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let units = self.read_wide_string_units(current.clone(), args[0].span(), objects)?;
        let mut start = 0usize;
        while start < units.len() && delim.contains(&units[start]) {
            start += 1;
        }
        if start == units.len() {
            let saved = Self::null_pointer();
            self.store_lvalue(
                objects,
                &state_lvalue,
                self.pointer_value_with_type(CType::pointer_to(self.wchar_type()), saved.clone()),
                args[2].span(),
            )?;
            self.record_wcstok_state(&state_pointer, saved, args[2].span(), objects)?;
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let mut end = start;
        while end < units.len() && !delim.contains(&units[end]) {
            end += 1;
        }
        let result_pointer = self.pointer_with_byte_offset(
            &current,
            start * self.wide_char_byte_width(),
            args[0].span(),
            objects,
        )?;
        if end < units.len() {
            let zero_pointer = self.pointer_with_byte_offset(
                &current,
                end * self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?;
            self.write_wide_units_to_pointer(&zero_pointer, &[0], args[0].span(), objects)?;
            let next_pointer = self.pointer_with_byte_offset(
                &current,
                (end + 1) * self.wide_char_byte_width(),
                args[0].span(),
                objects,
            )?;
            self.store_lvalue(
                objects,
                &state_lvalue,
                self.pointer_value_with_type(
                    CType::pointer_to(self.wchar_type()),
                    next_pointer.clone(),
                ),
                args[2].span(),
            )?;
            self.record_wcstok_state(&state_pointer, next_pointer, args[2].span(), objects)?;
        } else {
            let saved = Self::null_pointer();
            self.store_lvalue(
                objects,
                &state_lvalue,
                self.pointer_value_with_type(CType::pointer_to(self.wchar_type()), saved.clone()),
                args[2].span(),
            )?;
            self.record_wcstok_state(&state_pointer, saved, args[2].span(), objects)?;
        }
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    fn record_wcstok_state(
        &mut self,
        state_pointer: &PointerValue,
        saved: PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let pointer_size = self.type_size_of(&self.wchar_ptr_type()).unwrap_or(8);
        let (object_id, start, _) =
            self.byte_region_from_pointer(state_pointer, pointer_size, span, objects)?;
        let version = self
            .lookup_object(objects, object_id)
            .expect("wcstok state object exists")
            .modification_count;
        self.wcstok_state_provenance
            .insert((object_id, start), (version, saved));
        Ok(())
    }

    pub(super) fn eval_div_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let return_ty = self.host_function_return_type(function_name, span.file, span)?;
        let record = self.record_type(&return_ty).cloned().ok_or_else(|| {
            Diagnostic::error(
                format!("{function_name} does not return a structure type"),
                span,
            )
        })?;
        let (quot, rem) = match function_name {
            "div" => {
                let result = unsafe {
                    div(
                        evaluated[0].to_int()? as c_int,
                        evaluated[1].to_int()? as c_int,
                    )
                };
                (result.quot as i128, result.rem as i128)
            }
            "ldiv" => {
                let numer = evaluated[0].to_int()?;
                let denom = evaluated[1].to_int()?;
                (
                    self.integer_div_for_type(numer, denom, &CType::Long, span)?,
                    self.integer_rem_for_type(numer, denom, &CType::Long, span)?,
                )
            }
            "lldiv" => {
                let result = unsafe {
                    lldiv(
                        evaluated[0].to_int()? as c_longlong,
                        evaluated[1].to_int()? as c_longlong,
                    )
                };
                (result.quot as i128, result.rem as i128)
            }
            "imaxdiv" => {
                let numer = evaluated[0].to_int()?;
                let denom = evaluated[1].to_int()?;
                (
                    self.integer_div_for_type(numer, denom, &CType::Long, span)?,
                    self.integer_rem_for_type(numer, denom, &CType::Long, span)?,
                )
            }
            _ => unreachable!("checked by caller"),
        };
        let mut members = Vec::with_capacity(record.members.len());
        for member in &record.members {
            let value = match member.storage_name.as_str() {
                "quot" => TypedValue::integer(member.ty.clone(), quot),
                "rem" => TypedValue::integer(member.ty.clone(), rem),
                _ => self.zero_value(&member.ty),
            };
            members.push((
                member.storage_name.as_str().into(),
                self.stored_value_from_typed_value(value, &member.ty, span)?,
            ));
        }
        Ok(self.typed_value_from_stored(&return_ty, &StoredValue::Record(members)))
    }

    pub(super) fn run_atexit_handlers(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let handlers = std::mem::take(&mut self.atexit_handlers);
        self.running_atexit = true;
        for symbol in handlers.into_iter().rev() {
            let function = self
                .lookup_function_symbol(&symbol)
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "atexit registered a function that is no longer defined",
                        span,
                        Some("7.20.4.2"),
                    )
                })?;
            let result = catch_unwind(AssertUnwindSafe(|| {
                self.call_function(
                    &function,
                    self.function_may_setjmp_symbol(&symbol),
                    Vec::new(),
                    span,
                    objects,
                )
            }));
            match result {
                Ok(result) => {
                    let _ = result?;
                }
                Err(payload) => {
                    if let Some(signal) = payload.downcast_ref::<TerminationSignal>().copied() {
                        self.running_atexit = false;
                        if signal.run_atexit {
                            return Err(Diagnostic::ub(
                                "calling exit from an atexit handler is undefined behavior",
                                span,
                                Some("7.20.4.3"),
                            ));
                        }
                        resume_unwind(payload);
                    }
                    if payload.downcast_ref::<LongjmpSignal>().is_some() {
                        self.running_atexit = false;
                        return Err(Diagnostic::ub(
                            "using longjmp from an atexit handler is undefined behavior",
                            span,
                            Some("7.20.4.2"),
                        ));
                    }
                    self.running_atexit = false;
                    resume_unwind(payload);
                }
            }
        }
        self.running_atexit = false;
        Ok(())
    }

    pub(super) fn run_quick_exit_handlers(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let handlers = std::mem::take(&mut self.quick_exit_handlers);
        self.running_quick_exit = true;
        for symbol in handlers.into_iter().rev() {
            let function = self
                .lookup_function_symbol(&symbol)
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "at_quick_exit registered a function that is no longer defined",
                        span,
                        Some("7.22.4.1"),
                    )
                })?;
            let result = catch_unwind(AssertUnwindSafe(|| {
                self.call_function(
                    &function,
                    self.function_may_setjmp_symbol(&symbol),
                    Vec::new(),
                    span,
                    objects,
                )
            }));
            match result {
                Ok(result) => {
                    let _ = result?;
                }
                Err(payload) => {
                    self.running_quick_exit = false;
                    if payload.downcast_ref::<LongjmpSignal>().is_some() {
                        return Err(Diagnostic::ub(
                            "using longjmp from an at_quick_exit handler is undefined behavior",
                            span,
                            Some("7.22.4.1"),
                        ));
                    }
                    resume_unwind(payload);
                }
            }
        }
        self.running_quick_exit = false;
        Ok(())
    }

    pub(super) fn eval_abort_call(&mut self, _span: Span) -> Result<TypedValue, Diagnostic> {
        panic_any(TerminationSignal {
            status: libc::EXIT_FAILURE,
            run_atexit: false,
            run_quick_exit: false,
        });
    }

    pub(super) fn eval_atexit_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let symbol = match &evaluated[0].data {
            ValueData::Function(name) => name.clone(),
            _ => {
                return Err(Diagnostic::error(
                    "atexit requires a callable function pointer",
                    span,
                ));
            }
        };
        self.atexit_handlers.push(symbol.to_string());
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_at_quick_exit_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let symbol = match &evaluated[0].data {
            ValueData::Function(name) => name.clone(),
            _ => {
                return Err(Diagnostic::error(
                    "at_quick_exit requires a callable function pointer",
                    span,
                ));
            }
        };
        self.quick_exit_handlers.push(symbol.to_string());
        Ok(TypedValue::int(0))
    }

    pub(super) fn eval_exit_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
        run_atexit: bool,
        run_quick_exit: bool,
    ) -> Result<TypedValue, Diagnostic> {
        if run_quick_exit && self.running_quick_exit {
            return Err(Diagnostic::ub(
                "quick_exit was called from an at_quick_exit handler",
                span,
                Some("7.22.4.7"),
            ));
        }
        panic_any(TerminationSignal {
            status: evaluated[0].to_int()? as c_int,
            run_atexit,
            run_quick_exit,
        });
    }

    pub(super) fn array_region_snapshot(
        &mut self,
        object_id: ObjectId,
        start: usize,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<ByteCell>, Diagnostic> {
        let object = self
            .lookup_object(objects, object_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        let all_bytes = self.serialize_stored_value(&object.ty, &object.value, span)?;
        Ok(all_bytes[start..start + size].to_vec())
    }

    fn qsort_compare_call(
        &mut self,
        function_symbol: &str,
        base_object: ObjectId,
        base_start: usize,
        total: usize,
        base_pointer: &PointerValue,
        size: usize,
        lhs_index: usize,
        rhs_index: usize,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<i128, Diagnostic> {
        let before = self.array_region_snapshot(base_object, base_start, total, span, objects)?;
        let function = self
            .lookup_function_symbol(function_symbol)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::error(
                    format!("comparison function {function_symbol} is not defined"),
                    span,
                )
            })?;
        let lhs_pointer =
            self.pointer_with_byte_offset(base_pointer, lhs_index * size, span, objects)?;
        let rhs_pointer =
            self.pointer_with_byte_offset(base_pointer, rhs_index * size, span, objects)?;
        let result = self.call_function(
            &function,
            self.function_may_setjmp_symbol(function_symbol),
            vec![
                self.pointer_value_with_type(self.const_void_ptr_type(), lhs_pointer),
                self.pointer_value_with_type(self.const_void_ptr_type(), rhs_pointer),
            ],
            span,
            objects,
        )?;
        self.reject_missing_return_value(&result, span)?;
        let cmp = result.to_int()?;
        let after = self.array_region_snapshot(base_object, base_start, total, span, objects)?;
        if before != after {
            return Err(Diagnostic::ub(
                "comparison function modified the array being searched or sorted",
                span,
                Some("7.20.5"),
            ));
        }
        Ok(cmp)
    }

    pub(super) fn check_comparator_total_order(
        &mut self,
        function_name: &str,
        function_symbol: &str,
        base_object: ObjectId,
        base_start: usize,
        total: usize,
        base_pointer: &PointerValue,
        nmemb: usize,
        size: usize,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<Vec<Vec<i8>>, Diagnostic> {
        let mut signs = vec![vec![0i8; nmemb]; nmemb];
        for (lhs, row) in signs.iter_mut().enumerate() {
            for (rhs, sign) in row.iter_mut().enumerate() {
                let first = self.qsort_compare_call(
                    function_symbol,
                    base_object,
                    base_start,
                    total,
                    base_pointer,
                    size,
                    lhs,
                    rhs,
                    span,
                    objects,
                )?;
                let second = self.qsort_compare_call(
                    function_symbol,
                    base_object,
                    base_start,
                    total,
                    base_pointer,
                    size,
                    lhs,
                    rhs,
                    span,
                    objects,
                )?;
                let first = first.signum() as i8;
                let second = second.signum() as i8;
                if first != second {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} comparison function returned inconsistent results for the same objects"
                        ),
                        span,
                        Some("7.22.5"),
                    ));
                }
                *sign = first;
            }
        }

        for lhs in 0..nmemb {
            if signs[lhs][lhs] != 0 {
                return Err(Diagnostic::ub(
                    format!(
                        "{function_name} comparison function does not compare an object equal to itself"
                    ),
                    span,
                    Some("7.22.5"),
                ));
            }
            for rhs in 0..nmemb {
                if signs[lhs][rhs] != -signs[rhs][lhs] {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} comparison function does not define a consistent ordering"
                        ),
                        span,
                        Some("7.22.5"),
                    ));
                }
                if signs[lhs][rhs] == 0
                    && (0..nmemb).any(|other| {
                        signs[lhs][other] != signs[rhs][other]
                            || signs[other][lhs] != signs[other][rhs]
                    })
                {
                    return Err(Diagnostic::ub(
                        format!(
                            "{function_name} comparison function has a non-transitive equivalence relation"
                        ),
                        span,
                        Some("7.22.5"),
                    ));
                }
            }
        }

        let mut representatives = Vec::new();
        for (index, row) in signs.iter().enumerate() {
            if !representatives
                .iter()
                .any(|&representative| row[representative] == 0)
            {
                representatives.push(index);
            }
        }
        let mut less_counts = representatives
            .iter()
            .map(|&lhs| {
                representatives
                    .iter()
                    .filter(|&&rhs| signs[lhs][rhs] < 0)
                    .count()
            })
            .collect::<Vec<_>>();
        less_counts.sort_unstable();
        if less_counts != (0..representatives.len()).collect::<Vec<_>>() {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} comparison function defines a cyclic, non-transitive ordering"
                ),
                span,
                Some("7.22.5"),
            ));
        }
        Ok(signs)
    }

    pub(super) fn bsearch_compare_call(
        &mut self,
        function_symbol: &str,
        key_pointer: &PointerValue,
        key_object: ObjectId,
        key_start: usize,
        base_pointer: &PointerValue,
        base_object: ObjectId,
        base_start: usize,
        total: usize,
        size: usize,
        index: usize,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<i128, Diagnostic> {
        let key_before = self.array_region_snapshot(key_object, key_start, size, span, objects)?;
        let base_before =
            self.array_region_snapshot(base_object, base_start, total, span, objects)?;
        let function = self
            .lookup_function_symbol(function_symbol)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::error(
                    format!("comparison function {function_symbol} is not defined"),
                    span,
                )
            })?;
        let element_pointer =
            self.pointer_with_byte_offset(base_pointer, index * size, span, objects)?;
        let result = self.call_function(
            &function,
            self.function_may_setjmp_symbol(function_symbol),
            vec![
                self.pointer_value_with_type(self.const_void_ptr_type(), key_pointer.clone()),
                self.pointer_value_with_type(self.const_void_ptr_type(), element_pointer),
            ],
            span,
            objects,
        )?;
        self.reject_missing_return_value(&result, span)?;
        let cmp = result.to_int()?;
        let key_after = self.array_region_snapshot(key_object, key_start, size, span, objects)?;
        let base_after =
            self.array_region_snapshot(base_object, base_start, total, span, objects)?;
        if key_before != key_after || base_before != base_after {
            return Err(Diagnostic::ub(
                "bsearch comparison function modified the key or array being searched",
                span,
                Some("7.22.5.1"),
            ));
        }
        Ok(cmp)
    }

    fn swap_qsort_elements(
        &mut self,
        object_id: ObjectId,
        start: usize,
        size: usize,
        lhs_index: usize,
        rhs_index: usize,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if lhs_index == rhs_index || size == 0 {
            return Ok(());
        }
        let object = self
            .lookup_object(objects, object_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        let mut all_bytes = self.serialize_stored_value(&object.ty, &object.value, span)?;
        let lhs_start = start + lhs_index * size;
        let rhs_start = start + rhs_index * size;
        let lhs_bytes = all_bytes[lhs_start..lhs_start + size].to_vec();
        let rhs_bytes = all_bytes[rhs_start..rhs_start + size].to_vec();
        self.overlay_bytes(&mut all_bytes, lhs_start, &rhs_bytes);
        self.overlay_bytes(&mut all_bytes, rhs_start, &lhs_bytes);
        let new_value = self.deserialize_stored_value(&object.ty, &all_bytes, span)?;
        let initialized = self.stored_value_is_determinate(&new_value);
        let object = self.lookup_object_mut(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        object.value = new_value;
        object.initialized = initialized;
        object.indeterminate_reason = None;
        object.modification_count = object.modification_count.saturating_add(1);
        Ok(())
    }

    pub(super) fn eval_qsort_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        _frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let base_pointer = evaluated[0].as_pointer(args[0].span())?;
        let (nmemb, size, total) = self.check_sort_search_common(
            "qsort",
            &evaluated[1],
            &evaluated[2],
            args[1].span(),
            args[2].span(),
        )?;
        let order = self.pending_qsort_order.take().ok_or_else(|| {
            Diagnostic::error("qsort comparison precondition plan is unavailable", span)
        })?;
        if nmemb < 2 || size == 0 || total == 0 {
            return Ok(TypedValue::void());
        }
        let (object_id, start, _) =
            self.byte_region_from_pointer(&base_pointer, total, args[0].span(), objects)?;
        let mut current = (0..nmemb).collect::<Vec<_>>();
        for (target, &desired) in order.iter().enumerate() {
            let current_position = current
                .iter()
                .position(|&original| original == desired)
                .expect("qsort plan contains every element");
            self.swap_qsort_elements(
                object_id,
                start,
                size,
                target,
                current_position,
                span,
                objects,
            )?;
            current.swap(target, current_position);
        }
        Ok(TypedValue::void())
    }

    pub(super) fn eval_bsearch_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        _frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let base_pointer = evaluated[1].as_pointer(args[1].span())?;
        let (nmemb, size, total) = self.check_sort_search_common(
            "bsearch",
            &evaluated[2],
            &evaluated[3],
            args[2].span(),
            args[3].span(),
        )?;
        let return_ty = self.host_function_return_type("bsearch", span.file, span)?;
        let found = self.pending_bsearch_result.take().ok_or_else(|| {
            Diagnostic::error(
                "bsearch comparison precondition result is unavailable",
                span,
            )
        })?;
        if nmemb == 0 || size == 0 || total == 0 {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        if let Some(index) = found {
            let element_pointer =
                self.pointer_with_byte_offset(&base_pointer, index * size, span, objects)?;
            return Ok(self.pointer_value_with_type(return_ty, element_pointer));
        }
        Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()))
    }

    pub(super) fn eval_mblen_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "mblen byte count",
        )?;
        let source = evaluated[0].as_pointer(args[0].span())?;
        let result = if source.is_null() {
            unsafe { mblen(std::ptr::null(), 0) }
        } else {
            let buffer = self.read_multibyte_source_buffer(&source, n, args[0].span(), objects)?;
            unsafe { mblen(buffer.as_ptr().cast::<c_char>(), n) }
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_mbsinit_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let state_ptr = evaluated[0].as_pointer(args[0].span())?;
        if !state_ptr.is_null()
            && (self.mbrtoc16_pending.contains_key(&state_ptr)
                || self.c16rtomb_pending.contains_key(&state_ptr))
        {
            return Ok(TypedValue::int(0));
        }
        let result = if state_ptr.is_null() {
            unsafe { mbsinit(std::ptr::null()) }
        } else {
            let state = self.read_mbstate_value(&state_ptr, args[0].span(), objects)?;
            unsafe { mbsinit(&state) }
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_mbrlen_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source = evaluated[0].as_pointer(args[0].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "mbrlen byte count",
        )?;
        let state_ptr = evaluated[2].as_pointer(args[2].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[2].span(), objects)?
        };
        let result = if source.is_null() {
            unsafe {
                mbrlen(
                    std::ptr::null(),
                    0,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        } else {
            let buffer = self.read_multibyte_source_buffer(&source, n, args[0].span(), objects)?;
            unsafe {
                mbrlen(
                    buffer.as_ptr().cast::<c_char>(),
                    n,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[2].span(), objects)?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_mbrtowc_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out_ptr = evaluated[0].as_pointer(args[0].span())?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbrtowc byte count",
        )?;
        let state_ptr = evaluated[3].as_pointer(args[3].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[3].span(), objects)?
        };
        let mut out = 0 as libc::wchar_t;
        let result = if source.is_null() {
            unsafe {
                mbrtowc(
                    if out_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut out
                    },
                    std::ptr::null(),
                    0,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        } else {
            let buffer = self.read_multibyte_source_buffer(&source, n, args[1].span(), objects)?;
            unsafe {
                mbrtowc(
                    if out_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut out
                    },
                    buffer.as_ptr().cast::<c_char>(),
                    n,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[3].span(), objects)?;
        }
        if result != usize::MAX && !out_ptr.is_null() && !source.is_null() {
            self.write_wide_units_to_pointer(&out_ptr, &[out], args[0].span(), objects)?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_mbtowc_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out_pointer = evaluated[0].as_pointer(args[0].span())?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbtowc byte count",
        )?;
        let mut out = 0 as libc::wchar_t;
        let result = if source.is_null() {
            unsafe { mbtowc(std::ptr::null_mut(), std::ptr::null(), 0) }
        } else {
            let buffer = self.read_multibyte_source_buffer(&source, n, args[1].span(), objects)?;
            let target = if out_pointer.is_null() {
                std::ptr::null_mut()
            } else {
                &mut out
            };
            unsafe { mbtowc(target, buffer.as_ptr().cast::<c_char>(), n) }
        };
        if result >= 0 && !out_pointer.is_null() && !source.is_null() {
            self.write_wide_units_to_pointer(&out_pointer, &[out], args[0].span(), objects)?;
        }
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_wctomb_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let wide = self.wchar_scalar_to_host(&evaluated[1], args[1].span(), "wctomb")?;
        let result = if dest.is_null() {
            unsafe { wctomb(std::ptr::null_mut(), wide) }
        } else {
            let mut bytes = vec![0u8; host_mb_cur_max().max(1)];
            let result = unsafe { wctomb(bytes.as_mut_ptr().cast::<c_char>(), wide) };
            if result >= 0 {
                let written = result as usize;
                if written != 0 {
                    let (object_id, start, _) =
                        self.byte_region_from_pointer(&dest, written, args[0].span(), objects)?;
                    self.ensure_writable_object(object_id, args[0].span(), objects)?;
                    self.overlay_known_bytes_into_object(
                        object_id,
                        start,
                        &bytes[..written],
                        args[0].span(),
                        objects,
                    )?;
                }
            }
            result
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_wcrtomb_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let wide = self.wchar_scalar_to_host(&evaluated[1], args[1].span(), "wcrtomb")?;
        let state_ptr = evaluated[2].as_pointer(args[2].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[2].span(), objects)?
        };
        let result = if dest.is_null() {
            unsafe {
                wcrtomb(
                    std::ptr::null_mut(),
                    wide,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        } else {
            let mut bytes = vec![0u8; host_mb_cur_max().max(1)];
            let result = unsafe {
                wcrtomb(
                    bytes.as_mut_ptr().cast::<c_char>(),
                    wide,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            };
            if result != usize::MAX {
                let written = result as usize;
                if written != 0 {
                    let (object_id, start, _) =
                        self.byte_region_from_pointer(&dest, written, args[0].span(), objects)?;
                    self.ensure_writable_object(object_id, args[0].span(), objects)?;
                    self.overlay_known_bytes_into_object(
                        object_id,
                        start,
                        &bytes[..written],
                        args[0].span(),
                        objects,
                    )?;
                }
            }
            result
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[2].span(), objects)?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_mbrtoc_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let out_ptr = evaluated[0].as_pointer(args[0].span())?;
        let source = evaluated[1].as_pointer(args[1].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "multibyte byte count",
        )?;
        let state_ptr = evaluated[3].as_pointer(args[3].span())?;
        let pending_low = if function_name == "mbrtoc16" {
            if state_ptr.is_null() {
                self.mbrtoc16_null_pending.take()
            } else {
                self.mbrtoc16_pending.remove(&state_ptr)
            }
        } else {
            None
        };
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[3].span(), objects)?
        };
        let buffer = if source.is_null() {
            Vec::new()
        } else {
            self.read_multibyte_source_buffer(&source, n, args[1].span(), objects)?
        };
        let source_ptr = if source.is_null() {
            std::ptr::null()
        } else {
            buffer.as_ptr().cast::<c_char>()
        };
        let (result, unit) = if let Some(low) = pending_low {
            (usize::MAX - 2, low as i128)
        } else {
            let mut wide = 0 as libc::wchar_t;
            let result = unsafe {
                mbrtowc(
                    &mut wide,
                    source_ptr,
                    if source.is_null() { 0 } else { n },
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            };
            let scalar = wide as u32;
            if function_name == "mbrtoc16"
                && result != usize::MAX
                && result != usize::MAX - 1
                && scalar > 0xffff
            {
                let scalar = scalar - 0x10000;
                let high = 0xd800 | ((scalar >> 10) as u16);
                let low = 0xdc00 | ((scalar & 0x3ff) as u16);
                if state_ptr.is_null() {
                    self.mbrtoc16_null_pending = Some(low);
                } else {
                    self.mbrtoc16_pending.insert(state_ptr.clone(), low);
                }
                (result, high as i128)
            } else {
                (result, scalar as i128)
            }
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[3].span(), objects)?;
        }
        if !out_ptr.is_null() && result != usize::MAX && result != usize::MAX - 1 {
            let output_ty = evaluated[0]
                .ty
                .element_type()
                .cloned()
                .expect("validated output pointer");
            let lvalue = self.pointer_lvalue(
                function_name,
                &out_ptr,
                &output_ty,
                args[0].span(),
                "7.28.1",
            )?;
            self.store_lvalue(
                objects,
                &lvalue,
                TypedValue::integer(output_ty, unit),
                args[0].span(),
            )?;
        }
        let return_ty = self.host_function_return_type(function_name, span.file, span)?;
        Ok(TypedValue::integer(return_ty, result as i128))
    }

    pub(super) fn eval_c_rtomb_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let state_ptr = evaluated[2].as_pointer(args[2].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[2].span(), objects)?
        };
        let mut bytes = vec![0u8; host_mb_cur_max().max(1)];
        let dest_ptr = if dest.is_null() {
            std::ptr::null_mut()
        } else {
            bytes.as_mut_ptr().cast::<c_char>()
        };
        let mut scalar = evaluated[1].to_int()? as u32;
        if function_name == "c16rtomb" {
            let unit = scalar as u16;
            let pending_high = if state_ptr.is_null() {
                self.c16rtomb_null_pending
            } else {
                self.c16rtomb_pending.get(&state_ptr).copied()
            };
            if (0xd800..=0xdbff).contains(&unit) {
                if state_ptr.is_null() {
                    self.c16rtomb_null_pending = Some(unit);
                } else {
                    self.c16rtomb_pending.insert(state_ptr.clone(), unit);
                }
                return Ok(TypedValue::integer(
                    self.host_function_return_type(function_name, span.file, span)?,
                    0,
                ));
            }
            if let Some(high) = pending_high {
                if !(0xdc00..=0xdfff).contains(&unit) {
                    Self::set_host_errno(libc::EILSEQ);
                    return Ok(TypedValue::integer(
                        self.host_function_return_type(function_name, span.file, span)?,
                        usize::MAX as i128,
                    ));
                }
                if state_ptr.is_null() {
                    self.c16rtomb_null_pending = None;
                } else {
                    self.c16rtomb_pending.remove(&state_ptr);
                }
                scalar = 0x10000 + (((high as u32 - 0xd800) << 10) | (unit as u32 - 0xdc00));
            } else if (0xdc00..=0xdfff).contains(&unit) {
                Self::set_host_errno(libc::EILSEQ);
                return Ok(TypedValue::integer(
                    self.host_function_return_type(function_name, span.file, span)?,
                    usize::MAX as i128,
                ));
            }
        }
        let result = if scalar > 0x10ffff || (0xd800..=0xdfff).contains(&scalar) {
            Self::set_host_errno(libc::EILSEQ);
            usize::MAX
        } else {
            unsafe {
                wcrtomb(
                    dest_ptr,
                    scalar as libc::wchar_t,
                    if state_ptr.is_null() {
                        std::ptr::null_mut()
                    } else {
                        &mut state
                    },
                )
            }
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[2].span(), objects)?;
        }
        if !dest.is_null() && result != usize::MAX && result != 0 {
            let (object, start, _) =
                self.byte_region_from_pointer(&dest, result, args[0].span(), objects)?;
            self.overlay_known_bytes_into_object(
                object,
                start,
                &bytes[..result],
                args[0].span(),
                objects,
            )?;
        }
        let return_ty = self.host_function_return_type(function_name, span.file, span)?;
        Ok(TypedValue::integer(return_ty, result as i128))
    }

    pub(super) fn eval_mbstowcs_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbstowcs element count",
        )?;
        let source_bytes = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let source = CString::new(source_bytes).unwrap();
        let mut temp = vec![
            0 as libc::wchar_t;
            if dest.is_null() {
                source.as_bytes().len() + 1
            } else {
                n.max(1)
            }
        ];
        let result = unsafe {
            mbstowcs(
                temp.as_mut_ptr(),
                source.as_ptr(),
                if dest.is_null() { temp.len() } else { n },
            )
        };
        if result != usize::MAX && !dest.is_null() && n != 0 {
            let written = if result < n { result + 1 } else { n };
            self.write_wide_units_to_pointer(&dest, &temp[..written], args[0].span(), objects)?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_wcstombs_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let n = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "wcstombs byte count",
        )?;
        let mut source_units = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        source_units.push(0);
        let mut temp = vec![
            0u8;
            if dest.is_null() {
                source_units.len().saturating_mul(host_mb_cur_max().max(1))
            } else {
                n.max(1)
            }
        ];
        let result = unsafe {
            wcstombs(
                temp.as_mut_ptr().cast::<c_char>(),
                source_units.as_ptr(),
                if dest.is_null() { temp.len() } else { n },
            )
        };
        if result != usize::MAX && !dest.is_null() && n != 0 {
            let written = if result < n { result + 1 } else { n };
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest, written, args[0].span(), objects)?;
            self.ensure_writable_object(object_id, args[0].span(), objects)?;
            self.overlay_known_bytes_into_object(
                object_id,
                start,
                &temp[..written],
                args[0].span(),
                objects,
            )?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_wcsto_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let source_pointer = evaluated[0].as_pointer(args[0].span())?;
        let input = self.wide_c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let mut endptr: *mut libc::wchar_t = std::ptr::null_mut();
        let result_ty = self.host_function_return_type(function_name, span.file, span)?;
        let result = match function_name {
            "wcstof" => {
                TypedValue::floating(result_ty, unsafe { wcstof(input.as_ptr(), &mut endptr) }
                    as f64)
            }
            "wcstod" => {
                TypedValue::floating(result_ty, unsafe { wcstod(input.as_ptr(), &mut endptr) })
            }
            "wcstold" => {
                TypedValue::floating(result_ty, unsafe { wcstold(input.as_ptr(), &mut endptr) }
                    as f64)
            }
            "wcstol" => TypedValue::integer(result_ty.clone(), unsafe {
                wcstol(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
            } as i128),
            "wcstoll" => TypedValue::integer(result_ty.clone(), unsafe {
                wcstoll(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
            } as i128),
            "wcstoul" => TypedValue::integer(result_ty.clone(), unsafe {
                wcstoul(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
            } as i128),
            "wcstoull" => TypedValue::integer(result_ty.clone(), unsafe {
                wcstoull(input.as_ptr(), &mut endptr, evaluated[2].to_int()? as c_int)
            } as i128),
            _ => unreachable!("checked by caller"),
        };
        let offset = if endptr.is_null() {
            0
        } else {
            unsafe { endptr.offset_from(input.as_ptr()) as usize }
        };
        self.write_wcsto_end_pointer(
            function_name,
            &evaluated[1],
            args[1].span(),
            &source_pointer,
            offset,
            objects,
        )?;
        Ok(result)
    }

    pub(super) fn eval_mbsrtowcs_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let srcpp_ty = self.host_function_parameter_type("mbsrtowcs", span.file, 1, span)?;
        let srcpp = evaluated[1].as_pointer(args[1].span())?;
        let src_lvalue = self.pointer_lvalue(
            "mbsrtowcs",
            &srcpp,
            srcpp_ty.element_type().expect("pointer parameter"),
            args[1].span(),
            "7.24.6.4.1",
        )?;
        let current = self.load_lvalue(src_lvalue.clone(), args[1].span(), objects)?;
        let current_ptr = current.as_pointer(args[1].span())?;
        let len = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "mbsrtowcs element count",
        )?;
        let state_ptr = evaluated[3].as_pointer(args[3].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[3].span(), objects)?
        };
        let source_bytes = if current_ptr.is_null() {
            vec![0]
        } else {
            let mut bytes =
                self.read_c_string_bytes(current_ptr.clone(), args[1].span(), objects)?;
            bytes.push(0);
            bytes
        };
        let mut host_src: *const c_char = if current_ptr.is_null() {
            std::ptr::null()
        } else {
            source_bytes.as_ptr().cast::<c_char>()
        };
        let mut temp = vec![0 as libc::wchar_t; len.max(1)];
        let result = unsafe {
            mbsrtowcs(
                if dest.is_null() {
                    std::ptr::null_mut()
                } else {
                    temp.as_mut_ptr()
                },
                &mut host_src,
                len,
                if state_ptr.is_null() {
                    std::ptr::null_mut()
                } else {
                    &mut state
                },
            )
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[3].span(), objects)?;
        }
        if !dest.is_null() && result != usize::MAX && len != 0 {
            let written = if host_src.is_null() && result < len {
                result + 1
            } else {
                result.min(len)
            };
            if written != 0 {
                self.write_wide_units_to_pointer(&dest, &temp[..written], args[0].span(), objects)?;
            }
        }
        let new_source = if current_ptr.is_null() || host_src.is_null() {
            Self::null_pointer()
        } else {
            let offset =
                unsafe { host_src.offset_from(source_bytes.as_ptr().cast::<c_char>()) as usize };
            self.pointer_with_byte_offset(&current_ptr, offset, args[1].span(), objects)?
        };
        if !dest.is_null() {
            self.store_lvalue(
                objects,
                &src_lvalue,
                self.pointer_value_with_type(
                    srcpp_ty.element_type().expect("pointer parameter").clone(),
                    new_source,
                ),
                args[1].span(),
            )?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_wcsrtombs_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].as_pointer(args[0].span())?;
        let srcpp_ty = self.host_function_parameter_type("wcsrtombs", span.file, 1, span)?;
        let srcpp = evaluated[1].as_pointer(args[1].span())?;
        let src_lvalue = self.pointer_lvalue(
            "wcsrtombs",
            &srcpp,
            srcpp_ty.element_type().expect("pointer parameter"),
            args[1].span(),
            "7.24.6.4.2",
        )?;
        let current = self.load_lvalue(src_lvalue.clone(), args[1].span(), objects)?;
        let current_ptr = current.as_pointer(args[1].span())?;
        let len = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "wcsrtombs byte count",
        )?;
        let state_ptr = evaluated[3].as_pointer(args[3].span())?;
        let mut state = if state_ptr.is_null() {
            HostMbState { opaque: [0; 128] }
        } else {
            self.read_mbstate_value(&state_ptr, args[3].span(), objects)?
        };
        let source_units = if current_ptr.is_null() {
            vec![0]
        } else {
            self.wide_c_string_argument(&current, args[1].span(), objects)?
        };
        let mut host_src: *const libc::wchar_t = if current_ptr.is_null() {
            std::ptr::null()
        } else {
            source_units.as_ptr()
        };
        let mut temp = vec![0u8; len.max(1)];
        let result = unsafe {
            wcsrtombs(
                if dest.is_null() {
                    std::ptr::null_mut()
                } else {
                    temp.as_mut_ptr().cast::<c_char>()
                },
                &mut host_src,
                len,
                if state_ptr.is_null() {
                    std::ptr::null_mut()
                } else {
                    &mut state
                },
            )
        };
        if !state_ptr.is_null() {
            self.write_mbstate_value(&state_ptr, &state, args[3].span(), objects)?;
        }
        if !dest.is_null() && result != usize::MAX && len != 0 {
            let written = if host_src.is_null() && result < len {
                result + 1
            } else {
                result.min(len)
            };
            if written != 0 {
                let (object_id, start, _) =
                    self.byte_region_from_pointer(&dest, written, args[0].span(), objects)?;
                self.ensure_writable_object(object_id, args[0].span(), objects)?;
                self.overlay_known_bytes_into_object(
                    object_id,
                    start,
                    &temp[..written],
                    args[0].span(),
                    objects,
                )?;
            }
        }
        let new_source = if current_ptr.is_null() || host_src.is_null() {
            Self::null_pointer()
        } else {
            let offset_units = unsafe { host_src.offset_from(source_units.as_ptr()) as usize };
            self.pointer_with_byte_offset(
                &current_ptr,
                offset_units
                    .checked_mul(self.wide_char_byte_width())
                    .ok_or_else(|| {
                        Diagnostic::ub(
                            "wcsrtombs source offset is out of supported range",
                            args[1].span(),
                            Some("7.24.6.4.2"),
                        )
                    })?,
                args[1].span(),
                objects,
            )?
        };
        if !dest.is_null() {
            self.store_lvalue(
                objects,
                &src_lvalue,
                self.pointer_value_with_type(
                    srcpp_ty.element_type().expect("pointer parameter").clone(),
                    new_source,
                ),
                args[1].span(),
            )?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_wcscpy_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let dest_ptr = dest.as_pointer(args[0].span())?;
        let mut units = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        units.push(0);
        self.write_wide_units_to_pointer(&dest_ptr, &units, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wcsncpy_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsncpy count")?;
        let dest_ptr = dest.as_pointer(args[0].span())?;
        let src_ptr = evaluated[1].as_pointer(args[1].span())?;
        let src_buffer = self.read_bounded_wide_source(src_ptr, count, args[1].span(), objects)?;
        let mut copied = vec![0 as libc::wchar_t; count];
        if count != 0 {
            let copy_len = src_buffer
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(count.min(src_buffer.len()));
            copied[..copy_len].copy_from_slice(&src_buffer[..copy_len]);
        }
        self.write_wide_units_to_pointer(&dest_ptr, &copied, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wide_memory_copy_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let count = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            &format!("{function_name} count"),
        )?;
        let dest_ptr = dest.as_pointer(args[0].span())?;
        let units = self.read_exact_wide_units(
            evaluated[1].as_pointer(args[1].span())?,
            count,
            args[1].span(),
            objects,
        )?;
        self.write_wide_units_to_pointer(&dest_ptr, &units, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wcscat_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let dest_ptr = dest.as_pointer(args[0].span())?;
        let mut dest_units =
            self.read_wide_string_units(dest_ptr.clone(), args[0].span(), objects)?;
        let src_units = self.read_wide_string_units(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        dest_units.extend(src_units);
        dest_units.push(0);
        self.write_wide_units_to_pointer(&dest_ptr, &dest_units, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wcsncat_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsncat count")?;
        let dest_ptr = dest.as_pointer(args[0].span())?;
        let mut dest_units =
            self.read_wide_string_units(dest_ptr.clone(), args[0].span(), objects)?;
        let src_units = self.read_bounded_wide_source(
            evaluated[1].as_pointer(args[1].span())?,
            count,
            args[1].span(),
            objects,
        )?;
        let copy_len = src_units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(count);
        dest_units.extend_from_slice(&src_units[..copy_len]);
        dest_units.push(0);
        self.write_wide_units_to_pointer(&dest_ptr, &dest_units, args[0].span(), objects)?;
        Ok(dest)
    }

    pub(super) fn eval_wcscoll_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = self.wide_c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let rhs = self.wide_c_string_argument(&evaluated[1], args[1].span(), objects)?;
        let cmp = unsafe { wcscoll(lhs.as_ptr(), rhs.as_ptr()) };
        Ok(TypedValue::integer(
            self.host_function_return_type("wcscoll", span.file, span)?,
            cmp as i128,
        ))
    }

    pub(super) fn eval_wcsxfrm_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let count =
            self.checked_usize_from_unsigned_long(&evaluated[2], args[2].span(), "wcsxfrm count")?;
        let dest_ptr = evaluated[0].as_pointer(args[0].span())?;
        let src = self.wide_c_string_argument(&evaluated[1], args[1].span(), objects)?;
        let mut transformed = vec![0 as libc::wchar_t; count.max(1)];
        let result = unsafe {
            wcsxfrm(
                if count == 0 {
                    std::ptr::null_mut()
                } else {
                    transformed.as_mut_ptr()
                },
                src.as_ptr(),
                count,
            )
        };
        if count != 0 {
            let written = if result < count { result + 1 } else { count };
            if written != 0 {
                self.write_wide_units_to_pointer(
                    &dest_ptr,
                    &transformed[..written],
                    args[0].span(),
                    objects,
                )?;
            }
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_getenv_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if let Some(previous) = self.getenv_binding.take() {
            self.end_live_retired_object_lifetime(previous);
        }
        let name = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
        let result_ptr = unsafe { libc::getenv(name.as_ptr()) };
        let return_ty = self.host_function_return_type("getenv", span.file, span)?;
        if result_ptr.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let pointer =
            self.intern_readonly_c_bytes(unsafe { CStr::from_ptr(result_ptr) }.to_bytes());
        self.getenv_binding = pointer.object;
        Ok(self.pointer_value_with_type(return_ty, pointer))
    }

    pub(super) fn eval_system_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let result = if evaluated[0].as_pointer(args[0].span())?.is_null() {
            host_system(std::ptr::null())
        } else {
            let command = self.c_string_argument(&evaluated[0], args[0].span(), objects)?;
            host_system(command.as_ptr())
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_setlocale_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let category = evaluated[0].to_int()? as c_int;
        let locale_is_null = evaluated[1].as_pointer(args[1].span())?.is_null();
        let tracks_ctype = !locale_is_null && matches!(category, 0 | 2);
        let result_ptr = if locale_is_null {
            unsafe { setlocale(category, std::ptr::null()) }
        } else {
            let locale = self.c_string_argument(&evaluated[1], args[1].span(), objects)?;
            unsafe { setlocale(category, locale.as_ptr()) }
        };
        let return_ty = self.host_function_return_type("setlocale", span.file, span)?;
        if result_ptr.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let result_bytes = unsafe { CStr::from_ptr(result_ptr) }.to_bytes().to_vec();
        if tracks_ctype {
            self.locale_generation = self.locale_generation.saturating_add(1);
            self.wctrans_descriptors.clear();
            self.wctype_descriptors.clear();
        }
        let pointer = self.intern_readonly_c_bytes(&result_bytes);
        Ok(self.pointer_value_with_type(return_ty, pointer))
    }

    pub(super) fn eval_localeconv_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let return_ty = self.host_function_return_type("localeconv", span.file, span)?;
        let struct_ty = return_ty.element_type().cloned().ok_or_else(|| {
            Diagnostic::error("localeconv does not return a pointer to struct lconv", span)
        })?;
        let record = self
            .record_type(&struct_ty)
            .cloned()
            .ok_or_else(|| Diagnostic::error("localeconv return type is incomplete", span))?;
        let host = unsafe { localeconv() };
        if host.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let host = unsafe { &*host };
        let mut pinned_ids = Vec::new();
        let object = self.allocate_object(
            objects,
            struct_ty.clone(),
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        let mut members = Vec::with_capacity(record.members.len());
        for member in &record.members {
            let value = match member.storage_name.as_str() {
                "decimal_point" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.decimal_point,
                    &mut pinned_ids,
                ),
                "thousands_sep" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.thousands_sep,
                    &mut pinned_ids,
                ),
                "grouping" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.grouping,
                    &mut pinned_ids,
                ),
                "int_curr_symbol" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.int_curr_symbol,
                    &mut pinned_ids,
                ),
                "currency_symbol" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.currency_symbol,
                    &mut pinned_ids,
                ),
                "mon_decimal_point" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.mon_decimal_point,
                    &mut pinned_ids,
                ),
                "mon_thousands_sep" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.mon_thousands_sep,
                    &mut pinned_ids,
                ),
                "mon_grouping" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.mon_grouping,
                    &mut pinned_ids,
                ),
                "positive_sign" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.positive_sign,
                    &mut pinned_ids,
                ),
                "negative_sign" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.negative_sign,
                    &mut pinned_ids,
                ),
                "int_frac_digits" => {
                    TypedValue::integer(member.ty.clone(), host.int_frac_digits as i128)
                }
                "frac_digits" => TypedValue::integer(member.ty.clone(), host.frac_digits as i128),
                "p_cs_precedes" => {
                    TypedValue::integer(member.ty.clone(), host.p_cs_precedes as i128)
                }
                "p_sep_by_space" => {
                    TypedValue::integer(member.ty.clone(), host.p_sep_by_space as i128)
                }
                "n_cs_precedes" => {
                    TypedValue::integer(member.ty.clone(), host.n_cs_precedes as i128)
                }
                "n_sep_by_space" => {
                    TypedValue::integer(member.ty.clone(), host.n_sep_by_space as i128)
                }
                "p_sign_posn" => TypedValue::integer(member.ty.clone(), host.p_sign_posn as i128),
                "n_sign_posn" => TypedValue::integer(member.ty.clone(), host.n_sign_posn as i128),
                "int_p_cs_precedes" => {
                    TypedValue::integer(member.ty.clone(), host.int_p_cs_precedes as i128)
                }
                "int_p_sep_by_space" => {
                    TypedValue::integer(member.ty.clone(), host.int_p_sep_by_space as i128)
                }
                "int_n_cs_precedes" => {
                    TypedValue::integer(member.ty.clone(), host.int_n_cs_precedes as i128)
                }
                "int_n_sep_by_space" => {
                    TypedValue::integer(member.ty.clone(), host.int_n_sep_by_space as i128)
                }
                "int_p_sign_posn" => {
                    TypedValue::integer(member.ty.clone(), host.int_p_sign_posn as i128)
                }
                "int_n_sign_posn" => {
                    TypedValue::integer(member.ty.clone(), host.int_n_sign_posn as i128)
                }
                _ => self.zero_value(&member.ty),
            };
            members.push((
                member.storage_name.as_str().into(),
                self.stored_value_from_typed_value(value, &member.ty, span)?,
            ));
        }
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.address_taken = true;
            state.readonly = true;
            state.value = StoredValue::Record(members);
        }
        pinned_ids.insert(0, object);
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(object)))
    }

    pub(super) fn eval_signal_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let signal_number = evaluated[0].to_int()? as i32;
        let handler = match &evaluated[1].data {
            ValueData::Pointer(pointer) if pointer.is_null() => SignalHandlerState::Default,
            ValueData::Pointer(pointer)
                if Self::opaque_integer_pointer_address(pointer)
                    .is_some_and(|value| value == 1) =>
            {
                SignalHandlerState::Ignore
            }
            ValueData::Function(name) if name.as_ref() == "__codex_sig_dfl" => {
                SignalHandlerState::Default
            }
            ValueData::Function(name) if name.as_ref() == "__codex_sig_ign" => {
                SignalHandlerState::Ignore
            }
            ValueData::Function(name) => SignalHandlerState::Function(name.to_string()),
            _ => {
                return Err(Diagnostic::error(
                    "signal requires a compatible signal handler pointer",
                    span,
                ));
            }
        };
        let previous = self
            .signal_handlers
            .insert(signal_number, handler)
            .unwrap_or(SignalHandlerState::Default);
        let return_ty = self.host_function_return_type("signal", span.file, span)?;
        match previous {
            SignalHandlerState::Default => {
                Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()))
            }
            SignalHandlerState::Ignore => self.convert_value(TypedValue::int(1), &return_ty, span),
            SignalHandlerState::Function(name) => Ok(TypedValue::function(return_ty, name)),
        }
    }

    pub(super) fn eval_raise_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let signal_number = evaluated[0].to_int()? as i32;
        match self
            .signal_handlers
            .get(&signal_number)
            .cloned()
            .unwrap_or(SignalHandlerState::Default)
        {
            SignalHandlerState::Default | SignalHandlerState::Ignore => {}
            SignalHandlerState::Function(symbol) => {
                let function = self
                    .lookup_function_symbol(&symbol)
                    .cloned()
                    .ok_or_else(|| {
                        Diagnostic::error("signal handler does not name a supported function", span)
                    })?;
                let _ = self.call_function(
                    &function,
                    self.function_may_setjmp_symbol(&symbol),
                    vec![TypedValue::int(signal_number as i128)],
                    span,
                    objects,
                )?;
            }
        }
        Ok(TypedValue::int(0))
    }

    pub(super) fn tm_type_for_call(&self, span: Span) -> Result<CType, Diagnostic> {
        self.host_function_return_type("gmtime", span.file, span)?
            .element_type()
            .cloned()
            .ok_or_else(|| Diagnostic::error("struct tm type is unavailable", span))
    }

    fn host_tm_from_typed_value(
        &self,
        value: &TypedValue,
        span: Span,
        ignore_output_fields: bool,
    ) -> Result<HostTm, Diagnostic> {
        let ValueData::Aggregate(stored) = &value.data else {
            return Err(Diagnostic::error("expected struct tm value", span));
        };
        let StoredValue::Record(members) = stored.as_ref() else {
            return Err(Diagnostic::error("expected struct tm record storage", span));
        };
        let field = |name: &str| {
            members
                .iter()
                .find(|(member_name, _)| member_name.as_ref() == name)
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    Diagnostic::error(format!("struct tm is missing field {name}"), span)
                })
        };
        let int_field = |name: &str| -> Result<c_int, Diagnostic> {
            match field(name)? {
                StoredValue::Scalar(value) => Ok(value.to_int()? as c_int),
                StoredValue::Indeterminate => Err(Diagnostic::ub(
                    "time structure argument contains an indeterminate value",
                    span,
                    Some("7.23"),
                )),
                _ => Err(Diagnostic::error(
                    format!("struct tm field {name} is not scalar"),
                    span,
                )),
            }
        };
        let long_field = |name: &str| -> Result<c_long, Diagnostic> {
            match field(name)? {
                StoredValue::Scalar(value) => Ok(value.to_int()? as c_long),
                StoredValue::Indeterminate => Err(Diagnostic::ub(
                    "time structure argument contains an indeterminate value",
                    span,
                    Some("7.23"),
                )),
                _ => Err(Diagnostic::error(
                    format!("struct tm field {name} is not scalar"),
                    span,
                )),
            }
        };
        Ok(HostTm {
            tm_sec: int_field("tm_sec")?,
            tm_min: int_field("tm_min")?,
            tm_hour: int_field("tm_hour")?,
            tm_mday: int_field("tm_mday")?,
            tm_mon: int_field("tm_mon")?,
            tm_year: int_field("tm_year")?,
            tm_wday: if ignore_output_fields {
                0
            } else {
                int_field("tm_wday")?
            },
            tm_yday: if ignore_output_fields {
                0
            } else {
                int_field("tm_yday")?
            },
            tm_isdst: int_field("tm_isdst")?,
            tm_gmtoff: if ignore_output_fields {
                0
            } else {
                long_field("tm_gmtoff")?
            },
            tm_zone: std::ptr::null(),
        })
    }

    pub(super) fn load_host_tm_from_pointer(
        &mut self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<HostTm, Diagnostic> {
        let tm_ty = self.tm_type_for_call(span)?;
        let pointer = value.as_pointer(span)?;
        let lvalue = self.pointer_lvalue(function_name, &pointer, &tm_ty, span, "7.23")?;
        let loaded = self.load_lvalue(lvalue, span, objects)?;
        self.host_tm_from_typed_value(&loaded, span, false)
    }

    fn load_host_tm_for_mktime(
        &mut self,
        value: &TypedValue,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<HostTm, Diagnostic> {
        let tm_ty = self.tm_type_for_call(span)?;
        let pointer = value.as_pointer(span)?;
        let lvalue = self.pointer_lvalue("mktime", &pointer, &tm_ty, span, "7.23.2.3")?;
        let loaded = self.load_lvalue(lvalue, span, objects)?;
        self.host_tm_from_typed_value(&loaded, span, true)
    }

    fn stored_tm_value_from_host(
        &mut self,
        tm_ty: &CType,
        host: HostTm,
        span: Span,
        pinned_ids: &mut Vec<ObjectId>,
    ) -> Result<StoredValue, Diagnostic> {
        let record = self
            .record_type(tm_ty)
            .cloned()
            .ok_or_else(|| Diagnostic::error("struct tm type is incomplete", span))?;
        let mut members = Vec::with_capacity(record.members.len());
        for member in &record.members {
            let value = match member.storage_name.as_str() {
                "tm_sec" => TypedValue::integer(member.ty.clone(), host.tm_sec as i128),
                "tm_min" => TypedValue::integer(member.ty.clone(), host.tm_min as i128),
                "tm_hour" => TypedValue::integer(member.ty.clone(), host.tm_hour as i128),
                "tm_mday" => TypedValue::integer(member.ty.clone(), host.tm_mday as i128),
                "tm_mon" => TypedValue::integer(member.ty.clone(), host.tm_mon as i128),
                "tm_year" => TypedValue::integer(member.ty.clone(), host.tm_year as i128),
                "tm_wday" => TypedValue::integer(member.ty.clone(), host.tm_wday as i128),
                "tm_yday" => TypedValue::integer(member.ty.clone(), host.tm_yday as i128),
                "tm_isdst" => TypedValue::integer(member.ty.clone(), host.tm_isdst as i128),
                "tm_gmtoff" => TypedValue::integer(member.ty.clone(), host.tm_gmtoff as i128),
                "tm_zone" => self.intern_host_c_string_pointer(
                    member.ty.clone(),
                    host.tm_zone.cast_mut(),
                    pinned_ids,
                ),
                _ => self.zero_value(&member.ty),
            };
            members.push((
                member.storage_name.as_str().into(),
                self.stored_value_from_typed_value(value, &member.ty, span)?,
            ));
        }
        Ok(StoredValue::Record(members))
    }

    pub(super) fn eval_mktime_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let mut host_tm = self.load_host_tm_for_mktime(&evaluated[0], args[0].span(), objects)?;
        let result = calendar::make_local_time(&mut host_tm);
        let tm_ty = self.tm_type_for_call(span)?;
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let lvalue = self.pointer_lvalue("mktime", &pointer, &tm_ty, args[0].span(), "7.23.2.3")?;
        let mut pinned = Vec::new();
        let stored =
            self.stored_tm_value_from_host(&tm_ty, host_tm, args[0].span(), &mut pinned)?;
        let value = TypedValue::aggregate(tm_ty, stored);
        self.store_lvalue(objects, &lvalue, value, args[0].span())?;
        Ok(TypedValue::integer(CType::Long, result as i128))
    }

    pub(super) fn eval_time_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        // WASI libc's time() stub traps, even though its realtime clock is available. SystemTime
        // uses that same clock on Wasm and also avoids leaking the target's 32-bit C long ABI into
        // the interpreter's deliberately modeled 64-bit time_t.
        let result = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_secs()).ok())
            .map(i128::from)
            .unwrap_or(-1);
        if !evaluated[0].as_pointer(args[0].span())?.is_null() {
            let pointer = evaluated[0].as_pointer(args[0].span())?;
            let lvalue =
                self.pointer_lvalue("time", &pointer, &CType::Long, args[0].span(), "7.23.2.4")?;
            self.store_lvalue(
                objects,
                &lvalue,
                TypedValue::integer(CType::Long, result),
                args[0].span(),
            )?;
        }
        Ok(TypedValue::integer(CType::Long, result))
    }

    pub(super) fn eval_timespec_get_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let base = evaluated[1].to_int()?;
        if base != 1 {
            return Ok(TypedValue::int(0));
        }
        let timespec_ty = evaluated[0]
            .ty
            .element_type()
            .cloned()
            .expect("validated timespec pointer type");
        let record = self
            .record_type(&timespec_ty)
            .cloned()
            .expect("validated complete timespec type");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Diagnostic::error("host clock predates the Unix epoch", span))?;
        let mut members = Vec::with_capacity(record.members.len());
        for member in &record.members {
            let integer = match member.storage_name.as_str() {
                "tv_sec" => now.as_secs() as i128,
                "tv_nsec" => now.subsec_nanos() as i128,
                _ => 0,
            };
            members.push((
                member.storage_name.as_str().into(),
                self.stored_value_from_typed_value(
                    TypedValue::integer(member.ty.clone(), integer),
                    &member.ty,
                    span,
                )?,
            ));
        }
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let lvalue = self.pointer_lvalue(
            "timespec_get",
            &pointer,
            &timespec_ty,
            args[0].span(),
            "7.27.2.5",
        )?;
        self.store_lvalue(
            objects,
            &lvalue,
            TypedValue::aggregate(timespec_ty, StoredValue::Record(members)),
            args[0].span(),
        )?;
        Ok(TypedValue::int(1))
    }

    pub(super) fn eval_asctime_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let host_tm =
            self.load_host_tm_from_pointer("asctime", &evaluated[0], args[0].span(), objects)?;
        let result_ptr = unsafe { asctime(&host_tm) };
        let return_ty = self.host_function_return_type("asctime", span.file, span)?;
        if result_ptr.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let pointer = self.intern_time_c_bytes(
            unsafe { CStr::from_ptr(result_ptr) }.to_bytes(),
            span,
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, pointer))
    }

    pub(super) fn eval_ctime_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let lvalue =
            self.pointer_lvalue("ctime", &pointer, &CType::Long, args[0].span(), "7.23.3.1")?;
        let timer = self
            .load_lvalue(lvalue, args[0].span(), objects)?
            .to_int()? as libc::time_t;
        let result_ptr = unsafe { ctime(&timer) };
        let return_ty = self.host_function_return_type("ctime", span.file, span)?;
        if result_ptr.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let pointer = self.intern_time_c_bytes(
            unsafe { CStr::from_ptr(result_ptr) }.to_bytes(),
            span,
            objects,
        )?;
        Ok(self.pointer_value_with_type(return_ty, pointer))
    }

    pub(super) fn eval_gmtime_like_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let lvalue = self.pointer_lvalue(
            function_name,
            &pointer,
            &CType::Long,
            args[0].span(),
            "7.23.3",
        )?;
        let timer = self
            .load_lvalue(lvalue, args[0].span(), objects)?
            .to_int()? as libc::time_t;
        let host_ptr = match function_name {
            "gmtime" => unsafe { gmtime(&timer) },
            "localtime" => unsafe { localtime(&timer) },
            _ => unreachable!("checked by caller"),
        };
        let return_ty = self.host_function_return_type(function_name, span.file, span)?;
        if host_ptr.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let tm_ty = self.tm_type_for_call(span)?;
        let host_tm = unsafe { *host_ptr };
        let object = self.allocate_object(
            objects,
            tm_ty.clone(),
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        let mut pinned = Vec::new();
        let stored = self.stored_tm_value_from_host(&tm_ty, host_tm, span, &mut pinned)?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.address_taken = true;
            state.value = stored;
        }
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(object)))
    }

    pub(super) fn eval_strftime_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let maxsize = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "strftime maxsize",
        )?;
        let format = self.c_string_argument(&evaluated[2], args[2].span(), objects)?;
        let host_tm =
            self.load_host_tm_from_pointer("strftime", &evaluated[3], args[3].span(), objects)?;
        let mut buffer = vec![0u8; maxsize];
        let written = unsafe {
            strftime(
                buffer.as_mut_ptr().cast::<c_char>(),
                maxsize,
                format.as_ptr(),
                &host_tm,
            )
        };
        if written != 0 {
            let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
            let total = written + 1;
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
            self.overlay_known_bytes_into_object(
                object_id,
                start,
                &buffer[..total],
                args[0].span(),
                objects,
            )?;
        } else if maxsize != 0 {
            let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest_pointer, maxsize, args[0].span(), objects)?;
            self.overlay_byte_cells_into_object(
                object_id,
                start,
                &vec![ByteCell::Indeterminate; maxsize],
                args[0].span(),
                objects,
            )?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, written as i128))
    }

    pub(super) fn eval_wcsftime_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let maxsize = self.checked_usize_from_unsigned_long(
            &evaluated[1],
            args[1].span(),
            "wcsftime maxsize",
        )?;
        let format = self.wide_c_string_argument(&evaluated[2], args[2].span(), objects)?;
        let host_tm =
            self.load_host_tm_from_pointer("wcsftime", &evaluated[3], args[3].span(), objects)?;
        let mut buffer = vec![0 as libc::wchar_t; maxsize.max(1)];
        let written = unsafe { wcsftime(buffer.as_mut_ptr(), maxsize, format.as_ptr(), &host_tm) };
        if written != 0 {
            let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
            self.write_wide_string_to_pointer(
                &dest_pointer,
                &buffer[..written],
                args[0].span(),
                objects,
            )?;
        } else if maxsize != 0 {
            let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
            let total = maxsize
                .checked_mul(self.wide_char_byte_width())
                .ok_or_else(|| {
                    Diagnostic::error(
                        "wcsftime destination size is out of supported range",
                        args[1].span(),
                    )
                })?;
            let (object_id, start, _) =
                self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
            self.overlay_byte_cells_into_object(
                object_id,
                start,
                &vec![ByteCell::Indeterminate; total],
                args[0].span(),
                objects,
            )?;
        }
        Ok(TypedValue::integer(CType::UnsignedLong, written as i128))
    }

    pub(super) fn eval_fegetexceptflag_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let mask = evaluated[1].to_int()? as c_int;
        #[cfg(not(target_os = "wasi"))]
        let (flag, result) = {
            let mut flag: HostFExcept = 0;
            let result = unsafe { fegetexceptflag(&mut flag, mask) };
            (flag, result)
        };
        #[cfg(target_os = "wasi")]
        let (flag, result) = (
            (self.modeled_fenv.borrow().exceptions & mask) as HostFExcept,
            0,
        );
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 2, args[0].span(), objects)?;
        self.overlay_known_bytes_into_object(
            object_id,
            start,
            &flag.to_le_bytes(),
            args[0].span(),
            objects,
        )?;
        if result == 0 {
            let version = self
                .lookup_object(objects, object_id)
                .expect("fexcept_t output object exists")
                .modification_count;
            self.fexcept_provenance
                .insert((object_id, start), (version, evaluated[1].to_int()?));
        }
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fesetexceptflag_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let bytes = self.read_pointer_bytes(
            &evaluated[0].as_pointer(args[0].span())?,
            2,
            args[0].span(),
            objects,
        )?;
        let flag = HostFExcept::from_le_bytes([bytes[0], bytes[1]]);
        let mask = evaluated[1].to_int()? as c_int;
        #[cfg(not(target_os = "wasi"))]
        let result = unsafe { fesetexceptflag(&flag, mask) };
        #[cfg(target_os = "wasi")]
        let result = {
            let mut environment = self.modeled_fenv.borrow_mut();
            environment.exceptions = (environment.exceptions & !mask) | (c_int::from(flag) & mask);
            0
        };
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fegetenv_like_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        #[cfg(not(target_os = "wasi"))]
        let (env, result) = {
            let mut env = HostFEnv { opaque: [0; 16] };
            let result = match function_name {
                "fegetenv" => unsafe { fegetenv(&mut env) },
                "feholdexcept" => unsafe { feholdexcept(&mut env) },
                _ => unreachable!("checked by caller"),
            };
            (env, result)
        };
        #[cfg(target_os = "wasi")]
        let (env, result) = {
            let saved = *self.modeled_fenv.borrow();
            if function_name == "feholdexcept" {
                self.modeled_fenv.borrow_mut().exceptions = 0;
            }
            (
                HostFEnv {
                    opaque: Self::encode_modeled_fenv(saved),
                },
                0,
            )
        };
        let (object_id, start, _) =
            self.byte_region_from_pointer(&pointer, 16, args[0].span(), objects)?;
        self.overlay_known_bytes_into_object(
            object_id,
            start,
            &env.opaque,
            args[0].span(),
            objects,
        )?;
        if result == 0 {
            let version = self
                .lookup_object(objects, object_id)
                .expect("fenv_t output object exists")
                .modification_count;
            self.fenv_provenance.insert((object_id, start), version);
        }
        Ok(TypedValue::int(result as i128))
    }

    pub(super) fn eval_fesetenv_like_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let bytes = self.read_pointer_bytes(
            &evaluated[0].as_pointer(args[0].span())?,
            16,
            args[0].span(),
            objects,
        )?;
        let mut opaque = [0u8; 16];
        opaque.copy_from_slice(&bytes);
        #[cfg(not(target_os = "wasi"))]
        let result = {
            let env = HostFEnv { opaque };
            match function_name {
                "fesetenv" => unsafe { fesetenv(&env) },
                "feupdateenv" => unsafe { feupdateenv(&env) },
                _ => unreachable!("checked by caller"),
            }
        };
        #[cfg(target_os = "wasi")]
        let result = {
            let mut saved = Self::decode_modeled_fenv(opaque);
            if function_name == "feupdateenv" {
                saved.exceptions |= self.modeled_fenv.borrow().exceptions;
            }
            *self.modeled_fenv.borrow_mut() = saved;
            0
        };
        Ok(TypedValue::int(result as i128))
    }

    #[cfg(target_os = "wasi")]
    fn encode_modeled_fenv(environment: ModeledFloatingEnvironment) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&environment.exceptions.to_le_bytes());
        bytes[4..8].copy_from_slice(&environment.rounding.to_le_bytes());
        bytes
    }

    #[cfg(target_os = "wasi")]
    fn decode_modeled_fenv(bytes: [u8; 16]) -> ModeledFloatingEnvironment {
        ModeledFloatingEnvironment {
            exceptions: c_int::from_le_bytes(bytes[..4].try_into().unwrap()),
            rounding: c_int::from_le_bytes(bytes[4..8].try_into().unwrap()),
        }
    }

    pub(super) fn eval_fenv_scalar_call(
        &self,
        function_name: &str,
        value: Option<c_int>,
    ) -> TypedValue {
        #[cfg(not(target_os = "wasi"))]
        let result = unsafe {
            match function_name {
                "feclearexcept" => feclearexcept(value.unwrap()),
                "feraiseexcept" => feraiseexcept(value.unwrap()),
                "fetestexcept" => fetestexcept(value.unwrap()),
                "fegetround" => fegetround(),
                "fesetround" => fesetround(value.unwrap()),
                _ => unreachable!("checked by caller"),
            }
        };
        #[cfg(target_os = "wasi")]
        let result = match function_name {
            "feclearexcept" => {
                self.modeled_fenv.borrow_mut().exceptions &= !value.unwrap();
                0
            }
            "feraiseexcept" => {
                self.modeled_fenv.borrow_mut().exceptions |= value.unwrap();
                0
            }
            "fetestexcept" => self.modeled_fenv.borrow().exceptions & value.unwrap(),
            "fegetround" => self.modeled_fenv.borrow().rounding,
            "fesetround" => {
                let requested = value.unwrap();
                if matches!(
                    requested,
                    HOST_FE_TONEAREST | HOST_FE_UPWARD | HOST_FE_DOWNWARD | HOST_FE_TOWARDZERO
                ) {
                    self.modeled_fenv.borrow_mut().rounding = requested;
                    0
                } else {
                    1
                }
            }
            _ => unreachable!("checked by caller"),
        };
        TypedValue::int(result as i128)
    }

    fn ensure_fe_dfl_env_binding(
        &mut self,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<ObjectId, Diagnostic> {
        if let Some(object) = self.fe_dfl_env_binding {
            return Ok(object);
        }
        let return_ty = self.host_function_return_type("__codex_fe_dfl_env", span.file, span)?;
        let env_ty = return_ty.element_type().cloned().ok_or_else(|| {
            Diagnostic::error("__codex_fe_dfl_env does not return an fenv_t pointer", span)
        })?;
        let object = self.allocate_object(
            objects,
            env_ty.clone(),
            StorageDuration::Static,
            span,
            false,
            false,
        )?;
        let bytes = self
            .startup_fenv
            .iter()
            .copied()
            .map(ByteCell::Known)
            .collect::<Vec<_>>();
        let stored = self.deserialize_stored_value(&env_ty, &bytes, span)?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.initialized = true;
            state.address_taken = true;
            state.readonly = true;
            state.value = stored;
        }
        self.fe_dfl_env_binding = Some(object);
        Ok(object)
    }

    pub(super) fn eval_fe_dfl_env_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let object = self.ensure_fe_dfl_env_binding(objects, span)?;
        let return_ty = self.host_function_return_type("__codex_fe_dfl_env", span.file, span)?;
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(object)))
    }

    pub(super) fn eval_malloc_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let Some(PendingHeapCall::Malloc(size)) = self.pending_heap_call.take() else {
            return Err(Diagnostic::error(
                "malloc precondition result was not available",
                span,
            ));
        };
        if size == 0 {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        if !self.dynamic_allocation_fits(size, None, objects) {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        let object = self.allocate_dynamic_raw_object(objects, size, span, false)?;
        Ok(self.void_pointer_value(PointerValue::at_object(object)))
    }

    pub(super) fn eval_aligned_alloc_call(
        &mut self,
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let alignment =
            self.checked_usize_from_unsigned_long(&evaluated[0], span, "aligned_alloc alignment")?;
        let size =
            self.checked_usize_from_unsigned_long(&evaluated[1], span, "aligned_alloc size")?;
        if size == 0
            || alignment == 0
            || !alignment.is_power_of_two()
            || size % alignment != 0
            || !self.dynamic_allocation_fits(size, None, objects)
        {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        let object = self.allocate_dynamic_raw_object(objects, size, span, false)?;
        self.ensure_object_alignment(
            object,
            &CType::array_of(CType::UnsignedChar, size),
            Some(alignment),
        );
        Ok(self.void_pointer_value(PointerValue::at_object(object)))
    }

    pub(super) fn eval_calloc_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let Some(PendingHeapCall::Calloc(total)) = self.pending_heap_call.take() else {
            return Err(Diagnostic::error(
                "calloc precondition result was not available",
                span,
            ));
        };
        let Some(total) = total else {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        };
        if total == 0 {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        if !self.dynamic_allocation_fits(total, None, objects) {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        let object = self.allocate_dynamic_raw_object(objects, total, span, true)?;
        Ok(self.void_pointer_value(PointerValue::at_object(object)))
    }

    pub(super) fn eval_realloc_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let Some(PendingHeapCall::Realloc {
            object: old_object,
            size,
        }) = self.pending_heap_call.take()
        else {
            return Err(Diagnostic::error(
                "realloc precondition result was not available",
                span,
            ));
        };
        let Some(old_object_id) = old_object else {
            if size == 0 || !self.dynamic_allocation_fits(size, None, objects) {
                return Ok(self.void_pointer_value(Self::null_pointer()));
            }
            let object = self.allocate_dynamic_raw_object(objects, size, span, false)?;
            return Ok(self.void_pointer_value(PointerValue::at_object(object)));
        };
        if size == 0 {
            let _ = self.dynamic_allocations.remove(&old_object_id);
            self.retire_dynamic_object(old_object_id, objects)?;
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        if !self.dynamic_allocation_fits(size, Some(old_object_id), objects) {
            return Ok(self.void_pointer_value(Self::null_pointer()));
        }
        if !self.dynamic_allocations.contains(&old_object_id) {
            return Err(Diagnostic::error(
                "heap allocation is missing its interpreter backing store",
                span,
            ));
        }
        let old_snapshot = self.lookup_object(objects, old_object_id).cloned().ok_or_else(|| {
            Diagnostic::ub(
                "realloc requires a pointer value returned by malloc/calloc/realloc or a null pointer",
                span,
                Some("7.20.3.4"),
            )
        })?;
        let old_bytes = self.serialize_stored_value(&old_snapshot.ty, &old_snapshot.value, span)?;
        let copy_len = old_bytes.len().min(size);
        let mut new_bytes = vec![ByteCell::Indeterminate; size];
        self.overlay_bytes(&mut new_bytes, 0, &old_bytes[..copy_len]);
        let indeterminate_bytes = new_bytes
            .iter()
            .filter(|byte| matches!(byte, ByteCell::Indeterminate))
            .count();
        let object = self.allocate_dynamic_raw_object(objects, size, span, false)?;
        let new_value = StoredValue::ObjectRepresentation(new_bytes);
        let initialized = self.stored_value_is_determinate(&new_value);
        let preserved_effective_types = old_snapshot
            .effective_types
            .iter()
            .filter(|&region| {
                region
                    .start
                    .checked_add(region.size)
                    .is_some_and(|end| end <= copy_len)
            })
            .cloned()
            .collect();
        let preserved_pointer_slots = Self::copied_pointer_slots(&old_snapshot, 0, 0, copy_len);
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.value = new_value;
            state.initialized = initialized;
            state.indeterminate_reason = None;
            state.effective_types = preserved_effective_types;
            state.pointer_slots.extend(preserved_pointer_slots);
            state.raw_indeterminate_bytes = Some((state.modification_count, indeterminate_bytes));
        }
        let _ = self.dynamic_allocations.remove(&old_object_id);
        self.retire_dynamic_object(old_object_id, objects)?;
        Ok(self.void_pointer_value(PointerValue::at_object(object)))
    }

    fn dynamic_allocation_fits(
        &self,
        requested_size: usize,
        replaced_object: Option<ObjectId>,
        objects: &ObjectFrames,
    ) -> bool {
        let Some(allocation_limit) = self.run_options.allocation_limit_bytes else {
            return true;
        };
        if requested_size > allocation_limit {
            return false;
        }
        let replaced_size = replaced_object
            .and_then(|object| self.lookup_object(objects, object))
            .map(|object| object.byte_size)
            .unwrap_or(0);
        self.dynamic_bytes_allocated
            .checked_sub(replaced_size)
            .and_then(|used| used.checked_add(requested_size))
            .is_some_and(|total| total <= allocation_limit)
    }

    pub(super) fn eval_free_call(
        &mut self,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let Some(PendingHeapCall::Free(object)) = self.pending_heap_call.take() else {
            return Err(Diagnostic::error(
                "free precondition result was not available",
                span,
            ));
        };
        let Some(object_id) = object else {
            return Ok(TypedValue::void());
        };
        let _ = self.dynamic_allocations.remove(&object_id);
        self.retire_dynamic_object(object_id, objects)?;
        Ok(TypedValue::void())
    }

    pub(super) fn eval_memory_copy_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        frame: &Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let src = evaluated[1].clone();
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            &format!("{function_name} byte count"),
        )?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = src.as_pointer(args[1].span())?;
        self.sequence_point();
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer_untyped(&dest_pointer, size, args[0].span(), objects)?;
        let (_, src_start, _) =
            self.byte_region_from_pointer_untyped(&src_pointer, size, args[1].span(), objects)?;
        if size == 0 {
            return Ok(dest);
        }
        let src_snapshot = self
            .lookup_object(objects, src_pointer.object.unwrap())
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    args[1].span(),
                    Some("6.2.4"),
                )
            })?;
        let source_expr_ty = self.value_expr_type(&args[1], frame, objects)?;
        let source_pointee = source_expr_ty.element_type();
        let copied_effective_types =
            self.copied_effective_type_regions(&src_snapshot, src_start, size, source_pointee);
        let copied_pointer_slots =
            Self::copied_pointer_slots(&src_snapshot, src_start, dest_start, size);
        let next_effective_types =
            self.lookup_object(objects, dest_object_id)
                .and_then(|dest_object| {
                    self.dynamic_effective_types_after_copy(
                        dest_object,
                        dest_start,
                        size,
                        src_start,
                        copied_effective_types,
                    )
                });
        let src_all_bytes =
            self.serialize_stored_value(&src_snapshot.ty, &src_snapshot.value, args[1].span())?;
        let copied_bytes = src_all_bytes[src_start..src_start + size].to_vec();
        self.overlay_byte_cells_into_object(
            dest_object_id,
            dest_start,
            &copied_bytes,
            args[0].span(),
            objects,
        )?;
        if let (Some(dest_object), Some(next_effective_types)) = (
            self.lookup_object_mut(objects, dest_object_id),
            next_effective_types,
        ) {
            dest_object.effective_types = next_effective_types;
        }
        if let Some(dest_object) = self.lookup_object_mut(objects, dest_object_id) {
            dest_object.pointer_slots.extend(copied_pointer_slots);
        }
        Ok(dest)
    }

    pub(super) fn eval_memset_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "memset byte count",
        )?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        self.sequence_point();
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer_untyped(&dest_pointer, size, args[0].span(), objects)?;
        if size == 0 {
            return Ok(dest);
        }
        let mut filled = vec![0u8; size];
        if size != 0 {
            unsafe {
                libc::memset(
                    filled.as_mut_ptr().cast::<c_void>(),
                    evaluated[1].to_int()? as c_int,
                    size,
                );
            }
        }
        self.overlay_known_bytes_into_object(
            dest_object_id,
            dest_start,
            &filled,
            args[0].span(),
            objects,
        )?;
        self.update_object_effective_types_after_store(
            objects,
            dest_object_id,
            dest_start,
            size,
            &CType::UnsignedChar,
        );
        Ok(dest)
    }

    pub(super) fn eval_memchr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "memchr byte count",
        )?;
        let return_ty = self.host_function_return_type("memchr", span.file, span)?;
        if size == 0 {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let (object_id, start, _) =
            self.byte_region_from_pointer_untyped(&pointer, size, args[0].span(), objects)?;
        let snapshot = self
            .lookup_object(objects, object_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    args[0].span(),
                    Some("6.2.4"),
                )
            })?;
        let all_bytes =
            self.serialize_stored_value(&snapshot.ty, &snapshot.value, args[0].span())?;
        let bytes = self.known_bytes(&all_bytes[start..start + size], args[0].span())?;
        let needle = evaluated[1].to_int()? as c_int;
        let found = unsafe { libc::memchr(bytes.as_ptr().cast::<c_void>(), needle, size) };
        if found.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let offset = unsafe { found.cast::<u8>().offset_from(bytes.as_ptr()) } as usize;
        // memchr's n-byte region is its array for this operation (DR 0042/0054),
        // even when it spans a nested source array's ordinary arithmetic domain.
        let mut region_pointer = pointer.clone();
        region_pointer.arithmetic_domain_start = None;
        region_pointer.object_representation_domain = None;
        let result_pointer =
            self.pointer_with_byte_offset(&region_pointer, offset, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_memcmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "memcmp byte count",
        )?;
        if size == 0 {
            return Ok(TypedValue::int(0));
        }
        let lhs_pointer = evaluated[0].as_pointer(args[0].span())?;
        let rhs_pointer = evaluated[1].as_pointer(args[1].span())?;
        let (_, lhs_start, _) =
            self.byte_region_from_pointer_untyped(&lhs_pointer, size, args[0].span(), objects)?;
        let (_, rhs_start, _) =
            self.byte_region_from_pointer_untyped(&rhs_pointer, size, args[1].span(), objects)?;
        let lhs_snapshot = self
            .lookup_object(objects, lhs_pointer.object.unwrap())
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    args[0].span(),
                    Some("6.2.4"),
                )
            })?;
        let rhs_snapshot = self
            .lookup_object(objects, rhs_pointer.object.unwrap())
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    args[1].span(),
                    Some("6.2.4"),
                )
            })?;
        let lhs_all =
            self.serialize_stored_value(&lhs_snapshot.ty, &lhs_snapshot.value, args[0].span())?;
        let rhs_all =
            self.serialize_stored_value(&rhs_snapshot.ty, &rhs_snapshot.value, args[1].span())?;
        let lhs_bytes = self.known_bytes(&lhs_all[lhs_start..lhs_start + size], args[0].span())?;
        let rhs_bytes = self.known_bytes(&rhs_all[rhs_start..rhs_start + size], args[1].span())?;
        let cmp = unsafe {
            libc::memcmp(
                lhs_bytes.as_ptr().cast::<c_void>(),
                rhs_bytes.as_ptr().cast::<c_void>(),
                size,
            )
        };
        Ok(TypedValue::int(cmp as i128))
    }

    pub(super) fn eval_strlen_call(
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
        let mut c_bytes = bytes;
        c_bytes.push(0);
        let len = unsafe { libc::strlen(c_bytes.as_ptr().cast::<c_char>()) };
        Ok(TypedValue::integer(CType::UnsignedLong, len as i128))
    }

    pub(super) fn eval_strcmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let mut lhs = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let mut rhs = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        lhs.push(0);
        rhs.push(0);
        let cmp =
            unsafe { libc::strcmp(lhs.as_ptr().cast::<c_char>(), rhs.as_ptr().cast::<c_char>()) };
        Ok(TypedValue::int(cmp as i128))
    }

    pub(super) fn eval_strncmp_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "strncmp byte count",
        )?;
        if size == 0 {
            return Ok(TypedValue::int(0));
        }
        let lhs = self.read_bounded_c_string_source(
            evaluated[0].as_pointer(args[0].span())?,
            size,
            args[0].span(),
            objects,
        )?;
        let rhs = self.read_bounded_c_string_source(
            evaluated[1].as_pointer(args[1].span())?,
            size,
            args[1].span(),
            objects,
        )?;
        let cmp = unsafe {
            libc::strncmp(
                lhs.as_ptr().cast::<c_char>(),
                rhs.as_ptr().cast::<c_char>(),
                size,
            )
        };
        Ok(TypedValue::int(cmp as i128))
    }

    pub(super) fn eval_strcoll_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let mut lhs = self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?;
        let mut rhs = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        lhs.push(0);
        rhs.push(0);
        let cmp =
            unsafe { libc::strcoll(lhs.as_ptr().cast::<c_char>(), rhs.as_ptr().cast::<c_char>()) };
        Ok(TypedValue::int(cmp as i128))
    }

    pub(super) fn eval_strcpy_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = evaluated[1].as_pointer(args[1].span())?;
        let src_bytes = self.read_c_string_bytes(src_pointer, args[1].span(), objects)?;
        let size = src_bytes.len() + 1;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, size, args[0].span(), objects)?;
        let mut src_c = src_bytes;
        src_c.push(0);
        let mut copied = vec![0u8; size];
        unsafe {
            libc::strcpy(
                copied.as_mut_ptr().cast::<c_char>(),
                src_c.as_ptr().cast::<c_char>(),
            );
        }
        self.overlay_known_bytes_into_object(
            dest_object_id,
            dest_start,
            &copied,
            args[0].span(),
            objects,
        )?;
        Ok(dest)
    }

    pub(super) fn eval_strcat_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = evaluated[1].as_pointer(args[1].span())?;
        let dest_bytes = self.read_c_string_bytes(dest_pointer.clone(), args[0].span(), objects)?;
        let src_bytes = self.read_c_string_bytes(src_pointer, args[1].span(), objects)?;
        let total = dest_bytes.len() + src_bytes.len() + 1;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
        let mut dest_buffer = vec![0u8; total];
        dest_buffer[..dest_bytes.len()].copy_from_slice(&dest_bytes);
        dest_buffer[dest_bytes.len()] = 0;
        let mut src_c = src_bytes;
        src_c.push(0);
        unsafe {
            libc::strcat(
                dest_buffer.as_mut_ptr().cast::<c_char>(),
                src_c.as_ptr().cast::<c_char>(),
            );
        }
        self.overlay_known_bytes_into_object(
            dest_object_id,
            dest_start,
            &dest_buffer,
            args[0].span(),
            objects,
        )?;
        Ok(dest)
    }

    pub(super) fn eval_strncpy_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "strncpy byte count",
        )?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = evaluated[1].as_pointer(args[1].span())?;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, size, args[0].span(), objects)?;
        let src_buffer =
            self.read_bounded_c_string_source(src_pointer, size, args[1].span(), objects)?;
        let mut copied = vec![0u8; size];
        if size != 0 {
            unsafe {
                libc::strncpy(
                    copied.as_mut_ptr().cast::<c_char>(),
                    src_buffer.as_ptr().cast::<c_char>(),
                    size,
                );
            }
        }
        self.overlay_known_bytes_into_object(
            dest_object_id,
            dest_start,
            &copied,
            args[0].span(),
            objects,
        )?;
        Ok(dest)
    }

    pub(super) fn eval_strncat_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let dest = evaluated[0].clone();
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "strncat byte count",
        )?;
        let dest_pointer = dest.as_pointer(args[0].span())?;
        let src_pointer = evaluated[1].as_pointer(args[1].span())?;
        let dest_bytes = self.read_c_string_bytes(dest_pointer.clone(), args[0].span(), objects)?;
        if size == 0 {
            return Ok(dest);
        }
        let src_buffer =
            self.read_bounded_c_string_source(src_pointer, size, args[1].span(), objects)?;
        let copy_len = src_buffer
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(size);
        let total = dest_bytes.len() + copy_len + 1;
        let (dest_object_id, dest_start, _) =
            self.byte_region_from_pointer(&dest_pointer, total, args[0].span(), objects)?;
        let mut dest_buffer = vec![0u8; total];
        dest_buffer[..dest_bytes.len()].copy_from_slice(&dest_bytes);
        dest_buffer[dest_bytes.len()] = 0;
        unsafe {
            libc::strncat(
                dest_buffer.as_mut_ptr().cast::<c_char>(),
                src_buffer.as_ptr().cast::<c_char>(),
                size,
            );
        }
        self.overlay_known_bytes_into_object(
            dest_object_id,
            dest_start,
            &dest_buffer,
            args[0].span(),
            objects,
        )?;
        Ok(dest)
    }

    pub(super) fn eval_strxfrm_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let size = self.checked_usize_from_unsigned_long(
            &evaluated[2],
            args[2].span(),
            "strxfrm byte count",
        )?;
        let dest_pointer = evaluated[0].as_pointer(args[0].span())?;
        let src_bytes = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let mut src_c = src_bytes;
        src_c.push(0);
        let mut transformed = vec![0u8; size];
        let result = unsafe {
            libc::strxfrm(
                if size == 0 {
                    std::ptr::null_mut()
                } else {
                    transformed.as_mut_ptr().cast::<c_char>()
                },
                src_c.as_ptr().cast::<c_char>(),
                size,
            )
        };
        if size != 0 {
            let written = if result < size { result + 1 } else { size };
            if written != 0 {
                let (dest_object_id, dest_start, _) =
                    self.byte_region_from_pointer(&dest_pointer, size, args[0].span(), objects)?;
                self.overlay_known_bytes_into_object(
                    dest_object_id,
                    dest_start,
                    &transformed[..written],
                    args[0].span(),
                    objects,
                )?;
            }
        }
        Ok(TypedValue::integer(CType::UnsignedLong, result as i128))
    }

    pub(super) fn eval_strerror_call(
        &mut self,
        evaluated: &[TypedValue],
        _args: &[Expr],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        if let Some(previous) = self.strerror_binding.take() {
            self.end_live_retired_object_lifetime(previous);
        }
        let message_ptr = unsafe { libc::strerror(evaluated[0].to_int()? as c_int) };
        let bytes = if message_ptr.is_null() {
            Vec::new()
        } else {
            unsafe { CStr::from_ptr(message_ptr) }.to_bytes().to_vec()
        };
        let return_ty = self.host_function_return_type("strerror", span.file, span)?;
        let pointer = self.intern_readonly_c_bytes(&bytes);
        self.strerror_binding = pointer.object;
        Ok(self.pointer_value_with_type(return_ty, pointer))
    }

    pub(super) fn eval_strdup_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let src_pointer = evaluated[0].as_pointer(args[0].span())?;
        let mut bytes = self.read_c_string_bytes(src_pointer, args[0].span(), objects)?;
        bytes.push(0);
        let return_ty = self.host_function_return_type("strdup", span.file, span)?;
        if !self.dynamic_allocation_fits(bytes.len(), None, objects) {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let object = self.allocate_dynamic_raw_object(objects, bytes.len(), span, false)?;
        self.overlay_known_bytes_into_object(object, 0, &bytes, span, objects)?;
        Ok(self.pointer_value_with_type(return_ty, PointerValue::at_object(object)))
    }

    pub(super) fn eval_strchr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let mut bytes = self.read_c_string_bytes(pointer.clone(), args[0].span(), objects)?;
        bytes.push(0);
        let return_ty = self.host_function_return_type("strchr", span.file, span)?;
        let found = unsafe {
            libc::strchr(
                bytes.as_ptr().cast::<c_char>(),
                evaluated[1].to_int()? as c_int,
            )
        };
        if found.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let offset = unsafe { found.offset_from(bytes.as_ptr().cast::<c_char>()) } as usize;
        let result_pointer =
            self.pointer_with_byte_offset(&pointer, offset, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_strrchr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let mut bytes = self.read_c_string_bytes(pointer.clone(), args[0].span(), objects)?;
        bytes.push(0);
        let return_ty = self.host_function_return_type("strrchr", span.file, span)?;
        let found = unsafe {
            libc::strrchr(
                bytes.as_ptr().cast::<c_char>(),
                evaluated[1].to_int()? as c_int,
            )
        };
        if found.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let offset = unsafe { found.offset_from(bytes.as_ptr().cast::<c_char>()) } as usize;
        let result_pointer =
            self.pointer_with_byte_offset(&pointer, offset, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_strspn_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = CString::new(self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?)
        .unwrap();
        let rhs = CString::new(self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?)
        .unwrap();
        let len = unsafe { libc::strspn(lhs.as_ptr(), rhs.as_ptr()) };
        Ok(TypedValue::integer(CType::UnsignedLong, len as i128))
    }

    pub(super) fn eval_strcspn_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        _span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = CString::new(self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?)
        .unwrap();
        let rhs = CString::new(self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?)
        .unwrap();
        let len = unsafe { libc::strcspn(lhs.as_ptr(), rhs.as_ptr()) };
        Ok(TypedValue::integer(CType::UnsignedLong, len as i128))
    }

    pub(super) fn eval_strpbrk_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let pointer = evaluated[0].as_pointer(args[0].span())?;
        let lhs =
            CString::new(self.read_c_string_bytes(pointer.clone(), args[0].span(), objects)?)
                .unwrap();
        let rhs = CString::new(self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?)
        .unwrap();
        let return_ty = self.host_function_return_type("strpbrk", span.file, span)?;
        let found = unsafe { libc::strpbrk(lhs.as_ptr(), rhs.as_ptr()) };
        if found.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let offset = unsafe { found.offset_from(lhs.as_ptr()) } as usize;
        let result_pointer =
            self.pointer_with_byte_offset(&pointer, offset, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_strstr_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let haystack_pointer = evaluated[0].as_pointer(args[0].span())?;
        let haystack = CString::new(self.read_c_string_bytes(
            haystack_pointer.clone(),
            args[0].span(),
            objects,
        )?)
        .unwrap();
        let needle = CString::new(self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?)
        .unwrap();
        let return_ty = self.host_function_return_type("strstr", span.file, span)?;
        let found = unsafe { libc::strstr(haystack.as_ptr(), needle.as_ptr()) };
        if found.is_null() {
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let offset = unsafe { found.offset_from(haystack.as_ptr()) } as usize;
        let result_pointer =
            self.pointer_with_byte_offset(&haystack_pointer, offset, args[0].span(), objects)?;
        Ok(self.pointer_value_with_type(return_ty, result_pointer))
    }

    pub(super) fn eval_strtok_call(
        &mut self,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let return_ty = self.host_function_return_type("strtok", span.file, span)?;
        let delim = self.read_c_string_bytes(
            evaluated[1].as_pointer(args[1].span())?,
            args[1].span(),
            objects,
        )?;
        let first = evaluated[0].as_pointer(args[0].span())?;
        if !first.is_null() {
            self.strtok_started = true;
        }
        let source_pointer = if first.is_null() {
            let Some(pointer) = self.strtok_state.clone() else {
                return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
            };
            pointer
        } else {
            first
        };
        let source_bytes =
            self.read_c_string_bytes(source_pointer.clone(), args[0].span(), objects)?;
        let token_start = source_bytes
            .iter()
            .position(|byte| !delim.contains(byte))
            .unwrap_or(source_bytes.len());
        if token_start == source_bytes.len() {
            self.strtok_state = None;
            return Ok(self.pointer_value_with_type(return_ty, Self::null_pointer()));
        }
        let token_pointer =
            self.pointer_with_byte_offset(&source_pointer, token_start, args[0].span(), objects)?;
        let token_bytes = &source_bytes[token_start..];
        let token_len = token_bytes
            .iter()
            .position(|byte| delim.contains(byte))
            .unwrap_or(token_bytes.len());
        let delimiter_offset = token_start + token_len;
        if delimiter_offset < source_bytes.len() {
            let (object_id, start, _) =
                self.byte_region_from_pointer(&source_pointer, 0, args[0].span(), objects)?;
            self.ensure_writable_object(object_id, args[0].span(), objects)?;
            self.overlay_known_bytes_into_object(
                object_id,
                start + delimiter_offset,
                &[0],
                args[0].span(),
                objects,
            )?;
            self.strtok_state = Some(self.pointer_with_byte_offset(
                &source_pointer,
                delimiter_offset + 1,
                args[0].span(),
                objects,
            )?);
        } else {
            self.strtok_state = None;
        }
        Ok(self.pointer_value_with_type(return_ty, token_pointer))
    }
}
