//! Exact, bounded snapshots for the memory a loop's exit can depend on.

use super::*;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum LoopPathPart {
    Index(usize),
    Member(String),
}

pub(super) type LoopReadPaths = Option<HashSet<Vec<LoopPathPart>>>;

pub(super) fn loop_read_path(expr: &Expr) -> Option<(&str, Vec<LoopPathPart>)> {
    match expr {
        Expr::Variable(name, _) => Some((name, Vec::new())),
        Expr::Subscript { base, index, .. } => {
            let Expr::Number(number, _) = index.as_ref() else {
                return None;
            };
            let NumberValue::Integer(index) = number.value else {
                return None;
            };
            let (name, mut path) = loop_read_path(base)?;
            path.push(LoopPathPart::Index(usize::try_from(index).ok()?));
            Some((name, path))
        }
        Expr::Member { base, member, .. } => {
            let (name, mut path) = loop_read_path(base)?;
            path.push(LoopPathPart::Member(member.clone()));
            Some((name, path))
        }
        _ => None,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum LoopValue {
    UncreatedStatic,
    Stdin(LoopInputState),
    Object(ObjectId),
    Integer(CIntValue),
    Floating(u64),
    Complex(u64, u64),
    Pointer(Rc<PointerValue>),
    PointerSlot(usize, usize),
    Array(usize),
    Record(usize),
    Union(Option<StoredMemberName>),
    Bytes(Vec<ByteCell>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct LoopInputState {
    position: usize,
    pushback: Vec<u8>,
    orientation: i8,
    eof: bool,
    error: bool,
    input_closed: bool,
    last_operation: Option<StreamLastOperation>,
    last_input_hit_eof: bool,
    operation_performed: bool,
}

#[derive(Default)]
pub(super) struct LoopSnapshot {
    pub values: Vec<LoopValue>,
    visited: HashSet<ObjectId>,
    cells: usize,
}

impl<'a> Interpreter<'a> {
    pub(super) fn snapshot_loop_paths(
        &self,
        id: ObjectId,
        paths: &[Vec<LoopPathPart>],
        objects: &ObjectFrames,
        snapshot: &mut LoopSnapshot,
    ) -> Option<()> {
        let object = self.lookup_object(objects, id)?;
        if !object.alive
            || object.has_volatile_subobject
            || object.ty.top_level_qualifiers().is_atomic
        {
            return None;
        }
        // Do not mark a partially visited root as fully visited: a pointer in
        // another dependency may require its entire representation as well.
        snapshot.values.push(LoopValue::Object(id));
        if paths.is_empty() {
            // Input destinations use an array's fixed address or a pointer's value,
            // without reading the bytes that the input operation will overwrite.
            if matches!(object.ty.unqualified(), CType::Array(_, _)) {
                return Some(());
            }
            return match &object.value {
                StoredValue::Scalar(value) if !value.indeterminate && !value.missing_return => {
                    let ValueData::Pointer(pointer) = &value.data else {
                        return None;
                    };
                    snapshot.values.push(LoopValue::Pointer(pointer.clone()));
                    Some(())
                }
                _ => None,
            };
        }
        for path in paths {
            if let StoredValue::ObjectRepresentation(bytes) = &object.value {
                let Some((start, size)) = self.loop_path_scalar_bytes(&object.ty, path) else {
                    return self.snapshot_loop_object(id, objects, snapshot);
                };
                self.snapshot_loop_bytes(bytes.get(start..start.checked_add(size)?)?, snapshot)?;
                continue;
            }
            let mut value = &object.value;
            for part in path {
                value = match (part, value) {
                    (LoopPathPart::Index(index), StoredValue::Array(values)) => {
                        values.get(*index)?
                    }
                    (LoopPathPart::Member(name), StoredValue::Record(members)) => {
                        &members
                            .iter()
                            .find(|(member, _)| member.as_ref() == name)?
                            .1
                    }
                    // Pointer indexing, unions, and raw object representations still
                    // require the full conservative snapshot.
                    _ => return self.snapshot_loop_object(id, objects, snapshot),
                };
            }
            self.snapshot_loop_value(value, objects, snapshot)?;
        }
        Some(())
    }

    pub(super) fn snapshot_loop_stdin(&self) -> Option<LoopValue> {
        let id = self.host_capture_streams.get(&CaptureStream::Stdin)?;
        let input = self.host_streams.get(id)?;
        // With no live host input and no stream mutation calls in the analyzed loop,
        // the captured input bytes are fixed. Position, pushback, and flags completely
        // describe how the next scanf/getchar will behave.
        if !input.open
            || input.backing != StreamBacking::Capture(CaptureStream::Stdin)
            || input.mode.kind != HostStreamModeKind::Input
            || input.pushback.len() > 512
        {
            return None;
        }
        Some(LoopValue::Stdin(LoopInputState {
            position: input.position,
            pushback: input.pushback.clone(),
            orientation: input.orientation,
            eof: input.eof,
            error: input.error,
            input_closed: input.input_closed,
            last_operation: input.last_operation,
            last_input_hit_eof: input.last_input_hit_eof,
            operation_performed: input.operation_performed,
        }))
    }

    pub(super) fn snapshot_loop_object(
        &self,
        id: ObjectId,
        objects: &ObjectFrames,
        snapshot: &mut LoopSnapshot,
    ) -> Option<()> {
        snapshot.values.push(LoopValue::Object(id));
        if !snapshot.visited.insert(id) {
            return Some(());
        }
        if snapshot.visited.len() > 32 {
            return None;
        }
        let object = self.lookup_object(objects, id)?;
        if !object.alive
            || !object.initialized
            || object.has_volatile_subobject
            || object.ty.top_level_qualifiers().is_atomic
        {
            return None;
        }
        self.snapshot_loop_value(&object.value, objects, snapshot)?;
        // Raw bytes alone omit pointer provenance and the pointee's changing state.
        // In particular a malloc-backed struct may hold the loop's only reference
        // to a counter. Preserve slot locations and follow every stored object pointer.
        for ((start, size), pointer) in &object.pointer_slots {
            snapshot.cells += 1;
            if snapshot.cells > 512 {
                return None;
            }
            let StoredPointerValue::Object(pointer) = pointer else {
                return None;
            };
            snapshot.values.push(LoopValue::PointerSlot(*start, *size));
            snapshot
                .values
                .push(LoopValue::Pointer(Rc::new(pointer.clone())));
            if let Some(target) = pointer.object {
                self.snapshot_loop_object(target, objects, snapshot)?;
            }
        }
        Some(())
    }

    fn loop_path_scalar_bytes(&self, ty: &CType, path: &[LoopPathPart]) -> Option<(usize, usize)> {
        let mut ty = ty.clone();
        let mut offset = 0usize;
        for part in path {
            match (part, ty.unqualified()) {
                (LoopPathPart::Index(index), CType::Array(inner, len)) if index < len => {
                    offset = offset.checked_add(index.checked_mul(self.type_size_of(inner)?)?)?;
                    ty = inner.as_ref().clone();
                }
                (LoopPathPart::Member(name), CType::Struct(_, _)) => {
                    let member = self
                        .record_type(&ty)?
                        .members
                        .iter()
                        .find(|member| member.storage_name == *name)?;
                    if member.bit_width.is_some() {
                        return None;
                    }
                    offset = offset.checked_add(member.offset)?;
                    ty = member.ty.clone();
                }
                _ => return None,
            }
        }
        // Pointer-containing subobjects need the full pointer-slot traversal.
        if !ty.is_integer() && !ty.is_floating() && !ty.is_complex() {
            return None;
        }
        Some((offset, self.type_size_of(&ty)?))
    }

    fn snapshot_loop_value(
        &self,
        stored: &StoredValue,
        objects: &ObjectFrames,
        snapshot: &mut LoopSnapshot,
    ) -> Option<()> {
        snapshot.cells += 1;
        if snapshot.cells > 512 {
            return None;
        }
        match stored {
            StoredValue::Indeterminate => return None,
            StoredValue::Scalar(value) => {
                if value.indeterminate
                    || value.missing_return
                    || value.ty.is_volatile_qualified()
                    || value.ty.top_level_qualifiers().is_atomic
                {
                    return None;
                }
                let entry = match &value.data {
                    ValueData::Int(value) => LoopValue::Integer(*value),
                    ValueData::Float(value) => LoopValue::Floating(value.to_bits()),
                    ValueData::Complex(value) => {
                        LoopValue::Complex(value.real.to_bits(), value.imag.to_bits())
                    }
                    ValueData::Pointer(pointer) => {
                        snapshot.values.push(LoopValue::Pointer(pointer.clone()));
                        if let Some(id) = pointer.object {
                            self.snapshot_loop_object(id, objects, snapshot)?;
                        }
                        return Some(());
                    }
                    ValueData::Aggregate(stored) => {
                        return self.snapshot_loop_value(stored, objects, snapshot);
                    }
                    _ => return None,
                };
                snapshot.values.push(entry);
            }
            StoredValue::Array(values) => {
                snapshot.values.push(LoopValue::Array(values.len()));
                for value in values {
                    self.snapshot_loop_value(value, objects, snapshot)?;
                }
            }
            StoredValue::Record(members) => {
                snapshot.values.push(LoopValue::Record(members.len()));
                for (_, value) in members {
                    self.snapshot_loop_value(value, objects, snapshot)?;
                }
            }
            StoredValue::Union {
                active_member,
                members,
                bytes,
            } => {
                snapshot
                    .values
                    .push(LoopValue::Union(active_member.clone()));
                let (_, active) = members
                    .iter()
                    .find(|(name, _)| Some(name) == active_member.as_ref())?;
                self.snapshot_loop_value(active, objects, snapshot)?;
                self.snapshot_loop_bytes(bytes, snapshot)?;
            }
            StoredValue::ObjectRepresentation(bytes) => {
                self.snapshot_loop_bytes(bytes, snapshot)?
            }
        }
        Some(())
    }

    fn snapshot_loop_bytes(&self, bytes: &[ByteCell], snapshot: &mut LoopSnapshot) -> Option<()> {
        snapshot.cells = snapshot.cells.checked_add(bytes.len())?;
        if snapshot.cells > 512 || bytes.iter().any(|byte| !matches!(byte, ByteCell::Known(_))) {
            return None;
        }
        snapshot.values.push(LoopValue::Bytes(bytes.to_vec()));
        Some(())
    }
}
