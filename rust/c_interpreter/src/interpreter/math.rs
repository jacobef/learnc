use super::*;

fn multiply_preserving_exact_zero(lhs: f64, rhs: f64) -> f64 {
    if (lhs == 0.0 && rhs.is_infinite()) || (rhs == 0.0 && lhs.is_infinite()) {
        if lhs.is_sign_negative() ^ rhs.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        lhs * rhs
    }
}

fn log_hypot(lhs: f64, rhs: f64) -> f64 {
    let larger = lhs.abs().max(rhs.abs());
    let smaller = lhs.abs().min(rhs.abs());
    if larger == 0.0 {
        f64::NEG_INFINITY
    } else {
        larger.ln() + 0.5 * ((smaller / larger) * (smaller / larger)).ln_1p()
    }
}

fn hyperbolic_product(value: f64, factor: f64, odd: bool) -> f64 {
    if value.is_nan() || factor.is_nan() {
        return f64::NAN;
    }
    if factor == 0.0 {
        let negative = factor.is_sign_negative() ^ (odd && value.is_sign_negative());
        return if negative { -0.0 } else { 0.0 };
    }
    if !value.is_finite() || !factor.is_finite() || value.abs() <= 700.0 {
        let hyperbolic = if odd { value.sinh() } else { value.cosh() };
        return multiply_preserving_exact_zero(hyperbolic, factor);
    }
    let magnitude = (value.abs() - std::f64::consts::LN_2 + factor.abs().ln()).exp();
    let negative = factor.is_sign_negative() ^ (odd && value.is_sign_negative());
    magnitude.copysign(if negative { -1.0 } else { 1.0 })
}

fn inverse_trig_average(real: f64, imag: f64) -> (f64, f64) {
    let x = real.abs();
    let y = imag.abs();
    if x <= 1.0 && y <= 1.0 {
        let left = (x + 1.0).hypot(y);
        let right = (1.0 - x).hypot(y);
        let average = left * 0.5 + right * 0.5;
        // Rationalize A - 1 instead of subtracting nearly equal numbers.
        // Keep y outside the square root so tiny imaginary parts survive.
        let height = if y == 0.0 {
            0.0
        } else {
            let left_term = y / (left + x + 1.0);
            let right_term = y / (right + 1.0 - x);
            (y.sqrt() * ((left_term + right_term) * ((average + 1.0) * 0.5)).sqrt()).asinh()
        };
        return ((real / average).clamp(-1.0, 1.0), height);
    }
    let left_log = log_hypot(real + 1.0, imag);
    let right_log = log_hypot(real - 1.0, imag);
    let larger_log = left_log.max(right_log);
    let log_average = larger_log
        + ((left_log - larger_log).exp() + (right_log - larger_log).exp()).ln()
        - std::f64::consts::LN_2;
    let ratio = if real == 0.0 {
        real
    } else {
        real.signum() * (real.abs().ln() - log_average).exp()
    }
    .clamp(-1.0, 1.0);
    let acosh_average = if log_average > 350.0 {
        log_average + std::f64::consts::LN_2
    } else {
        log_average.exp().acosh()
    };
    (ratio, acosh_average)
}

impl<'a> Interpreter<'a> {
    fn typed_math_float_result(
        &self,
        function_name: &str,
        value: f64,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let ty = self.host_function_return_type(function_name, span.file, span)?;
        Ok(TypedValue::floating(
            ty.clone(),
            self.narrow_math_result(value, &ty, span)?,
        ))
    }

    fn typed_math_int_result(
        &self,
        function_name: &str,
        value: i128,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let ty = self.host_function_return_type(function_name, span.file, span)?;
        if !ty.is_integer() {
            return Err(Diagnostic::error(
                format!("{function_name} does not return an integer type"),
                span,
            ));
        }
        Ok(TypedValue::integer(ty, value))
    }

    fn typed_complex_result(
        &self,
        function_name: &str,
        value: ComplexValue,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let ty = self.host_function_return_type(function_name, span.file, span)?;
        let component_ty = ty.complex_component_type().ok_or_else(|| {
            Diagnostic::error(
                format!("{function_name} does not return a complex type"),
                span,
            )
        })?;
        Ok(TypedValue::complex(
            ty.clone(),
            self.narrow_math_result(value.real, component_ty, span)?,
            self.narrow_math_result(value.imag, component_ty, span)?,
        ))
    }

    fn narrow_math_result(
        &self,
        value: f64,
        target: &CType,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match target.unqualified() {
            CType::Float => Ok((value as f32) as f64),
            CType::Double | CType::LongDouble => Ok(value),
            _ => Err(Diagnostic::error("math result type is not floating", span)),
        }
    }

    pub(super) fn check_complex_call(
        &self,
        function_name: &str,
        args: &[Expr],
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some((kind, component_ty, _base)) = Self::complex_function_descriptor(function_name)
        else {
            return Err(Diagnostic::error(
                format!("unsupported host library function {}", function_name),
                span,
            ));
        };
        let complex_ty = CType::complex_of(component_ty.clone());
        match kind {
            ComplexFunctionKind::Constructor => {
                self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
                for (expr, value) in args.iter().zip(evaluated) {
                    self.check_library_argument_value(value, expr.span(), function_name)?;
                    if value.ty != component_ty {
                        return Err(Diagnostic::error(
                            format!("{function_name} arguments must have type {}", component_ty),
                            expr.span(),
                        ));
                    }
                }
            }
            ComplexFunctionKind::UnaryComplex | ComplexFunctionKind::UnaryReal => {
                self.require_exact_call_args(function_name, args, evaluated, 1, span)?;
                self.reject_missing_return_value(&evaluated[0], args[0].span())?;
                self.reject_indeterminate_library_value(
                    &evaluated[0],
                    args[0].span(),
                    function_name,
                )?;
                if evaluated[0].ty != complex_ty {
                    return Err(Diagnostic::error(
                        format!(
                            "{function_name} requires an argument of type {}",
                            complex_ty
                        ),
                        args[0].span(),
                    ));
                }
            }
            ComplexFunctionKind::BinaryComplex => {
                self.require_exact_call_args(function_name, args, evaluated, 2, span)?;
                for (expr, value) in args.iter().zip(evaluated) {
                    self.check_library_argument_value(value, expr.span(), function_name)?;
                    if value.ty != complex_ty {
                        return Err(Diagnostic::error(
                            format!("{function_name} requires arguments of type {}", complex_ty),
                            expr.span(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn complex_exp_value(&self, value: ComplexValue) -> ComplexValue {
        let scale = value.real.exp();
        ComplexValue::new(
            multiply_preserving_exact_zero(scale, value.imag.cos()),
            multiply_preserving_exact_zero(scale, value.imag.sin()),
        )
    }

    fn complex_log_value(&self, value: ComplexValue) -> ComplexValue {
        ComplexValue::new(
            log_hypot(value.real, value.imag),
            value.imag.atan2(value.real),
        )
    }

    fn complex_sqrt_value(&self, value: ComplexValue) -> ComplexValue {
        if value.real == 0.0 && value.imag == 0.0 {
            return ComplexValue::new(0.0, value.imag);
        }
        if value.real.abs().max(value.imag.abs()) < f64::MIN_POSITIVE {
            let scaled = self.complex_sqrt_value(ComplexValue::new(
                value.real * 18014398509481984.0,
                value.imag * 18014398509481984.0,
            ));
            return ComplexValue::new(scaled.real / 134217728.0, scaled.imag / 134217728.0);
        }
        let half_magnitude = (value.real * 0.5).hypot(value.imag * 0.5);
        if value.real >= 0.0 {
            let real = half_magnitude.sqrt() * (1.0 + (value.real * 0.5) / half_magnitude).sqrt();
            let imag = value.imag / (2.0 * real);
            ComplexValue::new(real, imag)
        } else {
            let imag_magnitude =
                half_magnitude.sqrt() * (1.0 - (value.real * 0.5) / half_magnitude).sqrt();
            let real = value.imag.abs() / (2.0 * imag_magnitude);
            let imag = if value.imag.is_sign_negative() {
                -imag_magnitude
            } else {
                imag_magnitude
            };
            ComplexValue::new(real, imag)
        }
    }

    fn complex_proj_value(&self, value: ComplexValue) -> ComplexValue {
        if value.real.is_infinite() || value.imag.is_infinite() {
            ComplexValue::new(
                f64::INFINITY,
                if value.imag.is_sign_negative() {
                    -0.0
                } else {
                    0.0
                },
            )
        } else {
            value
        }
    }

    fn complex_asin_value(&self, value: ComplexValue) -> ComplexValue {
        let (ratio, acosh_average) = inverse_trig_average(value.real, value.imag);
        ComplexValue::new(ratio.asin(), acosh_average.copysign(value.imag))
    }

    fn complex_acos_value(&self, value: ComplexValue) -> ComplexValue {
        let (ratio, acosh_average) = inverse_trig_average(value.real, value.imag);
        ComplexValue::new(ratio.acos(), -acosh_average.copysign(value.imag))
    }

    fn complex_atan_value(&self, value: ComplexValue) -> ComplexValue {
        let scale = value.real.abs().max(value.imag.abs()).max(1.0);
        let inverse_scale = scale.recip();
        let normalized_real = value.real / scale;
        let normalized_imag = value.imag / scale;
        let numerator = 2.0 * normalized_real * inverse_scale;
        let denominator = inverse_scale * inverse_scale
            - normalized_real * normalized_real
            - normalized_imag * normalized_imag;
        let real = 0.5 * numerator.atan2(denominator);
        let imag = 0.5
            * (log_hypot(value.real, value.imag + 1.0) - log_hypot(value.real, value.imag - 1.0));
        ComplexValue::new(real, imag)
    }

    fn complex_asinh_value(&self, value: ComplexValue) -> ComplexValue {
        let asin = self.complex_asin_value(ComplexValue::new(-value.imag, value.real));
        ComplexValue::new(asin.imag, -asin.real)
    }

    fn complex_acosh_value(&self, value: ComplexValue) -> ComplexValue {
        let (ratio, acosh_average) = inverse_trig_average(value.real, value.imag);
        ComplexValue::new(acosh_average, ratio.acos().copysign(value.imag))
    }

    fn complex_atanh_value(&self, value: ComplexValue) -> ComplexValue {
        let atan = self.complex_atan_value(ComplexValue::new(-value.imag, value.real));
        ComplexValue::new(atan.imag, -atan.real)
    }

    pub(super) fn eval_complex_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let Some((kind, _component_ty, base)) = Self::complex_function_descriptor(function_name)
        else {
            return Err(Diagnostic::error(
                format!("unsupported host library function {}", function_name),
                span,
            ));
        };
        match kind {
            ComplexFunctionKind::Constructor => Ok(TypedValue::complex(
                self.host_function_return_type(function_name, span.file, span)?,
                evaluated[0].to_float()?,
                evaluated[1].to_float()?,
            )),
            ComplexFunctionKind::UnaryReal => {
                let value = evaluated[0].to_complex()?;
                let result = match base {
                    "cabs" => value.real.hypot(value.imag),
                    "carg" => value.imag.atan2(value.real),
                    "cimag" => value.imag,
                    "creal" => value.real,
                    _ => unreachable!(),
                };
                self.typed_math_float_result(function_name, result, span)
            }
            ComplexFunctionKind::BinaryComplex => {
                let lhs = evaluated[0].to_complex()?;
                let rhs = evaluated[1].to_complex()?;
                let result = match base {
                    "cpow"
                        if lhs.real == 0.0
                            && lhs.imag == 0.0
                            && rhs.imag == 0.0
                            && rhs.real > 0.0 =>
                    {
                        ComplexValue::new(0.0, 0.0)
                    }
                    "cpow" => self.complex_exp_value(rhs.mul(self.complex_log_value(lhs))),
                    _ => unreachable!(),
                };
                self.typed_complex_result(function_name, result, span)
            }
            ComplexFunctionKind::UnaryComplex => {
                let value = evaluated[0].to_complex()?;
                let result = match base {
                    "cacos" => self.complex_acos_value(value),
                    "casin" => self.complex_asin_value(value),
                    "catan" => self.complex_atan_value(value),
                    "ccos" => ComplexValue::new(
                        hyperbolic_product(value.imag, value.real.cos(), false),
                        hyperbolic_product(value.imag, -value.real.sin(), true),
                    ),
                    "csin" => ComplexValue::new(
                        hyperbolic_product(value.imag, value.real.sin(), false),
                        hyperbolic_product(value.imag, value.real.cos(), true),
                    ),
                    "ctan" => {
                        if value.imag.abs() > 350.0 {
                            ComplexValue::new(
                                0.0f64.copysign((2.0 * value.real).sin()),
                                value.imag.signum(),
                            )
                        } else {
                            let denominator = (2.0 * value.real).cos() + (2.0 * value.imag).cosh();
                            ComplexValue::new(
                                (2.0 * value.real).sin() / denominator,
                                (2.0 * value.imag).sinh() / denominator,
                            )
                        }
                    }
                    "cacosh" => self.complex_acosh_value(value),
                    "casinh" => self.complex_asinh_value(value),
                    "catanh" => self.complex_atanh_value(value),
                    "ccosh" => ComplexValue::new(
                        hyperbolic_product(value.real, value.imag.cos(), false),
                        hyperbolic_product(value.real, value.imag.sin(), true),
                    ),
                    "csinh" => ComplexValue::new(
                        hyperbolic_product(value.real, value.imag.cos(), true),
                        hyperbolic_product(value.real, value.imag.sin(), false),
                    ),
                    "ctanh" => {
                        if value.real.abs() > 350.0 {
                            ComplexValue::new(
                                value.real.signum(),
                                0.0f64.copysign((2.0 * value.imag).sin()),
                            )
                        } else {
                            let denominator = (2.0 * value.real).cosh() + (2.0 * value.imag).cos();
                            ComplexValue::new(
                                (2.0 * value.real).sinh() / denominator,
                                (2.0 * value.imag).sin() / denominator,
                            )
                        }
                    }
                    "cexp" => self.complex_exp_value(value),
                    "clog" => self.complex_log_value(value),
                    "conj" => ComplexValue::new(value.real, -value.imag),
                    "cproj" => self.complex_proj_value(value),
                    "csqrt" => self.complex_sqrt_value(value),
                    _ => unreachable!(),
                };
                self.typed_complex_result(function_name, result, span)
            }
        }
    }

    fn math_pointer_lvalue(
        &self,
        function_name: &str,
        pointer: &PointerValue,
        pointed_ty: &CType,
        span: Span,
    ) -> Result<LValue, Diagnostic> {
        let Some(object) = pointer.object else {
            return Err(Diagnostic::ub(
                format!("{function_name} requires a writable result pointer"),
                span,
                Some("7.1.4"),
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
            bit_field_width: None,
            restrict_source: None,
        })
    }

    fn store_math_output_pointer(
        &mut self,
        function_name: &str,
        pointer: &PointerValue,
        pointed_ty: &CType,
        value: TypedValue,
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let lvalue = self.math_pointer_lvalue(function_name, pointer, pointed_ty, span)?;
        self.store_lvalue(objects, &lvalue, value, span)
    }

    fn call_host_unary_real(
        &self,
        ty: &CType,
        value: f64,
        double_fn: HostUnaryF64Fn,
        float_fn: HostUnaryF32Fn,
        long_fn: HostUnaryLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float) } as f64),
            CType::Double => Ok(unsafe { double_fn(value as c_double) }),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble) } as f64),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_binary_real(
        &self,
        ty: &CType,
        lhs: f64,
        rhs: f64,
        double_fn: HostBinaryF64Fn,
        float_fn: HostBinaryF32Fn,
        long_fn: HostBinaryLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(lhs as c_float, rhs as c_float) } as f64),
            CType::Double => Ok(unsafe { double_fn(lhs as c_double, rhs as c_double) }),
            CType::LongDouble => {
                Ok(unsafe { long_fn(lhs as HostLongDouble, rhs as HostLongDouble) } as f64)
            }
            _ => Err(Diagnostic::error("expected floating arguments", span)),
        }
    }

    fn call_host_ternary_real(
        &self,
        ty: &CType,
        x: f64,
        y: f64,
        z: f64,
        double_fn: HostTernaryF64Fn,
        float_fn: HostTernaryF32Fn,
        long_fn: HostTernaryLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => {
                Ok(unsafe { float_fn(x as c_float, y as c_float, z as c_float) } as f64)
            }
            CType::Double => Ok(unsafe { double_fn(x as c_double, y as c_double, z as c_double) }),
            CType::LongDouble => Ok(unsafe {
                long_fn(
                    x as HostLongDouble,
                    y as HostLongDouble,
                    z as HostLongDouble,
                )
            } as f64),
            _ => Err(Diagnostic::error("expected floating arguments", span)),
        }
    }

    fn call_host_unary_int_result(
        &self,
        ty: &CType,
        value: f64,
        double_fn: HostUnaryIntF64Fn,
        float_fn: HostUnaryIntF32Fn,
        long_fn: HostUnaryIntLongDoubleFn,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float) } as i128),
            CType::Double => Ok(unsafe { double_fn(value as c_double) } as i128),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble) } as i128),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_unary_long_result(
        &self,
        ty: &CType,
        value: f64,
        double_fn: HostUnaryLongF64Fn,
        float_fn: HostUnaryLongF32Fn,
        long_fn: HostUnaryLongLongDoubleFn,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float) } as i128),
            CType::Double => Ok(unsafe { double_fn(value as c_double) } as i128),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble) } as i128),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_unary_longlong_result(
        &self,
        ty: &CType,
        value: f64,
        double_fn: HostUnaryLongLongF64Fn,
        float_fn: HostUnaryLongLongF32Fn,
        long_fn: HostUnaryLongLongLongDoubleFn,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float) } as i128),
            CType::Double => Ok(unsafe { double_fn(value as c_double) } as i128),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble) } as i128),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_real_int(
        &self,
        ty: &CType,
        value: f64,
        int_arg: c_int,
        double_fn: HostRealIntF64Fn,
        float_fn: HostRealIntF32Fn,
        long_fn: HostRealIntLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float, int_arg) } as f64),
            CType::Double => Ok(unsafe { double_fn(value as c_double, int_arg) }),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble, int_arg) } as f64),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_real_long(
        &self,
        ty: &CType,
        value: f64,
        long_arg: c_long,
        double_fn: HostRealLongF64Fn,
        float_fn: HostRealLongF32Fn,
        long_fn: HostRealLongLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float, long_arg) } as f64),
            CType::Double => Ok(unsafe { double_fn(value as c_double, long_arg) }),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble, long_arg) } as f64),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_real_longdouble(
        &self,
        ty: &CType,
        value: f64,
        longdouble_arg: f64,
        double_fn: HostRealLongDoubleF64Fn,
        float_fn: HostRealLongDoubleF32Fn,
        long_fn: HostRealLongDoubleLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => {
                Ok(unsafe { float_fn(value as c_float, longdouble_arg as HostLongDouble) } as f64)
            }
            CType::Double => {
                Ok(unsafe { double_fn(value as c_double, longdouble_arg as HostLongDouble) })
            }
            CType::LongDouble => {
                Ok(
                    unsafe { long_fn(value as HostLongDouble, longdouble_arg as HostLongDouble) }
                        as f64,
                )
            }
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_frexp(
        &self,
        ty: &CType,
        value: f64,
        exponent: &mut c_int,
        double_fn: HostFrexpF64Fn,
        float_fn: HostFrexpF32Fn,
        long_fn: HostFrexpLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(value as c_float, exponent) } as f64),
            CType::Double => Ok(unsafe { double_fn(value as c_double, exponent) }),
            CType::LongDouble => Ok(unsafe { long_fn(value as HostLongDouble, exponent) } as f64),
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_modf(
        &self,
        ty: &CType,
        value: f64,
        int_part: &mut f64,
        double_fn: HostModfF64Fn,
        float_fn: HostModfF32Fn,
        long_fn: HostModfLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => {
                let mut storage = *int_part as c_float;
                let frac = unsafe { float_fn(value as c_float, &mut storage) } as f64;
                *int_part = storage as f64;
                Ok(frac)
            }
            CType::Double => Ok(unsafe { double_fn(value as c_double, int_part) }),
            CType::LongDouble => {
                let mut storage = *int_part as HostLongDouble;
                let frac = unsafe { long_fn(value as HostLongDouble, &mut storage) } as f64;
                *int_part = storage as f64;
                Ok(frac)
            }
            _ => Err(Diagnostic::error("expected floating argument", span)),
        }
    }

    fn call_host_remquo(
        &self,
        ty: &CType,
        lhs: f64,
        rhs: f64,
        quotient: &mut c_int,
        double_fn: HostRemquoF64Fn,
        float_fn: HostRemquoF32Fn,
        long_fn: HostRemquoLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => {
                Ok(unsafe { float_fn(lhs as c_float, rhs as c_float, quotient) } as f64)
            }
            CType::Double => Ok(unsafe { double_fn(lhs as c_double, rhs as c_double, quotient) }),
            CType::LongDouble => {
                Ok(
                    unsafe { long_fn(lhs as HostLongDouble, rhs as HostLongDouble, quotient) }
                        as f64,
                )
            }
            _ => Err(Diagnostic::error("expected floating arguments", span)),
        }
    }

    fn call_host_nan(
        &self,
        ty: &CType,
        tag: &CString,
        double_fn: HostStringF64Fn,
        float_fn: HostStringF32Fn,
        long_fn: HostStringLongDoubleFn,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match ty.unqualified() {
            CType::Float => Ok(unsafe { float_fn(tag.as_ptr()) } as f64),
            CType::Double => Ok(unsafe { double_fn(tag.as_ptr()) }),
            CType::LongDouble => Ok(unsafe { long_fn(tag.as_ptr()) } as f64),
            _ => Err(Diagnostic::error("expected floating result type", span)),
        }
    }

    pub(super) fn eval_unary_real_math_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let input = evaluated[0].to_float()?;
        let value = match function_name {
            "acos" | "acosf" | "acosl" => {
                self.call_host_unary_real(&input_ty, input, acos, acosf, acosl, span)?
            }
            "asin" | "asinf" | "asinl" => {
                self.call_host_unary_real(&input_ty, input, asin, asinf, asinl, span)?
            }
            "atan" | "atanf" | "atanl" => {
                self.call_host_unary_real(&input_ty, input, atan, atanf, atanl, span)?
            }
            "cos" | "cosf" | "cosl" => {
                self.call_host_unary_real(&input_ty, input, cos, cosf, cosl, span)?
            }
            "sin" | "sinf" | "sinl" => {
                self.call_host_unary_real(&input_ty, input, sin, sinf, sinl, span)?
            }
            "tan" | "tanf" | "tanl" => {
                self.call_host_unary_real(&input_ty, input, tan, tanf, tanl, span)?
            }
            "acosh" | "acoshf" | "acoshl" => {
                self.call_host_unary_real(&input_ty, input, acosh, acoshf, acoshl, span)?
            }
            "asinh" | "asinhf" | "asinhl" => {
                self.call_host_unary_real(&input_ty, input, asinh, asinhf, asinhl, span)?
            }
            "atanh" | "atanhf" | "atanhl" => {
                self.call_host_unary_real(&input_ty, input, atanh, atanhf, atanhl, span)?
            }
            "cosh" | "coshf" | "coshl" => {
                self.call_host_unary_real(&input_ty, input, cosh, coshf, coshl, span)?
            }
            "sinh" | "sinhf" | "sinhl" => {
                self.call_host_unary_real(&input_ty, input, sinh, sinhf, sinhl, span)?
            }
            "tanh" | "tanhf" | "tanhl" => {
                self.call_host_unary_real(&input_ty, input, tanh, tanhf, tanhl, span)?
            }
            "exp" | "expf" | "expl" => {
                self.call_host_unary_real(&input_ty, input, exp, expf, expl, span)?
            }
            "log" | "logf" | "logl" => {
                self.call_host_unary_real(&input_ty, input, log, logf, logl, span)?
            }
            "log10" | "log10f" | "log10l" => {
                self.call_host_unary_real(&input_ty, input, log10, log10f, log10l, span)?
            }
            "exp2" | "exp2f" | "exp2l" => {
                self.call_host_unary_real(&input_ty, input, exp2, exp2f, exp2l, span)?
            }
            "expm1" | "expm1f" | "expm1l" => {
                self.call_host_unary_real(&input_ty, input, expm1, expm1f, expm1l, span)?
            }
            "log1p" | "log1pf" | "log1pl" => {
                self.call_host_unary_real(&input_ty, input, log1p, log1pf, log1pl, span)?
            }
            "log2" | "log2f" | "log2l" => {
                self.call_host_unary_real(&input_ty, input, log2, log2f, log2l, span)?
            }
            "logb" | "logbf" | "logbl" => {
                self.call_host_unary_real(&input_ty, input, logb, logbf, logbl, span)?
            }
            "sqrt" | "sqrtf" | "sqrtl" => {
                self.call_host_unary_real(&input_ty, input, sqrt, sqrtf, sqrtl, span)?
            }
            "cbrt" | "cbrtf" | "cbrtl" => {
                self.call_host_unary_real(&input_ty, input, cbrt, cbrtf, cbrtl, span)?
            }
            "erf" | "erff" | "erfl" => {
                self.call_host_unary_real(&input_ty, input, erf, erff, erfl, span)?
            }
            "erfc" | "erfcf" | "erfcl" => {
                self.call_host_unary_real(&input_ty, input, erfc, erfcf, erfcl, span)?
            }
            "tgamma" | "tgammaf" | "tgammal" => {
                self.call_host_unary_real(&input_ty, input, tgamma, tgammaf, tgammal, span)?
            }
            "lgamma" | "lgammaf" | "lgammal" => {
                let value =
                    self.call_host_unary_real(&input_ty, input, lgamma, lgammaf, lgammal, span)?;
                self.sync_host_signgam(objects, span)?;
                value
            }
            "fabs" | "fabsf" | "fabsl" => {
                self.call_host_unary_real(&input_ty, input, fabs, fabsf, fabsl, span)?
            }
            "ceil" | "ceilf" | "ceill" => {
                self.call_host_unary_real(&input_ty, input, ceil, ceilf, ceill, span)?
            }
            "floor" | "floorf" | "floorl" => {
                self.call_host_unary_real(&input_ty, input, floor, floorf, floorl, span)?
            }
            "trunc" | "truncf" | "truncl" => {
                self.call_host_unary_real(&input_ty, input, trunc, truncf, truncl, span)?
            }
            "round" | "roundf" | "roundl" => {
                self.call_host_unary_real(&input_ty, input, round, roundf, roundl, span)?
            }
            "rint" | "rintf" | "rintl" => {
                self.call_host_unary_real(&input_ty, input, rint, rintf, rintl, span)?
            }
            "nearbyint" | "nearbyintf" | "nearbyintl" => self
                .call_host_unary_real(&input_ty, input, nearbyint, nearbyintf, nearbyintl, span)?,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported unary math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_binary_real_math_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let lhs = evaluated[0].to_float()?;
        let rhs = evaluated[1].to_float()?;
        let value = match function_name {
            "atan2" | "atan2f" | "atan2l" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, atan2, atan2f, atan2l, span)?
            }
            "pow" | "powf" | "powl" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, pow, powf, powl, span)?
            }
            "hypot" | "hypotf" | "hypotl" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, hypot, hypotf, hypotl, span)?
            }
            "fmod" | "fmodf" | "fmodl" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, fmod, fmodf, fmodl, span)?
            }
            "remainder" | "remainderf" | "remainderl" => self.call_host_binary_real(
                &input_ty, lhs, rhs, remainder, remainderf, remainderl, span,
            )?,
            "copysign" | "copysignf" | "copysignl" => self
                .call_host_binary_real(&input_ty, lhs, rhs, copysign, copysignf, copysignl, span)?,
            "nextafter" | "nextafterf" | "nextafterl" => self.call_host_binary_real(
                &input_ty, lhs, rhs, nextafter, nextafterf, nextafterl, span,
            )?,
            "fdim" | "fdimf" | "fdiml" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, fdim, fdimf, fdiml, span)?
            }
            "fmax" | "fmaxf" | "fmaxl" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, fmax, fmaxf, fmaxl, span)?
            }
            "fmin" | "fminf" | "fminl" => {
                self.call_host_binary_real(&input_ty, lhs, rhs, fmin, fminf, fminl, span)?
            }
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported binary math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_ternary_real_math_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let x = evaluated[0].to_float()?;
        let y = evaluated[1].to_float()?;
        let z = evaluated[2].to_float()?;
        let value = match function_name {
            "fma" | "fmaf" | "fmal" => {
                self.call_host_ternary_real(&input_ty, x, y, z, fma, fmaf, fmal, span)?
            }
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported ternary math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_real_int_math_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let value = evaluated[0].to_float()?;
        let exponent = evaluated[1].to_int()?;
        let result = match function_name {
            "ldexp" | "ldexpf" | "ldexpl" => self.call_host_real_int(
                &input_ty,
                value,
                exponent as c_int,
                ldexp,
                ldexpf,
                ldexpl,
                span,
            )?,
            "scalbn" | "scalbnf" | "scalbnl" => self.call_host_real_int(
                &input_ty,
                value,
                exponent as c_int,
                scalbn,
                scalbnf,
                scalbnl,
                span,
            )?,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, result, span)
    }

    pub(super) fn eval_real_long_math_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let value = evaluated[0].to_float()?;
        let exponent = evaluated[1].to_int()? as c_long;
        let result = match function_name {
            "scalbln" | "scalblnf" | "scalblnl" => self.call_host_real_long(
                &input_ty, value, exponent, scalbln, scalblnf, scalblnl, span,
            )?,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, result, span)
    }

    pub(super) fn eval_unary_int_math_result_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let input = evaluated[0].to_float()?;
        let value = match function_name {
            "ilogb" | "ilogbf" | "ilogbl" => {
                self.call_host_unary_int_result(&input_ty, input, ilogb, ilogbf, ilogbl, span)?
            }
            "lround" | "lroundf" | "lroundl" => {
                self.call_host_unary_long_result(&input_ty, input, lround, lroundf, lroundl, span)?
            }
            "lrint" | "lrintf" | "lrintl" => {
                self.call_host_unary_long_result(&input_ty, input, lrint, lrintf, lrintl, span)?
            }
            "llround" | "llroundf" | "llroundl" => self.call_host_unary_longlong_result(
                &input_ty, input, llround, llroundf, llroundl, span,
            )?,
            "llrint" | "llrintf" | "llrintl" => self.call_host_unary_longlong_result(
                &input_ty, input, llrint, llrintf, llrintl, span,
            )?,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_int_result(function_name, value, span)
    }

    pub(super) fn eval_frexp_math_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let input = evaluated[0].to_float()?;
        let mut exponent = 0;
        let value =
            self.call_host_frexp(&input_ty, input, &mut exponent, frexp, frexpf, frexpl, span)?;
        let out_pointer = evaluated[1].as_pointer(args[1].span())?;
        let out_ty = evaluated[1]
            .ty
            .element_type()
            .cloned()
            .unwrap_or(CType::Int);
        self.store_math_output_pointer(
            function_name,
            &out_pointer,
            &out_ty,
            TypedValue::integer(out_ty.clone(), exponent as i128),
            args[1].span(),
            objects,
        )?;
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_modf_math_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let input = evaluated[0].to_float()?;
        let mut int_part = 0.0;
        let value =
            self.call_host_modf(&input_ty, input, &mut int_part, modf, modff, modfl, span)?;
        let out_pointer = evaluated[1].as_pointer(args[1].span())?;
        let out_ty = evaluated[1]
            .ty
            .element_type()
            .cloned()
            .unwrap_or_else(|| input_ty.clone());
        self.store_math_output_pointer(
            function_name,
            &out_pointer,
            &out_ty,
            TypedValue::floating(
                out_ty.clone(),
                self.convert_float_to_float(int_part, &out_ty, args[1].span())?,
            ),
            args[1].span(),
            objects,
        )?;
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_remquo_math_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let lhs = evaluated[0].to_float()?;
        let rhs = evaluated[1].to_float()?;
        let mut quotient = 0;
        let value = self.call_host_remquo(
            &input_ty,
            lhs,
            rhs,
            &mut quotient,
            remquo,
            remquof,
            remquol,
            span,
        )?;
        let out_pointer = evaluated[2].as_pointer(args[2].span())?;
        let out_ty = evaluated[2]
            .ty
            .element_type()
            .cloned()
            .unwrap_or(CType::Int);
        self.store_math_output_pointer(
            function_name,
            &out_pointer,
            &out_ty,
            TypedValue::integer(out_ty.clone(), quotient as i128),
            args[2].span(),
            objects,
        )?;
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_nan_math_call(
        &mut self,
        function_name: &str,
        evaluated: &[TypedValue],
        args: &[Expr],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let tag = CString::new(self.read_c_string_bytes(
            evaluated[0].as_pointer(args[0].span())?,
            args[0].span(),
            objects,
        )?)
        .unwrap();
        let ty = self.host_function_return_type(function_name, span.file, span)?;
        let value = self.call_host_nan(&ty, &tag, nan, nanf, nanl, span)?;
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_nexttoward_math_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let input_ty = evaluated[0].ty.clone();
        let lhs = evaluated[0].to_float()?;
        let rhs = evaluated[1].to_float()?;
        let value = match function_name {
            "nexttoward" | "nexttowardf" | "nexttowardl" => self.call_host_real_longdouble(
                &input_ty,
                lhs,
                rhs,
                nexttoward,
                nexttowardf,
                nexttowardl,
                span,
            )?,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math function {function_name}"),
                    span,
                ));
            }
        };
        self.typed_math_float_result(function_name, value, span)
    }

    pub(super) fn eval_math_helper_nullary_call(
        &self,
        function_name: &str,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        match function_name {
            "__codex_math_errhandling" => Ok(TypedValue::integer(
                CType::Int,
                HOST_MATH_ERRHANDLING as i128,
            )),
            "__codex_huge_val" => self.typed_math_float_result(function_name, f64::INFINITY, span),
            "__codex_huge_valf" => {
                self.typed_math_float_result(function_name, f32::INFINITY as f64, span)
            }
            "__codex_huge_vall" => self.typed_math_float_result(function_name, f64::INFINITY, span),
            _ => Err(Diagnostic::error(
                format!("unsupported math helper {function_name}"),
                span,
            )),
        }
    }

    pub(super) fn eval_math_unary_helper_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let value = evaluated[0].to_float()?;
        let result = match function_name {
            "__codex_fpclassifyf" => match (value as f32).classify() {
                std::num::FpCategory::Nan => HOST_FP_NAN,
                std::num::FpCategory::Infinite => HOST_FP_INFINITE,
                std::num::FpCategory::Zero => HOST_FP_ZERO,
                std::num::FpCategory::Subnormal => HOST_FP_SUBNORMAL,
                std::num::FpCategory::Normal => HOST_FP_NORMAL,
            },
            "__codex_fpclassify" | "__codex_fpclassifyl" => match value.classify() {
                std::num::FpCategory::Nan => HOST_FP_NAN,
                std::num::FpCategory::Infinite => HOST_FP_INFINITE,
                std::num::FpCategory::Zero => HOST_FP_ZERO,
                std::num::FpCategory::Subnormal => HOST_FP_SUBNORMAL,
                std::num::FpCategory::Normal => HOST_FP_NORMAL,
            },
            "__codex_isfinite" => (!value.is_nan() && !value.is_infinite()) as c_int,
            "__codex_isinf" => value.is_infinite() as c_int,
            "__codex_isnan" => value.is_nan() as c_int,
            "__codex_isnormalf" => {
                matches!((value as f32).classify(), std::num::FpCategory::Normal) as c_int
            }
            "__codex_isnormal" | "__codex_isnormall" => {
                matches!(value.classify(), std::num::FpCategory::Normal) as c_int
            }
            "__codex_signbit" => value.is_sign_negative() as c_int,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math helper {function_name}"),
                    span,
                ));
            }
        };
        Ok(TypedValue::integer(CType::Int, result as i128))
    }

    pub(super) fn eval_math_binary_helper_call(
        &self,
        function_name: &str,
        evaluated: &[TypedValue],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = evaluated[0].to_float()?;
        let rhs = evaluated[1].to_float()?;
        let unordered = lhs.is_nan() || rhs.is_nan();
        let result = match function_name {
            "__codex_isgreater" => (!unordered && lhs > rhs) as c_int,
            "__codex_isgreaterequal" => (!unordered && lhs >= rhs) as c_int,
            "__codex_isless" => (!unordered && lhs < rhs) as c_int,
            "__codex_islessequal" => (!unordered && lhs <= rhs) as c_int,
            "__codex_islessgreater" => (!unordered && (lhs < rhs || lhs > rhs)) as c_int,
            "__codex_isunordered" => unordered as c_int,
            _ => {
                return Err(Diagnostic::error(
                    format!("unsupported math helper {function_name}"),
                    span,
                ));
            }
        };
        Ok(TypedValue::integer(CType::Int, result as i128))
    }

    pub(super) fn validate_heap_allocation_pointer(
        &self,
        pointer: &PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Option<ObjectId>, Diagnostic> {
        if pointer.is_null() {
            return Ok(None);
        }
        if pointer.base_offset != 0
            || pointer.offset != 0
            || !pointer.member_path.is_empty()
            || pointer.object.is_none()
        {
            return Err(Diagnostic::ub(
                "allocation function requires a pointer value returned by malloc/calloc/realloc or a null pointer",
                span,
                Some("7.20.3"),
            ));
        }
        let object_id = pointer.object.expect("checked above");
        let object = self.lookup_object(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "allocation function requires a pointer value returned by malloc/calloc/realloc or a null pointer",
                span,
                Some("7.20.3"),
            )
        })?;
        if object.storage_duration != StorageDuration::Dynamic {
            return Err(Diagnostic::ub(
                "allocation function requires a pointer value returned by malloc/calloc/realloc or a null pointer",
                span,
                Some("7.20.3"),
            ));
        }
        if !object.alive {
            return Err(Diagnostic::ub(
                "allocation function requires a live pointer value returned by malloc/calloc/realloc or a null pointer",
                span,
                Some("7.20.3"),
            ));
        }
        Ok(Some(object_id))
    }

    pub(super) fn ensure_writable_object(
        &self,
        object_id: ObjectId,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let object = self.lookup_object(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        if !object.alive {
            return Err(Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            ));
        }
        if object.readonly {
            return Err(Diagnostic::ub(
                "attempt to modify a string literal or other read-only object",
                span,
                Some("6.4.5"),
            ));
        }
        if object.const_object {
            return Err(Diagnostic::ub(
                "attempt to modify an object defined with a const-qualified type",
                span,
                Some("6.7.3p6"),
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_writable_region(
        &self,
        object_id: ObjectId,
        start: usize,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let object = self.lookup_object(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        if !object.alive {
            return Err(Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            ));
        }
        if object.readonly {
            return Err(Diagnostic::ub(
                "attempt to modify a string literal or other read-only object",
                span,
                Some("6.4.5"),
            ));
        }
        if object.has_const_subobject
            && self.qualified_subobject_overlaps(
                &object.ty,
                0,
                start,
                size,
                TrackedQualifier::Const,
            )
        {
            return Err(Diagnostic::ub(
                "attempt to modify a const-qualified object or subobject",
                span,
                Some("6.7.3p6"),
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_library_writable_region(
        &self,
        object_id: ObjectId,
        start: usize,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.ensure_writable_region(object_id, start, size, span, objects)?;
        if size != 0 {
            let object = self
                .lookup_object(objects, object_id)
                .expect("writable object exists");
            if object.has_volatile_subobject
                && self.qualified_subobject_overlaps(
                    &object.ty,
                    0,
                    start,
                    size,
                    TrackedQualifier::Volatile,
                )
            {
                return Err(Diagnostic::ub(
                    "library function would access a volatile-qualified object or subobject through a non-volatile pointer",
                    span,
                    Some("6.7.3"),
                ));
            }
        }
        Ok(())
    }

    /// Validate a pointer argument that the library specification describes as
    /// an array.  C11 7.1.4 still requires such pointer arguments to have valid
    /// values when the requested element count is zero.
    pub(super) fn library_array_region(
        &self,
        pointer: &PointerValue,
        access_size: usize,
        _element_size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        let region = self.byte_region_from_pointer(pointer, access_size, span, objects)?;
        if access_size != 0 {
            let object = self
                .lookup_object(objects, region.0)
                .expect("validated library array object exists");
            if object.has_volatile_subobject
                && self.qualified_subobject_overlaps(
                    &object.ty,
                    0,
                    region.1,
                    access_size,
                    TrackedQualifier::Volatile,
                )
            {
                return Err(Diagnostic::ub(
                    "library function would access a volatile-qualified object or subobject through a non-volatile pointer",
                    span,
                    Some("6.7.3"),
                ));
            }
        }
        Ok(region)
    }

    pub(super) fn ensure_library_array_destination(
        &self,
        pointer: &PointerValue,
        access_size: usize,
        element_size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        let (object_id, start, object_size) =
            self.library_array_region(pointer, access_size, element_size, span, objects)?;
        if access_size != 0 {
            self.ensure_library_writable_region(object_id, start, access_size, span, objects)?;
        }
        Ok((object_id, start, object_size))
    }

    pub(super) fn qualified_subobject_overlaps(
        &self,
        ty: &CType,
        base: usize,
        query_start: usize,
        query_size: usize,
        qualifier: TrackedQualifier,
    ) -> bool {
        if query_size == 0 {
            return false;
        }
        let qualifiers = ty.top_level_qualifiers();
        let has_tracked_qualifier = match qualifier {
            TrackedQualifier::Const => qualifiers.is_const,
            TrackedQualifier::Volatile => qualifiers.is_volatile,
        };
        if !has_tracked_qualifier
            && !matches!(
                ty.unqualified(),
                CType::Array(..) | CType::Struct(..) | CType::Union(..)
            )
        {
            return false;
        }
        let Some(type_size) = self.type_size_of(ty) else {
            return false;
        };
        let query_end = query_start.saturating_add(query_size);
        let type_end = base.saturating_add(type_size);
        if query_start >= type_end || base >= query_end {
            return false;
        }
        if has_tracked_qualifier {
            return true;
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return false;
                };
                if stride == 0 || *len == 0 {
                    return false;
                }
                let first = query_start.saturating_sub(base) / stride;
                let last = query_end
                    .saturating_sub(1)
                    .saturating_sub(base)
                    .checked_div(stride)
                    .unwrap_or(0)
                    .min(len.saturating_sub(1));
                (first..=last).any(|index| {
                    self.qualified_subobject_overlaps(
                        inner,
                        base.saturating_add(index.saturating_mul(stride)),
                        query_start,
                        query_size,
                        qualifier,
                    )
                })
            }
            CType::Struct(_, _) | CType::Union(_, _) => {
                self.record_type(ty).is_some_and(|record| {
                    record.members.iter().any(|member| {
                        self.qualified_subobject_overlaps(
                            &member.ty,
                            base.saturating_add(member.offset),
                            query_start,
                            query_size,
                            qualifier,
                        )
                    })
                })
            }
            _ => false,
        }
    }

    pub(super) fn reject_nonvolatile_library_pointer_access(
        &self,
        function_name: &str,
        value: &TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let pointer = value.as_pointer(span)?;
        let Some(object_id) = pointer.object else {
            return Ok(());
        };
        let Some(object) = self.lookup_object(objects, object_id) else {
            return Ok(());
        };
        let Some(pointee_ty) = value.ty.element_type() else {
            return Ok(());
        };
        let start = self
            .pointer_byte_offset(&pointer, pointee_ty, objects)
            .unwrap_or(0);
        if object.has_volatile_subobject
            && self.qualified_subobject_overlaps(
                &object.ty,
                0,
                start,
                1,
                TrackedQualifier::Volatile,
            )
            && !pointee_ty.top_level_qualifiers().is_volatile
        {
            return Err(Diagnostic::ub(
                format!(
                    "{function_name} would access an object defined with a volatile-qualified type through a non-volatile pointer"
                ),
                span,
                Some("6.7.3"),
            ));
        }
        Ok(())
    }

    pub(super) fn ranges_overlap(
        &self,
        lhs_object: ObjectId,
        lhs_start: usize,
        rhs_object: ObjectId,
        rhs_start: usize,
        size: usize,
    ) -> bool {
        self.ranges_overlap_with_sizes(lhs_object, lhs_start, size, rhs_object, rhs_start, size)
    }

    pub(super) fn ranges_overlap_with_sizes(
        &self,
        lhs_object: ObjectId,
        lhs_start: usize,
        lhs_size: usize,
        rhs_object: ObjectId,
        rhs_start: usize,
        rhs_size: usize,
    ) -> bool {
        lhs_size != 0
            && rhs_size != 0
            && lhs_object == rhs_object
            && lhs_start < rhs_start.saturating_add(rhs_size)
            && rhs_start < lhs_start.saturating_add(lhs_size)
    }

    pub(super) fn capture_formatted_io_accesses<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<(T, Vec<FormattedIoAccess>), Diagnostic> {
        debug_assert!(self.formatted_io_accesses.is_none());
        let previous = self.formatted_io_accesses.take();
        self.formatted_io_accesses = Some(Vec::new());
        let result = operation(self);
        let accesses = self.formatted_io_accesses.take().unwrap_or_default();
        self.formatted_io_accesses = previous;
        result.map(|value| (value, accesses))
    }

    pub(super) fn record_formatted_io_access(
        &mut self,
        pointer: &PointerValue,
        element_ty: &CType,
        element_count: usize,
        role: FormattedIoAccessRole,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if self.formatted_io_accesses.is_none() || element_count == 0 {
            return Ok(());
        }
        let object = pointer.object.ok_or_else(|| {
            Diagnostic::ub("formatted I/O accessed a null pointer", span, Some("7.1.4"))
        })?;
        let start = self
            .pointer_byte_offset(pointer, element_ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "formatted I/O pointer does not designate a valid object region",
                    span,
                    Some("7.1.4"),
                )
            })?;
        let element_size = self.type_size_of(element_ty).ok_or_else(|| {
            Diagnostic::error("formatted I/O used an incomplete element type", span)
        })?;
        let size = element_count.checked_mul(element_size).ok_or_else(|| {
            Diagnostic::ub(
                "formatted I/O access is out of supported range",
                span,
                Some("7.1.4"),
            )
        })?;
        self.formatted_io_accesses
            .as_mut()
            .expect("capture checked above")
            .push(FormattedIoAccess {
                object,
                start,
                size,
                role,
                span,
            });
        Ok(())
    }

    pub(super) fn reject_formatted_io_destination_overlap(
        &self,
        function_name: &'static str,
        destination: &PointerValue,
        destination_size: usize,
        destination_span: Span,
        accesses: &[FormattedIoAccess],
        objects: &ObjectFrames,
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        self.reject_percent_n_format_overlap(function_name, accesses, standard)?;
        if destination_size == 0 {
            return Ok(());
        }
        let (destination_object, destination_start, _) = self.byte_region_from_pointer(
            destination,
            destination_size,
            destination_span,
            objects,
        )?;
        let Some(access) = accesses.iter().find(|access| {
            self.ranges_overlap_with_sizes(
                destination_object,
                destination_start,
                destination_size,
                access.object,
                access.start,
                access.size,
            )
        }) else {
            return Ok(());
        };
        let role = match access.role {
            FormattedIoAccessRole::Format => "format string",
            FormattedIoAccessRole::StringArgument => "string argument",
            FormattedIoAccessRole::PercentN => "%n destination",
        };
        let conflicting = self.sources.snippet(access.span);
        Err(Diagnostic::ub(
            format!("{function_name} output array overlaps a {role} accessed by the same call"),
            destination_span,
            Some(standard),
        )
        .with_note(format!(
            "the conflicting {role} is passed at {}:{}:{}",
            conflicting.path.display(),
            conflicting.line_number,
            conflicting.column
        )))
    }

    fn reject_percent_n_format_overlap(
        &self,
        function_name: &'static str,
        accesses: &[FormattedIoAccess],
        standard: &'static str,
    ) -> Result<(), Diagnostic> {
        let conflict = accesses.iter().find_map(|write| {
            (write.role == FormattedIoAccessRole::PercentN).then(|| {
                accesses
                    .iter()
                    .find(|read| {
                        read.role == FormattedIoAccessRole::Format
                            && self.ranges_overlap_with_sizes(
                                write.object,
                                write.start,
                                write.size,
                                read.object,
                                read.start,
                                read.size,
                            )
                    })
                    .map(|read| (write, read))
            })?
        });
        let Some((write, format)) = conflict else {
            return Ok(());
        };
        let format_location = self.sources.snippet(format.span);
        Err(Diagnostic::ub(
            format!("{function_name} %n destination overlaps the restrict-qualified format string"),
            write.span,
            Some(standard),
        )
        .with_note(format!(
            "the overlapping format string is passed at {}:{}:{}",
            format_location.path.display(),
            format_location.line_number,
            format_location.column
        )))
    }
}
