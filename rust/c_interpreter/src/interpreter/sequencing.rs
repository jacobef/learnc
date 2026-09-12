//! Sequence points, overlapping accesses, and restrict tracking.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn sequence_point(&mut self) {
        self.expr_state.accesses.clear();
    }

    pub(super) fn sequencing_snapshot(&self) -> SequencingSnapshot {
        SequencingSnapshot {
            accesses: self.expr_state.accesses.clone(),
            assignment_targets: self.assignment_targets.clone(),
        }
    }

    pub(super) fn finish_sequenced_operand(
        &mut self,
        snapshot: SequencingSnapshot,
    ) -> HashMap<AccessRegion, ObjectAccess> {
        let mut footprint = HashMap::default();
        for (&region, access) in &self.expr_state.accesses {
            let baseline = snapshot.accesses.get(&region).copied().unwrap_or_default();
            let delta = ObjectAccess {
                self_read: (access.self_read != baseline.self_read)
                    .then_some(access.self_read)
                    .flatten(),
                other_read: (access.other_read != baseline.other_read)
                    .then_some(access.other_read)
                    .flatten(),
                write: (access.write != baseline.write)
                    .then_some(access.write)
                    .flatten(),
            };
            if delta != ObjectAccess::default() {
                footprint.insert(region, delta);
            }
        }
        self.expr_state.accesses = snapshot.accesses;
        self.assignment_targets = snapshot.assignment_targets;
        footprint
    }

    pub(super) fn merge_sequenced_footprint(
        &mut self,
        footprint: HashMap<AccessRegion, ObjectAccess>,
    ) {
        for (region, mut access) in footprint {
            if self.assignment_targets.iter().any(|target| {
                Self::access_region_contains(target.region, region)
                    && target.kind == AssignmentTargetKind::Simple
            }) {
                access.write = None;
            }
            let current = self.expr_state.accesses.entry(region).or_default();
            current.self_read = current.self_read.or(access.self_read);
            current.other_read = current.other_read.or(access.other_read);
            current.write = current.write.or(access.write);
        }
    }

    pub(super) fn accumulate_initializer_footprint(
        &mut self,
        footprint: HashMap<AccessRegion, ObjectAccess>,
    ) {
        let Some(sequence) = self.initializer_sequencing.as_mut() else {
            return;
        };
        for (region, access) in footprint {
            let accumulated = sequence.footprint.entry(region).or_default();
            accumulated.self_read = accumulated.self_read.or(access.self_read);
            accumulated.other_read = accumulated.other_read.or(access.other_read);
            accumulated.write = accumulated.write.or(access.write);
        }
    }

    fn in_host_library_runtime(&self) -> bool {
        self.host_library_runtime_depth != 0
    }

    pub(super) fn push_assignment_target(
        &mut self,
        region: AccessRegion,
        kind: AssignmentTargetKind,
    ) {
        self.assignment_targets
            .push(AssignmentTarget { region, kind });
    }

    pub(super) fn pop_assignment_target(&mut self) {
        let _ = self.assignment_targets.pop();
    }

    pub(super) fn validate_restricted_pointer_assignment(
        &self,
        target: &LValue,
        source_value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !target.ty.is_pointer() || !target.ty.top_level_qualifiers().is_restrict {
            return Ok(());
        }
        let Some(source) = source_value.restrict_source.as_ref() else {
            return Ok(());
        };
        let same_restricted_pointer = source.object == target.object
            && source.base_offset == target.base_offset
            && source.offset == target.offset
            && source.member_path == target.member_path;
        if same_restricted_pointer {
            return Ok(());
        }
        let current_frame = self.current_frame_ids.last().copied();
        let block_rank = |object: ObjectId| {
            self.active_block_scopes
                .iter()
                .filter(|scope| Some(scope.frame_id) == current_frame)
                .enumerate()
                .filter_map(|(rank, scope)| {
                    scope
                        .existing_objects
                        .as_ref()
                        .is_some_and(|existing| !existing.contains(&object))
                        .then_some(rank)
                })
                .last()
                .unwrap_or(0)
        };
        if block_rank(source.object) >= block_rank(target.object) {
            return Err(Diagnostic::ub(
                "assignment between restrict-qualified pointer objects whose associated blocks do not have the required ordering",
                span,
                Some("6.7.3.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn validate_simple_assignment_overlap(
        &self,
        target: &LValue,
        source: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let target_region = self.lvalue_access_region(target, objects, span)?;
        let source_region = self.lvalue_access_region(source, objects, span)?;
        if !Self::access_regions_overlap(target_region, source_region) {
            return Ok(());
        }
        let exact_overlap = target_region == source_region;
        let compatible_types = self
            .cross_unit_tagged_type_compatible(target.ty.unqualified(), source.ty.unqualified());
        if !exact_overlap || !compatible_types {
            return Err(Diagnostic::ub(
                "assignment reads its stored value from an overlapping object without exact overlap and compatible type",
                span,
                Some("6.5.16.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn validate_atomic_member_access(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(object) = self.lookup_object(objects, lvalue.object) else {
            return Ok(());
        };
        let mut ty = lvalue
            .designated_root_ty
            .as_deref()
            .unwrap_or(&object.ty)
            .clone();
        for member in lvalue.member_path.iter() {
            while let CType::Array(inner, _) = ty.unqualified() {
                ty = (**inner).clone();
            }
            if ty.top_level_qualifiers().is_atomic {
                return Err(Diagnostic::ub(
                    "access to a member of an atomic structure or union object",
                    span,
                    Some("6.5.2.3p5"),
                ));
            }
            let Some(next) = self.direct_member_by_storage_name(&ty, member) else {
                break;
            };
            ty = next.ty.clone();
        }
        Ok(())
    }

    fn is_assignment_target(&self, region: AccessRegion) -> bool {
        self.assignment_targets
            .iter()
            .any(|target| Self::access_regions_overlap(target.region, region))
    }

    fn access_regions_overlap(lhs: AccessRegion, rhs: AccessRegion) -> bool {
        lhs.object == rhs.object
            && lhs.bit_size != 0
            && rhs.bit_size != 0
            && lhs.bit_start < rhs.bit_start.saturating_add(rhs.bit_size)
            && rhs.bit_start < lhs.bit_start.saturating_add(lhs.bit_size)
    }

    fn access_region_contains(outer: AccessRegion, inner: AccessRegion) -> bool {
        outer.object == inner.object
            && inner.bit_start >= outer.bit_start
            && inner.bit_start.saturating_add(inner.bit_size)
                <= outer.bit_start.saturating_add(outer.bit_size)
    }

    pub(super) fn lvalue_bit_field_offset(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
    ) -> Option<usize> {
        lvalue.bit_field_width?;
        let object_ty = &self.lookup_object(objects, lvalue.object)?.ty;
        let mut ty = lvalue.designated_root_ty.as_deref().unwrap_or(object_ty);
        let mut bit_offset = None;
        for storage_name in lvalue.member_path.iter() {
            while let CType::Array(inner, _) = ty.unqualified() {
                ty = inner;
            }
            let member = self.direct_member_by_storage_name(ty, storage_name)?;
            bit_offset = Some(member.bit_offset as usize);
            ty = &member.ty;
        }
        bit_offset
    }

    pub(super) fn lvalue_access_region(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<AccessRegion, Diagnostic> {
        let (_, start, size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to access", span, Some("6.5.3.2"))
            })?;
        self.lvalue_access_region_from_range(lvalue, start, size, objects, span)
    }

    pub(super) fn lvalue_access_region_from_range(
        &self,
        lvalue: &LValue,
        start: usize,
        size: usize,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<AccessRegion, Diagnostic> {
        let mut bit_start = start.checked_mul(8).ok_or_else(|| {
            Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
        })?;
        let bit_size = if let Some(width) = lvalue.bit_field_width {
            bit_start = bit_start
                .checked_add(self.lvalue_bit_field_offset(lvalue, objects).unwrap_or(0))
                .ok_or_else(|| {
                    Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
                })?;
            width as usize
        } else {
            size.checked_mul(8).ok_or_else(|| {
                Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
            })?
        };
        Ok(AccessRegion {
            object: lvalue.object,
            bit_start,
            bit_size,
        })
    }

    pub(super) fn record_read(
        &mut self,
        region: AccessRegion,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !self.expr_state.track_unsequenced_accesses || self.in_host_library_runtime() {
            return Ok(());
        }
        let is_target = self.is_assignment_target(region);
        if let Some(write_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.write)
        {
            return Err(self.unsequenced_access_diag(
                "unsequenced read of an object after it was modified",
                span,
                write_span,
            ));
        }
        let access = self.expr_state.accesses.entry(region).or_default();
        if is_target {
            access.self_read.get_or_insert(span);
        } else {
            access.other_read.get_or_insert(span);
        }
        Ok(())
    }

    pub(super) fn record_write(
        &mut self,
        region: AccessRegion,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !self.expr_state.track_unsequenced_accesses || self.in_host_library_runtime() {
            return Ok(());
        }
        if let Some(write_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.write)
        {
            return Err(self.unsequenced_access_diag(
                "multiple unsequenced modifications of the same object",
                span,
                write_span,
            ));
        }
        if let Some(read_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.other_read)
        {
            return Err(self.unsequenced_access_diag(
                "modification of an object is unsequenced relative to another access of the same object",
                span,
                read_span,
            ));
        }
        let access = self.expr_state.accesses.entry(region).or_default();
        access.write = Some(span);
        Ok(())
    }

    pub(super) fn record_restrict_access(
        &mut self,
        lvalue: &LValue,
        effective_ty: &CType,
        is_write: bool,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if !self.restrict_tracking_required || self.in_host_library_runtime() {
            return Ok(());
        }
        let Some((object, start, size)) = self.lvalue_byte_range(lvalue, effective_ty, objects)
        else {
            return Ok(());
        };
        let Some(tracker) = self.restrict_trackers.last_mut() else {
            return Ok(());
        };
        let current_source = lvalue.restrict_source.clone();
        if is_write
            && current_source
                .as_ref()
                .is_some_and(|source| source.pointed_to_const)
        {
            return Err(Diagnostic::ub(
                "modification through a pointer based on a restrict-qualified pointer to const-qualified type",
                span,
                Some("6.7.3.1"),
            ));
        }
        if current_source.is_none() && tracker.source_objects.is_empty() {
            let entries = tracker.unrestricted_accesses.entry(object).or_default();
            if let Some(entry) = entries.get_mut(&(start, size)) {
                entry.saw_write |= is_write;
            } else {
                entries.insert(
                    (start, size),
                    RestrictAccess {
                        source: None,
                        start,
                        size,
                        saw_write: is_write,
                        span,
                    },
                );
            }
            return Ok(());
        }
        if let Some(source) = current_source.as_ref() {
            tracker.source_objects.insert(source.object);
        }
        let entries = tracker.accesses.entry(object).or_default();
        if let Some(unrestricted) = tracker.unrestricted_accesses.remove(&object) {
            entries.extend(unrestricted.into_values());
        }
        for entry in entries.iter() {
            let overlaps = start < entry.start.saturating_add(entry.size)
                && entry.start < start.saturating_add(size);
            let different_source = entry.source != current_source;
            let restrict_involved = entry.source.is_some() || current_source.is_some();
            if overlaps && different_source && restrict_involved && (is_write || entry.saw_write) {
                let previous = self.sources.snippet(entry.span);
                return Err(Diagnostic::ub(
                    "access to overlapping object through different restrict-qualified pointer bases",
                    span,
                    Some("6.7.3.1"),
                )
                .with_note(format!(
                    "conflicting prior access through a different base at {}:{}:{}",
                    previous.path.display(),
                    previous.line_number,
                    previous.column
                )));
            }
        }
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry.source == current_source && entry.start == start && entry.size == size
        }) {
            entry.saw_write |= is_write;
        } else {
            entries.push(RestrictAccess {
                source: current_source,
                start,
                size,
                saw_write: is_write,
                span,
            });
        }
        Ok(())
    }

    pub(super) fn forget_retired_restrict_sources(&mut self, retired_ids: &[ObjectId]) {
        if !self.restrict_tracking_required || retired_ids.is_empty() {
            return;
        }
        let retired_set =
            (retired_ids.len() > 8).then(|| retired_ids.iter().copied().collect::<HashSet<_>>());
        let contains_retired = |object: ObjectId| match retired_ids {
            [only] => *only == object,
            [first, second] => *first == object || *second == object,
            ids if ids.len() <= 8 => ids.contains(&object),
            _ => retired_set
                .as_ref()
                .is_some_and(|retired| retired.contains(&object)),
        };
        for tracker in &mut self.restrict_trackers {
            for &retired in retired_ids {
                tracker.accesses.remove(&retired);
                tracker.unrestricted_accesses.remove(&retired);
            }
            let retired_source = retired_ids
                .iter()
                .copied()
                .any(|retired| tracker.source_objects.remove(&retired));
            if !retired_source {
                continue;
            }
            tracker.accesses.retain(|_, entries| {
                entries.retain(|entry| {
                    entry
                        .source
                        .as_ref()
                        .is_none_or(|source| !contains_retired(source.object))
                });
                !entries.is_empty()
            });
        }
    }

    fn unsequenced_access_diag(
        &self,
        message: &'static str,
        current_span: Span,
        previous_span: Span,
    ) -> Diagnostic {
        let previous = self.sources.snippet(previous_span);
        Diagnostic::ub(message, current_span, Some("6.5p2")).with_note(format!(
            "previous unsequenced access at {}:{}:{}",
            previous.path.display(),
            previous.line_number,
            previous.column
        ))
    }
}
