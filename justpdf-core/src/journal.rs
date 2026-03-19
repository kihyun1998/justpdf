//! Journal / undo-redo system for tracking document modifications (Phase 7, §7.10).
//!
//! This module provides an application-level undo/redo journal that records
//! [`Operation`]s performed on a PDF document.  The caller is responsible for
//! actually applying the inverse operations; the journal merely tracks state.

use crate::object::{PdfDict, PdfObject};

// ---------------------------------------------------------------------------
// Operation
// ---------------------------------------------------------------------------

/// A single recorded operation that can be undone.
#[derive(Debug, Clone)]
pub enum Operation {
    /// An object was added (stores the obj_num for undo = delete).
    AddObject { obj_num: u32 },
    /// An object was modified (stores old value for undo).
    ModifyObject { obj_num: u32, old_value: PdfObject },
    /// An object was deleted (stores old value for undo = re-add).
    DeleteObject { obj_num: u32, old_value: PdfObject },
    /// A batch of operations grouped as one undoable action.
    Batch {
        ops: Vec<Operation>,
        description: String,
    },
}

// ---------------------------------------------------------------------------
// Journal
// ---------------------------------------------------------------------------

/// A journal that records operations for undo/redo support.
#[derive(Debug)]
pub struct Journal {
    /// Stack of past operations (most recent last).
    undo_stack: Vec<Operation>,
    /// Stack of undone operations (for redo).
    redo_stack: Vec<Operation>,
    /// Whether recording is enabled.
    recording: bool,
}

impl Journal {
    /// Create a new empty journal with recording enabled.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            recording: true,
        }
    }

    /// Start recording operations.
    pub fn start_recording(&mut self) {
        self.recording = true;
    }

    /// Stop recording operations.
    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    /// Whether recording is active.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Record an operation. Clears the redo stack.
    pub fn record(&mut self, op: Operation) {
        if !self.recording {
            return;
        }
        self.redo_stack.clear();
        self.undo_stack.push(op);
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Pop the last operation for undoing. Returns the operation to undo.
    /// The caller is responsible for applying the inverse.
    pub fn undo(&mut self) -> Option<Operation> {
        let op = self.undo_stack.pop()?;
        self.redo_stack.push(op.clone());
        Some(op)
    }

    /// Pop the last undone operation for redoing.
    pub fn redo(&mut self) -> Option<Operation> {
        let op = self.redo_stack.pop()?;
        self.undo_stack.push(op.clone());
        Some(op)
    }

    /// Get the inverse of an operation (what to do to undo it).
    pub fn inverse(op: &Operation) -> Operation {
        match op {
            Operation::AddObject { obj_num } => Operation::DeleteObject {
                obj_num: *obj_num,
                old_value: PdfObject::Null,
            },
            Operation::ModifyObject { obj_num, old_value } => Operation::ModifyObject {
                obj_num: *obj_num,
                old_value: old_value.clone(),
            },
            Operation::DeleteObject { obj_num, old_value: _ } => Operation::AddObject {
                obj_num: *obj_num,
            },
            Operation::Batch { ops, description } => Operation::Batch {
                ops: ops.iter().rev().map(Journal::inverse).collect(),
                description: format!("Undo: {}", description),
            },
        }
    }

    /// Number of undo steps available.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo steps available.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Begin a batch operation. Returns a [`BatchBuilder`].
    pub fn begin_batch(&mut self, description: &str) -> BatchBuilder<'_> {
        BatchBuilder {
            journal: self,
            ops: Vec::new(),
            description: description.to_string(),
        }
    }

    /// Serialize journal to bytes (for persistence).
    ///
    /// Binary format:
    /// - 4 bytes: magic `JRNL`
    /// - 4 bytes: version (1) as little-endian u32
    /// - 4 bytes: number of undo entries (LE u32)
    /// - For each entry: serialized [`Operation`]
    /// - 4 bytes: number of redo entries (LE u32)
    /// - For each entry: serialized [`Operation`]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Magic
        buf.extend_from_slice(b"JRNL");
        // Version
        buf.extend_from_slice(&1u32.to_le_bytes());
        // Undo stack
        buf.extend_from_slice(&(self.undo_stack.len() as u32).to_le_bytes());
        for op in &self.undo_stack {
            serialize_operation(&mut buf, op);
        }
        // Redo stack
        buf.extend_from_slice(&(self.redo_stack.len() as u32).to_le_bytes());
        for op in &self.redo_stack {
            serialize_operation(&mut buf, op);
        }
        buf
    }

    /// Deserialize journal from bytes.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut cursor = Cursor::new(data);
        // Magic
        let magic = cursor.read_bytes(4)?;
        if magic != b"JRNL" {
            return None;
        }
        // Version
        let version = cursor.read_u32()?;
        if version != 1 {
            return None;
        }
        // Undo stack
        let undo_count = cursor.read_u32()? as usize;
        let mut undo_stack = Vec::with_capacity(undo_count);
        for _ in 0..undo_count {
            undo_stack.push(deserialize_operation(&mut cursor)?);
        }
        // Redo stack
        let redo_count = cursor.read_u32()? as usize;
        let mut redo_stack = Vec::with_capacity(redo_count);
        for _ in 0..redo_count {
            redo_stack.push(deserialize_operation(&mut cursor)?);
        }
        Some(Self {
            undo_stack,
            redo_stack,
            recording: true,
        })
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BatchBuilder
// ---------------------------------------------------------------------------

/// Builder for batch operations.
pub struct BatchBuilder<'a> {
    journal: &'a mut Journal,
    ops: Vec<Operation>,
    description: String,
}

impl<'a> BatchBuilder<'a> {
    /// Record an operation into this batch.
    pub fn record(&mut self, op: Operation) {
        self.ops.push(op);
    }

    /// Commit the batch to the journal.
    pub fn commit(self) {
        if self.ops.is_empty() {
            return;
        }
        let batch = Operation::Batch {
            ops: self.ops,
            description: self.description,
        };
        self.journal.record(batch);
    }

    /// Cancel the batch (discard all recorded ops).
    pub fn cancel(self) {
        // Simply drop self without committing.
    }
}

// ---------------------------------------------------------------------------
// Binary serialization helpers
// ---------------------------------------------------------------------------

/// Tag bytes for [`Operation`] variants.
const TAG_ADD: u8 = 1;
const TAG_MODIFY: u8 = 2;
const TAG_DELETE: u8 = 3;
const TAG_BATCH: u8 = 4;

/// Tag bytes for [`PdfObject`] variants.
const OBJ_NULL: u8 = 0;
const OBJ_BOOL: u8 = 1;
const OBJ_INTEGER: u8 = 2;
const OBJ_REAL: u8 = 3;
const OBJ_NAME: u8 = 4;
const OBJ_STRING: u8 = 5;
const OBJ_ARRAY: u8 = 6;
const OBJ_DICT: u8 = 7;
const OBJ_STREAM: u8 = 8;
const OBJ_REFERENCE: u8 = 9;

fn serialize_operation(buf: &mut Vec<u8>, op: &Operation) {
    match op {
        Operation::AddObject { obj_num } => {
            buf.push(TAG_ADD);
            buf.extend_from_slice(&obj_num.to_le_bytes());
        }
        Operation::ModifyObject { obj_num, old_value } => {
            buf.push(TAG_MODIFY);
            buf.extend_from_slice(&obj_num.to_le_bytes());
            serialize_pdf_object(buf, old_value);
        }
        Operation::DeleteObject { obj_num, old_value } => {
            buf.push(TAG_DELETE);
            buf.extend_from_slice(&obj_num.to_le_bytes());
            serialize_pdf_object(buf, old_value);
        }
        Operation::Batch { ops, description } => {
            buf.push(TAG_BATCH);
            let desc_bytes = description.as_bytes();
            buf.extend_from_slice(&(desc_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(desc_bytes);
            buf.extend_from_slice(&(ops.len() as u32).to_le_bytes());
            for op in ops {
                serialize_operation(buf, op);
            }
        }
    }
}

fn serialize_pdf_object(buf: &mut Vec<u8>, obj: &PdfObject) {
    match obj {
        PdfObject::Null => buf.push(OBJ_NULL),
        PdfObject::Bool(v) => {
            buf.push(OBJ_BOOL);
            buf.push(if *v { 1 } else { 0 });
        }
        PdfObject::Integer(v) => {
            buf.push(OBJ_INTEGER);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        PdfObject::Real(v) => {
            buf.push(OBJ_REAL);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        PdfObject::Name(v) => {
            buf.push(OBJ_NAME);
            buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
            buf.extend_from_slice(v);
        }
        PdfObject::String(v) => {
            buf.push(OBJ_STRING);
            buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
            buf.extend_from_slice(v);
        }
        PdfObject::Array(items) => {
            buf.push(OBJ_ARRAY);
            buf.extend_from_slice(&(items.len() as u32).to_le_bytes());
            for item in items {
                serialize_pdf_object(buf, item);
            }
        }
        PdfObject::Dict(dict) => {
            buf.push(OBJ_DICT);
            serialize_pdf_dict(buf, dict);
        }
        PdfObject::Stream { dict, data } => {
            buf.push(OBJ_STREAM);
            serialize_pdf_dict(buf, dict);
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }
        PdfObject::Reference(r) => {
            buf.push(OBJ_REFERENCE);
            buf.extend_from_slice(&r.obj_num.to_le_bytes());
            buf.extend_from_slice(&r.gen_num.to_le_bytes());
        }
    }
}

fn serialize_pdf_dict(buf: &mut Vec<u8>, dict: &PdfDict) {
    let entries: Vec<_> = dict.iter().collect();
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (key, value) in entries {
        buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
        buf.extend_from_slice(key);
        serialize_pdf_object(buf, value);
    }
}

// ---------------------------------------------------------------------------
// Deserialization
// ---------------------------------------------------------------------------

/// A simple cursor over a byte slice for deserialization.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(slice)
    }

    fn read_u8(&mut self) -> Option<u8> {
        let b = self.read_bytes(1)?;
        Some(b[0])
    }

    fn read_u16(&mut self) -> Option<u16> {
        let b = self.read_bytes(2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.read_bytes(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i64(&mut self) -> Option<i64> {
        let b = self.read_bytes(8)?;
        Some(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_f64(&mut self) -> Option<f64> {
        let b = self.read_bytes(8)?;
        Some(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
}

fn deserialize_operation(cursor: &mut Cursor<'_>) -> Option<Operation> {
    let tag = cursor.read_u8()?;
    match tag {
        TAG_ADD => {
            let obj_num = cursor.read_u32()?;
            Some(Operation::AddObject { obj_num })
        }
        TAG_MODIFY => {
            let obj_num = cursor.read_u32()?;
            let old_value = deserialize_pdf_object(cursor)?;
            Some(Operation::ModifyObject { obj_num, old_value })
        }
        TAG_DELETE => {
            let obj_num = cursor.read_u32()?;
            let old_value = deserialize_pdf_object(cursor)?;
            Some(Operation::DeleteObject { obj_num, old_value })
        }
        TAG_BATCH => {
            let desc_len = cursor.read_u32()? as usize;
            let desc_bytes = cursor.read_bytes(desc_len)?;
            let description = std::str::from_utf8(desc_bytes).ok()?.to_string();
            let count = cursor.read_u32()? as usize;
            let mut ops = Vec::with_capacity(count);
            for _ in 0..count {
                ops.push(deserialize_operation(cursor)?);
            }
            Some(Operation::Batch { ops, description })
        }
        _ => None,
    }
}

fn deserialize_pdf_object(cursor: &mut Cursor<'_>) -> Option<PdfObject> {
    let tag = cursor.read_u8()?;
    match tag {
        OBJ_NULL => Some(PdfObject::Null),
        OBJ_BOOL => {
            let v = cursor.read_u8()?;
            Some(PdfObject::Bool(v != 0))
        }
        OBJ_INTEGER => {
            let v = cursor.read_i64()?;
            Some(PdfObject::Integer(v))
        }
        OBJ_REAL => {
            let v = cursor.read_f64()?;
            Some(PdfObject::Real(v))
        }
        OBJ_NAME => {
            let len = cursor.read_u32()? as usize;
            let bytes = cursor.read_bytes(len)?;
            Some(PdfObject::Name(bytes.to_vec()))
        }
        OBJ_STRING => {
            let len = cursor.read_u32()? as usize;
            let bytes = cursor.read_bytes(len)?;
            Some(PdfObject::String(bytes.to_vec()))
        }
        OBJ_ARRAY => {
            let count = cursor.read_u32()? as usize;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(deserialize_pdf_object(cursor)?);
            }
            Some(PdfObject::Array(items))
        }
        OBJ_DICT => {
            let dict = deserialize_pdf_dict(cursor)?;
            Some(PdfObject::Dict(dict))
        }
        OBJ_STREAM => {
            let dict = deserialize_pdf_dict(cursor)?;
            let data_len = cursor.read_u32()? as usize;
            let data = cursor.read_bytes(data_len)?.to_vec();
            Some(PdfObject::Stream { dict, data })
        }
        OBJ_REFERENCE => {
            let obj_num = cursor.read_u32()?;
            let gen_num = cursor.read_u16()?;
            Some(PdfObject::Reference(crate::object::IndirectRef {
                obj_num,
                gen_num,
            }))
        }
        _ => None,
    }
}

fn deserialize_pdf_dict(cursor: &mut Cursor<'_>) -> Option<PdfDict> {
    let count = cursor.read_u32()? as usize;
    let mut dict = PdfDict::new();
    for _ in 0..count {
        let key_len = cursor.read_u32()? as usize;
        let key = cursor.read_bytes(key_len)?.to_vec();
        let value = deserialize_pdf_object(cursor)?;
        dict.insert(key, value);
    }
    Some(dict)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{IndirectRef, PdfDict, PdfObject};

    fn sample_object() -> PdfObject {
        PdfObject::Integer(42)
    }

    #[test]
    fn record_and_undo_single() {
        let mut j = Journal::new();
        j.record(Operation::AddObject { obj_num: 1 });
        assert!(j.can_undo());
        assert_eq!(j.undo_count(), 1);

        let op = j.undo().unwrap();
        assert!(!j.can_undo());
        assert!(j.can_redo());
        match op {
            Operation::AddObject { obj_num } => assert_eq!(obj_num, 1),
            _ => panic!("expected AddObject"),
        }
    }

    #[test]
    fn record_and_redo_after_undo() {
        let mut j = Journal::new();
        j.record(Operation::ModifyObject {
            obj_num: 5,
            old_value: sample_object(),
        });
        j.undo();
        assert!(j.can_redo());
        assert_eq!(j.redo_count(), 1);

        let op = j.redo().unwrap();
        assert!(j.can_undo());
        assert!(!j.can_redo());
        match op {
            Operation::ModifyObject { obj_num, .. } => assert_eq!(obj_num, 5),
            _ => panic!("expected ModifyObject"),
        }
    }

    #[test]
    fn recording_disabled_no_ops() {
        let mut j = Journal::new();
        j.stop_recording();
        assert!(!j.is_recording());
        j.record(Operation::AddObject { obj_num: 1 });
        assert!(!j.can_undo());
        assert_eq!(j.undo_count(), 0);
    }

    #[test]
    fn redo_cleared_on_new_record() {
        let mut j = Journal::new();
        j.record(Operation::AddObject { obj_num: 1 });
        j.record(Operation::AddObject { obj_num: 2 });
        j.undo(); // redo has obj_num=2
        assert!(j.can_redo());

        // Recording a new op clears redo
        j.record(Operation::AddObject { obj_num: 3 });
        assert!(!j.can_redo());
        assert_eq!(j.redo_count(), 0);
    }

    #[test]
    fn batch_operations() {
        let mut j = Journal::new();
        {
            let mut batch = j.begin_batch("add two objects");
            batch.record(Operation::AddObject { obj_num: 10 });
            batch.record(Operation::AddObject { obj_num: 11 });
            batch.commit();
        }
        assert_eq!(j.undo_count(), 1);
        let op = j.undo().unwrap();
        match op {
            Operation::Batch { ops, description } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(description, "add two objects");
            }
            _ => panic!("expected Batch"),
        }
    }

    #[test]
    fn batch_cancel() {
        let mut j = Journal::new();
        {
            let mut batch = j.begin_batch("will cancel");
            batch.record(Operation::AddObject { obj_num: 99 });
            batch.cancel();
        }
        assert!(!j.can_undo());
        assert_eq!(j.undo_count(), 0);
    }

    #[test]
    fn inverse_add() {
        let op = Operation::AddObject { obj_num: 7 };
        let inv = Journal::inverse(&op);
        match inv {
            Operation::DeleteObject { obj_num, .. } => assert_eq!(obj_num, 7),
            _ => panic!("expected DeleteObject"),
        }
    }

    #[test]
    fn inverse_modify() {
        let op = Operation::ModifyObject {
            obj_num: 3,
            old_value: PdfObject::Bool(true),
        };
        let inv = Journal::inverse(&op);
        match inv {
            Operation::ModifyObject { obj_num, old_value } => {
                assert_eq!(obj_num, 3);
                assert_eq!(old_value, PdfObject::Bool(true));
            }
            _ => panic!("expected ModifyObject"),
        }
    }

    #[test]
    fn inverse_delete() {
        let op = Operation::DeleteObject {
            obj_num: 4,
            old_value: PdfObject::Integer(100),
        };
        let inv = Journal::inverse(&op);
        match inv {
            Operation::AddObject { obj_num } => assert_eq!(obj_num, 4),
            _ => panic!("expected AddObject"),
        }
    }

    #[test]
    fn inverse_batch() {
        let op = Operation::Batch {
            ops: vec![
                Operation::AddObject { obj_num: 1 },
                Operation::DeleteObject {
                    obj_num: 2,
                    old_value: PdfObject::Null,
                },
            ],
            description: "test batch".to_string(),
        };
        let inv = Journal::inverse(&op);
        match inv {
            Operation::Batch { ops, description } => {
                assert_eq!(description, "Undo: test batch");
                // Should be reversed order
                assert_eq!(ops.len(), 2);
                match &ops[0] {
                    Operation::AddObject { obj_num } => assert_eq!(*obj_num, 2),
                    _ => panic!("expected AddObject as inverse of DeleteObject"),
                }
                match &ops[1] {
                    Operation::DeleteObject { obj_num, .. } => assert_eq!(*obj_num, 1),
                    _ => panic!("expected DeleteObject as inverse of AddObject"),
                }
            }
            _ => panic!("expected Batch"),
        }
    }

    #[test]
    fn multiple_undo_redo_cycles() {
        let mut j = Journal::new();
        j.record(Operation::AddObject { obj_num: 1 });
        j.record(Operation::AddObject { obj_num: 2 });
        j.record(Operation::AddObject { obj_num: 3 });
        assert_eq!(j.undo_count(), 3);

        // Undo all
        j.undo();
        j.undo();
        j.undo();
        assert_eq!(j.undo_count(), 0);
        assert_eq!(j.redo_count(), 3);

        // Redo all
        j.redo();
        j.redo();
        j.redo();
        assert_eq!(j.undo_count(), 3);
        assert_eq!(j.redo_count(), 0);

        // Undo two, redo one
        j.undo();
        j.undo();
        assert_eq!(j.undo_count(), 1);
        assert_eq!(j.redo_count(), 2);
        j.redo();
        assert_eq!(j.undo_count(), 2);
        assert_eq!(j.redo_count(), 1);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut j = Journal::new();
        j.record(Operation::AddObject { obj_num: 1 });
        j.record(Operation::ModifyObject {
            obj_num: 2,
            old_value: PdfObject::Name(b"Type".to_vec()),
        });
        j.record(Operation::DeleteObject {
            obj_num: 3,
            old_value: PdfObject::Array(vec![
                PdfObject::Integer(10),
                PdfObject::Real(3.14),
                PdfObject::String(b"hello".to_vec()),
            ]),
        });
        // Create a batch
        {
            let mut batch = j.begin_batch("batch op");
            batch.record(Operation::AddObject { obj_num: 100 });
            batch.commit();
        }
        // Undo one to populate redo stack
        j.undo();

        let bytes = j.to_bytes();
        let j2 = Journal::from_bytes(&bytes).expect("deserialization failed");

        assert_eq!(j2.undo_count(), j.undo_count());
        assert_eq!(j2.redo_count(), j.redo_count());

        // Re-serialize and check bytes are identical
        let bytes2 = j2.to_bytes();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn serialization_roundtrip_complex_objects() {
        let mut dict = PdfDict::new();
        dict.insert(b"Key".to_vec(), PdfObject::Bool(false));
        dict.insert(b"Other".to_vec(), PdfObject::Null);

        let mut j = Journal::new();
        j.record(Operation::ModifyObject {
            obj_num: 50,
            old_value: PdfObject::Dict(dict),
        });
        j.record(Operation::DeleteObject {
            obj_num: 51,
            old_value: PdfObject::Stream {
                dict: PdfDict::new(),
                data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            },
        });
        j.record(Operation::ModifyObject {
            obj_num: 52,
            old_value: PdfObject::Reference(IndirectRef {
                obj_num: 99,
                gen_num: 2,
            }),
        });

        let bytes = j.to_bytes();
        let j2 = Journal::from_bytes(&bytes).expect("deserialization failed");
        assert_eq!(j2.undo_count(), 3);
        assert_eq!(j2.to_bytes(), bytes);
    }

    #[test]
    fn empty_journal_serialization() {
        let j = Journal::new();
        let bytes = j.to_bytes();
        // Magic(4) + version(4) + undo_count(4) + redo_count(4) = 16 bytes
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], b"JRNL");

        let j2 = Journal::from_bytes(&bytes).expect("deserialization failed");
        assert_eq!(j2.undo_count(), 0);
        assert_eq!(j2.redo_count(), 0);
        assert!(j2.is_recording());
    }

    #[test]
    fn can_undo_can_redo_states() {
        let mut j = Journal::new();
        assert!(!j.can_undo());
        assert!(!j.can_redo());

        j.record(Operation::AddObject { obj_num: 1 });
        assert!(j.can_undo());
        assert!(!j.can_redo());

        j.undo();
        assert!(!j.can_undo());
        assert!(j.can_redo());

        j.redo();
        assert!(j.can_undo());
        assert!(!j.can_redo());
    }

    #[test]
    fn clear_history() {
        let mut j = Journal::new();
        j.record(Operation::AddObject { obj_num: 1 });
        j.record(Operation::AddObject { obj_num: 2 });
        j.undo();
        assert!(j.can_undo());
        assert!(j.can_redo());

        j.clear();
        assert!(!j.can_undo());
        assert!(!j.can_redo());
        assert_eq!(j.undo_count(), 0);
        assert_eq!(j.redo_count(), 0);
    }

    #[test]
    fn undo_on_empty_returns_none() {
        let mut j = Journal::new();
        assert!(j.undo().is_none());
    }

    #[test]
    fn redo_on_empty_returns_none() {
        let mut j = Journal::new();
        assert!(j.redo().is_none());
    }

    #[test]
    fn from_bytes_invalid_magic() {
        assert!(Journal::from_bytes(b"XXXX").is_none());
    }

    #[test]
    fn from_bytes_truncated() {
        assert!(Journal::from_bytes(b"JRN").is_none());
        assert!(Journal::from_bytes(b"").is_none());
    }
}
