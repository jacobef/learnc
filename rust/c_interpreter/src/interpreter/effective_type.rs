//! Effective types, aliasing rules, and copied object representations.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn type_contains_union(&self, ty: &CType) -> bool {
        fn visit(interpreter: &Interpreter<'_>, ty: &CType, seen: &mut HashSet<usize>) -> bool {
            match ty.unqualified() {
                CType::Union(_, _) => true,
                CType::Array(inner, _) => visit(interpreter, inner, seen),
                CType::Struct(id, _) => {
                    if !seen.insert(*id) {
                        return false;
                    }
                    let contains_union =
                        interpreter.program.records.get(id).is_some_and(|record| {
                            record
                                .members
                                .iter()
                                .any(|member| visit(interpreter, &member.ty, seen))
                        });
                    seen.remove(id);
                    contains_union
                }
                _ => false,
            }
        }

        visit(self, ty, &mut HashSet::default())
    }

    fn effective_type_alias_allowed(&self, access_ty: &CType, effective_ty: &CType) -> bool {
        fn visit(
            interpreter: &Interpreter<'_>,
            access_ty: &CType,
            effective_ty: &CType,
            seen: &mut HashSet<usize>,
        ) -> bool {
            if interpreter.cross_unit_tagged_type_compatible(
                access_ty.unqualified(),
                effective_ty.unqualified(),
            ) || Interpreter::corresponding_signed_unsigned_types(access_ty, effective_ty)
            {
                return true;
            }
            let (CType::Struct(id, _) | CType::Union(id, _)) = access_ty.unqualified() else {
                return false;
            };
            if !seen.insert(*id) {
                return false;
            }
            let contains = interpreter.record_type(access_ty).is_some_and(|record| {
                record.members.iter().any(|member| {
                    member.bit_width != Some(0)
                        && visit(interpreter, &member.ty, effective_ty, seen)
                })
            });
            seen.remove(id);
            contains
        }

        access_ty.is_character() || visit(self, access_ty, effective_ty, &mut HashSet::default())
    }

    fn type_region_allows_effective_access(
        &self,
        parent_ty: &CType,
        parent_start: usize,
        access_start: usize,
        access_size: usize,
        access_ty: &CType,
    ) -> bool {
        let Some(parent_size) = self.type_size_of(parent_ty) else {
            return false;
        };
        let Some(parent_end) = parent_start.checked_add(parent_size) else {
            return false;
        };
        let Some(access_end) = access_start.checked_add(access_size) else {
            return false;
        };
        if access_start < parent_start || access_end > parent_end {
            return false;
        }
        if self.type_region_is_padding(parent_ty, parent_start, access_start, access_size) {
            return true;
        }
        if access_start == parent_start
            && access_size == parent_size
            && self.effective_type_alias_allowed(access_ty, parent_ty)
        {
            return true;
        }
        match parent_ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return false;
                };
                if stride == 0 {
                    return false;
                }
                let first = (access_start - parent_start) / stride;
                if first >= *len {
                    return false;
                }
                let child_start = parent_start + first * stride;
                self.type_region_allows_effective_access(
                    inner,
                    child_start,
                    access_start,
                    access_size,
                    access_ty,
                )
            }
            CType::Struct(_, _) | CType::Union(_, _) => {
                self.record_type(parent_ty).is_some_and(|record| {
                    record.members.iter().any(|member| {
                        if member.bit_width == Some(0) {
                            return false;
                        }
                        // A scalar member does not make two enclosing records
                        // compatible, even when all three have the same size.
                        let mut member_ty = &member.ty;
                        while let CType::Array(inner, _) = member_ty.unqualified() {
                            member_ty = inner;
                        }
                        if matches!(
                            access_ty.unqualified(),
                            CType::Struct(..) | CType::Union(..)
                        ) && !matches!(
                            member_ty.unqualified(),
                            CType::Struct(..) | CType::Union(..)
                        ) {
                            return false;
                        }
                        let member_start = if record.kind == crate::types::RecordKind::Union {
                            parent_start
                        } else {
                            parent_start.saturating_add(member.offset)
                        };
                        self.type_region_allows_effective_access(
                            &member.ty,
                            member_start,
                            access_start,
                            access_size,
                            access_ty,
                        )
                    })
                })
            }
            _ => false,
        }
    }

    fn type_region_is_padding(&self, ty: &CType, base: usize, start: usize, size: usize) -> bool {
        if size == 0 {
            return true;
        }
        let Some(end) = start.checked_add(size) else {
            return false;
        };
        let Some(ty_size) = self.type_size_of(ty) else {
            return false;
        };
        let Some(ty_end) = base.checked_add(ty_size) else {
            return false;
        };
        if start < base || end > ty_end {
            return false;
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return false;
                };
                if stride == 0 {
                    return false;
                }
                (0..*len).all(|index| {
                    let child_start = base + index * stride;
                    let child_end = child_start + stride;
                    if start >= child_end || child_start >= end {
                        true
                    } else {
                        self.type_region_is_padding(
                            inner,
                            child_start,
                            start.max(child_start),
                            end.min(child_end) - start.max(child_start),
                        )
                    }
                })
            }
            CType::Struct(_, _) | CType::Union(_, _) => {
                self.record_type(ty).is_some_and(|record| {
                    record.members.iter().all(|member| {
                        if member.bit_width == Some(0) {
                            return true;
                        }
                        let Some(member_size) = self.type_size_of(&member.ty) else {
                            return true;
                        };
                        let member_start = if record.kind == crate::types::RecordKind::Union {
                            base
                        } else {
                            base.saturating_add(member.offset)
                        };
                        let member_end = member_start.saturating_add(member_size);
                        if start >= member_end || member_start >= end {
                            return true;
                        }
                        if member.bit_width.is_some() {
                            return false;
                        }
                        let overlap_start = start.max(member_start);
                        let overlap_end = end.min(member_end);
                        self.type_region_is_padding(
                            &member.ty,
                            member_start,
                            overlap_start,
                            overlap_end - overlap_start,
                        )
                    })
                })
            }
            _ => false,
        }
    }

    fn effective_region_allows_access(
        &self,
        region: &EffectiveTypeRegion,
        access_start: usize,
        access_size: usize,
        access_ty: &CType,
    ) -> bool {
        let Some(region_end) = region.start.checked_add(region.size) else {
            return false;
        };
        let Some(access_end) = access_start.checked_add(access_size) else {
            return false;
        };
        if access_start >= region.start && access_end <= region_end {
            return self.type_region_allows_effective_access(
                &region.ty,
                region.start,
                access_start,
                access_size,
                access_ty,
            );
        }
        if region.start >= access_start && region_end <= access_end {
            if let Some(element_ty) = region.coalesced_element_type.as_ref()
                && let Some(element_size) = self.type_size_of(element_ty)
                && element_size != 0
                && region.size.is_multiple_of(element_size)
                && (0..region.size / element_size).all(|index| {
                    self.type_region_allows_effective_access(
                        access_ty,
                        access_start,
                        region.start + index * element_size,
                        element_size,
                        element_ty,
                    )
                })
            {
                return true;
            }
            return self.type_region_allows_effective_access(
                access_ty,
                access_start,
                region.start,
                region.size,
                &region.ty,
            );
        }
        if let Some(element_ty) = region.coalesced_element_type.as_ref()
            && let Some(element_size) = self.type_size_of(element_ty)
            && element_size != 0
            && region.size.is_multiple_of(element_size)
        {
            return (0..region.size / element_size)
                .filter_map(|index| {
                    let element_start = region.start + index * element_size;
                    let element_end = element_start + element_size;
                    (access_start < element_end && element_start < access_end)
                        .then_some((element_start, element_end))
                })
                .all(|(element_start, element_end)| {
                    if element_start >= access_start && element_end <= access_end {
                        self.type_region_allows_effective_access(
                            access_ty,
                            access_start,
                            element_start,
                            element_size,
                            element_ty,
                        )
                    } else if access_start >= element_start && access_end <= element_end {
                        self.type_region_allows_effective_access(
                            element_ty,
                            element_start,
                            access_start,
                            access_size,
                            access_ty,
                        )
                    } else {
                        false
                    }
                });
        }
        false
    }

    pub(super) fn check_dynamic_effective_type_read(
        &self,
        object: &ObjectState,
        start: usize,
        size: usize,
        access_ty: &CType,
        access_root_ty: &CType,
        member_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if object.storage_duration != StorageDuration::Dynamic
            || access_ty.is_character()
            || size == 0
        {
            return Ok(());
        }
        let end = start.saturating_add(size);
        let mut member_containers = Vec::new();
        let mut container_ty = access_root_ty;
        for (index, member_name) in member_path.iter().enumerate() {
            while let CType::Array(inner, _) = container_ty.unqualified() {
                container_ty = inner;
            }
            if let Some(container_start) = self
                .member_path_offset(container_ty, &member_path[index..])
                .and_then(|offset| start.checked_sub(offset))
                && let Some(container_size) = self.type_size_of(container_ty)
                // Flexible array elements extend beyond their enclosing header;
                // validate the element's own record type instead.
                && end <= container_start.saturating_add(container_size)
            {
                member_containers.push((container_start, container_size, container_ty));
            }
            let Some(member) = self.direct_member_by_storage_name(container_ty, member_name) else {
                break;
            };
            container_ty = &member.ty;
        }
        let first = object
            .effective_types
            .partition_point(|region| region.start.saturating_add(region.size) <= start);
        for region in object.effective_types[first..]
            .iter()
            .take_while(|region| region.start < end)
        {
            // Check only regions touched by the member read. Other members may
            // have been overwritten with new effective types independently.
            let mut stored_ty = region.coalesced_element_type.as_ref().unwrap_or(&region.ty);
            while let CType::Array(inner, _) = stored_ty.unqualified() {
                stored_ty = inner;
            }
            // Scalar stores establish member types independently; only an
            // existing record type constrains the enclosing member access.
            let incompatible_container = if matches!(
                stored_ty.unqualified(),
                CType::Struct(..) | CType::Union(..)
            ) {
                member_containers.iter().copied().find(
                    |&(container_start, container_size, container_ty)| {
                        !self.effective_region_allows_access(
                            region,
                            container_start,
                            container_size,
                            container_ty,
                        )
                    },
                )
            } else {
                None
            };
            if (incompatible_container.is_some()
                || !self.effective_region_allows_access(region, start, size, access_ty))
                && !self.union_member_path_allows_effective_access(
                    access_root_ty,
                    member_path,
                    region.coalesced_element_type.as_ref().unwrap_or(&region.ty),
                    access_ty,
                )
            {
                let access_ty = incompatible_container.map_or(access_ty, |(_, _, ty)| ty);
                return Err(Diagnostic::ub(
                    format!(
                        "access through an lvalue of type {} is incompatible with the allocated object's effective type {}",
                        access_ty, region.ty
                    ),
                    span,
                    Some("6.5p6-7"),
                ));
            }
        }
        Ok(())
    }

    fn union_member_path_allows_effective_access(
        &self,
        root_ty: &CType,
        member_path: &[String],
        stored_ty: &CType,
        access_ty: &CType,
    ) -> bool {
        let mut current_ty = root_ty;
        for (path_index, member_name) in member_path.iter().enumerate() {
            while let CType::Array(inner, _) = current_ty.unqualified() {
                current_ty = inner;
            }
            let Some(record) = self.record_type(current_ty) else {
                return false;
            };
            let Some(selected) = record
                .members
                .iter()
                .find(|member| member.storage_name == *member_name)
            else {
                return false;
            };
            if record.kind == crate::types::RecordKind::Union {
                let selected_access_ty = self
                    .storage_path_type(&selected.ty, &member_path[path_index + 1..])
                    .unwrap_or_else(|| selected.ty.clone());
                if self.compatible_object_layout_types(&selected_access_ty, access_ty)
                    && record
                        .members
                        .iter()
                        .any(|member| self.effective_type_alias_allowed(&member.ty, stored_ty))
                {
                    return true;
                }
            }
            current_ty = &selected.ty;
        }
        false
    }

    fn split_effective_region_around_write(
        &self,
        region: &EffectiveTypeRegion,
        write_start: usize,
        write_size: usize,
        out: &mut Vec<EffectiveTypeRegion>,
    ) {
        let region_end = region.start.saturating_add(region.size);
        let write_end = write_start.saturating_add(write_size);
        if write_start >= region_end || region.start >= write_end {
            out.push(region.clone());
            return;
        }
        match region.ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return;
                };
                if stride == 0 || *len == 0 {
                    return;
                }
                let overlap_start = write_start.max(region.start) - region.start;
                let overlap_end = write_end.min(region_end) - region.start;
                let first = (overlap_start / stride).min(*len);
                let last = overlap_end
                    .saturating_add(stride - 1)
                    .checked_div(stride)
                    .unwrap_or(*len)
                    .min(*len);
                if first != 0 {
                    out.push(EffectiveTypeRegion {
                        start: region.start,
                        size: first * stride,
                        ty: CType::array_of((**inner).clone(), first),
                        coalesced_element_type: region.coalesced_element_type.clone(),
                    });
                }
                for index in first..last {
                    let child = EffectiveTypeRegion {
                        start: region.start + index * stride,
                        size: stride,
                        ty: (**inner).clone(),
                        coalesced_element_type: None,
                    };
                    let child_end = child.start + child.size;
                    if !(write_start <= child.start && write_end >= child_end) {
                        self.split_effective_region_around_write(
                            &child,
                            write_start,
                            write_size,
                            out,
                        );
                    }
                }
                if last < *len {
                    out.push(EffectiveTypeRegion {
                        start: region.start + last * stride,
                        size: (*len - last) * stride,
                        ty: CType::array_of((**inner).clone(), *len - last),
                        coalesced_element_type: region.coalesced_element_type.clone(),
                    });
                }
            }
            CType::Struct(_, _) => {
                if let Some(record) = self.record_type(&region.ty) {
                    for member in &record.members {
                        if member.bit_width.is_some() {
                            continue;
                        }
                        let Some(member_size) = self.type_size_of(&member.ty) else {
                            continue;
                        };
                        self.split_effective_region_around_write(
                            &EffectiveTypeRegion {
                                start: region.start.saturating_add(member.offset),
                                size: member_size,
                                ty: member.ty.clone(),
                                coalesced_element_type: None,
                            },
                            write_start,
                            write_size,
                            out,
                        );
                    }
                }
            }
            CType::Union(_, _) => {}
            _ => {}
        }
    }

    fn coalesce_effective_type_regions(&self, regions: &mut Vec<EffectiveTypeRegion>) {
        if !regions.is_sorted_by_key(|region| region.start) {
            regions.sort_by_key(|region| region.start);
        }
        let mut coalesced: Vec<EffectiveTypeRegion> = Vec::with_capacity(regions.len());
        for region in regions.drain(..) {
            let Some(previous) = coalesced.last_mut() else {
                coalesced.push(region);
                continue;
            };
            if previous.start == region.start
                && previous.size == region.size
                && previous.ty == region.ty
            {
                continue;
            }
            let Some(previous_end) = previous.start.checked_add(previous.size) else {
                coalesced.push(region);
                continue;
            };
            if previous_end != region.start {
                coalesced.push(region);
                continue;
            }
            let previous_element = previous
                .coalesced_element_type
                .as_ref()
                .unwrap_or(&previous.ty);
            let region_element = region.coalesced_element_type.as_ref().unwrap_or(&region.ty);
            if previous_element != region_element {
                coalesced.push(region);
                continue;
            }
            let Some(element_size) = self.type_size_of(previous_element) else {
                coalesced.push(region);
                continue;
            };
            if element_size == 0
                || previous.size % element_size != 0
                || region.size % element_size != 0
            {
                coalesced.push(region);
                continue;
            }
            let Some(size) = previous.size.checked_add(region.size) else {
                coalesced.push(region);
                continue;
            };
            let count = size / element_size;
            let previous_element = previous_element.clone();
            previous.size = size;
            previous.ty = CType::array_of(previous_element.clone(), count);
            previous.coalesced_element_type = Some(previous_element);
        }
        *regions = coalesced;
    }

    fn merge_effective_type_regions_at(
        &self,
        regions: &mut Vec<EffectiveTypeRegion>,
        index: usize,
    ) -> bool {
        if index + 1 >= regions.len() {
            return false;
        }
        let merge = {
            let (left, right) = regions.split_at_mut(index + 1);
            let previous = &mut left[index];
            let region = &right[0];
            if previous.start == region.start
                && previous.size == region.size
                && previous.ty == region.ty
            {
                true
            } else if previous.start.checked_add(previous.size) == Some(region.start) {
                let previous_element = previous
                    .coalesced_element_type
                    .as_ref()
                    .unwrap_or(&previous.ty);
                let region_element = region.coalesced_element_type.as_ref().unwrap_or(&region.ty);
                if previous_element != region_element {
                    false
                } else if let Some(element_size) = self.type_size_of(previous_element) {
                    if element_size == 0
                        || previous.size % element_size != 0
                        || region.size % element_size != 0
                    {
                        false
                    } else if let Some(size) = previous.size.checked_add(region.size) {
                        let element = previous_element.clone();
                        previous.size = size;
                        previous.ty = CType::array_of(element.clone(), size / element_size);
                        previous.coalesced_element_type = Some(element);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };
        if merge {
            regions.remove(index + 1);
        }
        merge
    }

    fn update_dynamic_effective_types_after_store(
        &self,
        regions: &mut Vec<EffectiveTypeRegion>,
        start: usize,
        size: usize,
        access_ty: &CType,
    ) {
        if size == 0 {
            return;
        }
        let end = start.saturating_add(size);
        let first =
            regions.partition_point(|region| region.start.saturating_add(region.size) <= start);
        let last = first + regions[first..].partition_point(|region| region.start < end);
        let replaced = regions.drain(first..last).collect::<Vec<_>>();
        let mut replacement = Vec::with_capacity(replaced.len().saturating_add(1));
        for region in &replaced {
            self.split_effective_region_around_write(region, start, size, &mut replacement);
        }
        if !access_ty.is_character() {
            replacement.push(EffectiveTypeRegion {
                start,
                size,
                ty: access_ty.unqualified().clone(),
                coalesced_element_type: None,
            });
        }
        self.coalesce_effective_type_regions(&mut replacement);
        let inserted = replacement.len();
        regions.splice(first..first, replacement);

        let mut index = first.saturating_sub(1);
        let mut pairs_to_check = inserted.saturating_add(2);
        while index + 1 < regions.len() && pairs_to_check != 0 {
            if !self.merge_effective_type_regions_at(regions, index) {
                index += 1;
            }
            pairs_to_check -= 1;
        }
    }

    pub(super) fn update_object_effective_types_after_store(
        &mut self,
        objects: &mut ObjectFrames,
        object_id: ObjectId,
        start: usize,
        size: usize,
        access_ty: &CType,
    ) {
        let Some(object) = self.lookup_active_object_mut(objects, object_id) else {
            return;
        };
        if object.storage_duration != StorageDuration::Dynamic || size == 0 {
            return;
        }
        let mut regions = std::mem::take(&mut object.effective_types);
        self.update_dynamic_effective_types_after_store(&mut regions, start, size, access_ty);
        if let Some(object) = self.lookup_active_object_mut(objects, object_id) {
            object.effective_types = regions;
        }
    }

    fn collect_exact_subobject_types(
        &self,
        ty: &CType,
        base: usize,
        start: usize,
        size: usize,
        out: &mut Vec<CType>,
    ) {
        let Some(ty_size) = self.type_size_of(ty) else {
            return;
        };
        let Some(end) = start.checked_add(size) else {
            return;
        };
        let Some(ty_end) = base.checked_add(ty_size) else {
            return;
        };
        if start < base || end > ty_end {
            return;
        }
        if start == base && size == ty_size {
            out.push(ty.clone());
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return;
                };
                if stride == 0 {
                    return;
                }
                let index = (start - base) / stride;
                if index < *len {
                    self.collect_exact_subobject_types(
                        inner,
                        base + index * stride,
                        start,
                        size,
                        out,
                    );
                }
            }
            CType::Struct(_, _) => {
                if let Some(record) = self.record_type(ty) {
                    for member in &record.members {
                        if member.bit_width.is_none() {
                            self.collect_exact_subobject_types(
                                &member.ty,
                                base.saturating_add(member.offset),
                                start,
                                size,
                                out,
                            );
                        }
                    }
                }
            }
            CType::Union(_, _) => {
                if let Some(record) = self.record_type(ty) {
                    for member in &record.members {
                        if member.bit_width.is_none() {
                            self.collect_exact_subobject_types(&member.ty, base, start, size, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn choose_copied_effective_type(
        &self,
        ty: &CType,
        base: usize,
        start: usize,
        size: usize,
        source_pointee: Option<&CType>,
    ) -> Option<CType> {
        let mut candidates = Vec::new();
        self.collect_exact_subobject_types(ty, base, start, size, &mut candidates);
        if let Some(source_pointee) = source_pointee {
            if !matches!(source_pointee.unqualified(), CType::Void)
                && !source_pointee.is_character()
                && let Some(candidate) = candidates.iter().find(|candidate| {
                    self.cross_unit_tagged_type_compatible(
                        candidate.unqualified(),
                        source_pointee.unqualified(),
                    )
                })
            {
                return Some(candidate.unqualified().clone());
            }
        }
        candidates
            .into_iter()
            .next()
            .map(|candidate| candidate.unqualified().clone())
    }

    fn collect_copied_subobject_regions(
        &self,
        ty: &CType,
        base: usize,
        start: usize,
        end: usize,
        out: &mut Vec<EffectiveTypeRegion>,
    ) {
        let Some(size) = self.type_size_of(ty) else {
            return;
        };
        let Some(ty_end) = base.checked_add(size) else {
            return;
        };
        if size == 0 || ty_end <= start || base >= end {
            return;
        }
        if start <= base && ty_end <= end {
            out.push(EffectiveTypeRegion {
                start: base,
                size,
                ty: ty.unqualified().clone(),
                coalesced_element_type: None,
            });
            return;
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner).filter(|&size| size != 0) else {
                    return;
                };
                let first = start.saturating_sub(base) / stride;
                let last = end
                    .min(ty_end)
                    .saturating_sub(base)
                    .div_ceil(stride)
                    .min(*len);
                for index in first..last {
                    self.collect_copied_subobject_regions(
                        inner,
                        base + index * stride,
                        start,
                        end,
                        out,
                    );
                }
            }
            CType::Struct(_, _) => {
                if let Some(record) = self.record_type(ty) {
                    for member in &record.members {
                        if member.bit_width.is_none() {
                            self.collect_copied_subobject_regions(
                                &member.ty,
                                base.saturating_add(member.offset),
                                start,
                                end,
                                out,
                            );
                        }
                    }
                }
            }
            // A partial union representation cannot identify which overlapping
            // member supplied it. Exact copies are handled by the caller.
            _ => {}
        }
    }

    pub(super) fn copied_effective_type_regions(
        &self,
        source: &ObjectState,
        start: usize,
        size: usize,
        source_pointee: Option<&CType>,
    ) -> Vec<EffectiveTypeRegion> {
        if size == 0 {
            return Vec::new();
        }
        if source.storage_duration != StorageDuration::Dynamic {
            if let Some(ty) =
                self.choose_copied_effective_type(&source.ty, 0, start, size, source_pointee)
            {
                return vec![EffectiveTypeRegion {
                    start,
                    size,
                    ty,
                    coalesced_element_type: None,
                }];
            }
            let mut copied = Vec::new();
            // DR 219: even a copy through char* preserves the declared types of
            // complete subobjects contained in the copied byte range.
            self.collect_copied_subobject_regions(
                &source.ty,
                0,
                start,
                start.saturating_add(size),
                &mut copied,
            );
            self.coalesce_effective_type_regions(&mut copied);
            return copied;
        }

        let end = start.saturating_add(size);
        let mut copied = Vec::new();
        for region in &source.effective_types {
            let region_end = region.start.saturating_add(region.size);
            if region_end <= start || region.start >= end {
                continue;
            }
            if start <= region.start && region_end <= end {
                copied.push(region.clone());
                continue;
            }
            let copy_start = start.max(region.start);
            let copy_end = end.min(region_end);
            if let Some(ty) = self.choose_copied_effective_type(
                &region.ty,
                region.start,
                copy_start,
                copy_end - copy_start,
                source_pointee,
            ) {
                copied.push(EffectiveTypeRegion {
                    start: copy_start,
                    size: copy_end - copy_start,
                    ty,
                    coalesced_element_type: None,
                });
            } else {
                self.collect_copied_subobject_regions(
                    &region.ty,
                    region.start,
                    copy_start,
                    copy_end,
                    &mut copied,
                );
            }
        }
        self.coalesce_effective_type_regions(&mut copied);
        copied
    }

    pub(super) fn dynamic_effective_types_after_copy(
        &self,
        object: &ObjectState,
        dest_start: usize,
        size: usize,
        source_start: usize,
        source_regions: Vec<EffectiveTypeRegion>,
    ) -> Option<Vec<EffectiveTypeRegion>> {
        if object.storage_duration != StorageDuration::Dynamic || size == 0 {
            return None;
        }
        let mut updated = Vec::with_capacity(
            object
                .effective_types
                .len()
                .saturating_add(source_regions.len()),
        );
        for region in &object.effective_types {
            self.split_effective_region_around_write(region, dest_start, size, &mut updated);
        }
        for region in source_regions.into_iter().filter_map(|region| {
            let relative = region.start.checked_sub(source_start)?;
            Some(EffectiveTypeRegion {
                start: dest_start.checked_add(relative)?,
                size: region.size,
                ty: region.ty,
                coalesced_element_type: region.coalesced_element_type,
            })
        }) {
            let insertion = updated.partition_point(|existing| existing.start <= region.start);
            updated.insert(insertion, region);
        }
        self.coalesce_effective_type_regions(&mut updated);
        Some(updated)
    }
}
