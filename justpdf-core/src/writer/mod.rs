pub mod document;
pub mod encode;
pub mod modify;
pub mod page;
pub mod serialize;

pub use document::{DocumentBuilder, embed_jpeg, embed_png};
pub use encode::{encode_flate, make_stream};
pub use modify::{merge_documents, incremental_save, DocumentModifier};
pub use page::PageBuilder;
pub use serialize::{serialize_pdf, serialize_pdf_encrypted};

use crate::error::Result;
use crate::object::{IndirectRef, PdfObject};

use std::path::Path;

/// Low-level PDF writer that accumulates indirect objects and serializes them
/// into a valid PDF byte stream.
pub struct PdfWriter {
    /// Stored indirect objects: (object number, object).
    pub objects: Vec<(u32, PdfObject)>,
    /// Next available object number.
    pub(crate) next_obj_num: u32,
    /// PDF version as (major, minor), e.g. (1, 7).
    pub version: (u8, u8),
}

impl PdfWriter {
    /// Create a new writer with default PDF 1.7 version.
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            next_obj_num: 1,
            version: (1, 7),
        }
    }

    /// Allocate the next object number without adding an object.
    /// Useful for forward references.
    pub fn alloc_object_num(&mut self) -> u32 {
        let num = self.next_obj_num;
        self.next_obj_num += 1;
        num
    }

    /// Add an object, assigning it the next available object number.
    /// Returns an `IndirectRef` that can be used to reference this object.
    pub fn add_object(&mut self, obj: PdfObject) -> IndirectRef {
        let num = self.alloc_object_num();
        self.objects.push((num, obj));
        IndirectRef {
            obj_num: num,
            gen_num: 0,
        }
    }

    /// Set (or replace) an object at a specific object number.
    /// If an object with this number already exists, it is replaced.
    pub fn set_object(&mut self, obj_num: u32, obj: PdfObject) {
        if let Some(entry) = self.objects.iter_mut().find(|(n, _)| *n == obj_num) {
            entry.1 = obj;
        } else {
            self.objects.push((obj_num, obj));
        }
    }

    /// Serialize all objects into a complete PDF byte stream.
    pub fn write_to_bytes(&self, catalog_ref: &IndirectRef) -> Result<Vec<u8>> {
        serialize_pdf(&self.objects, self.version, catalog_ref, None)
    }

    /// Serialize and write to a file.
    pub fn write_to_file(&self, path: &Path, catalog_ref: &IndirectRef) -> Result<()> {
        let bytes = self.write_to_bytes(catalog_ref)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::PdfObject;

    #[test]
    fn test_add_object_increments_refs() {
        let mut writer = PdfWriter::new();

        let ref1 = writer.add_object(PdfObject::Integer(1));
        assert_eq!(ref1.obj_num, 1);
        assert_eq!(ref1.gen_num, 0);

        let ref2 = writer.add_object(PdfObject::Integer(2));
        assert_eq!(ref2.obj_num, 2);
        assert_eq!(ref2.gen_num, 0);

        let ref3 = writer.add_object(PdfObject::Integer(3));
        assert_eq!(ref3.obj_num, 3);
        assert_eq!(ref3.gen_num, 0);

        assert_eq!(writer.objects.len(), 3);
    }

    #[test]
    fn test_alloc_object_num() {
        let mut writer = PdfWriter::new();
        let num = writer.alloc_object_num();
        assert_eq!(num, 1);

        let ref1 = writer.add_object(PdfObject::Null);
        assert_eq!(ref1.obj_num, 2);
    }

    #[test]
    fn test_set_object_replaces() {
        let mut writer = PdfWriter::new();
        let r = writer.add_object(PdfObject::Integer(10));
        writer.set_object(r.obj_num, PdfObject::Integer(20));

        assert_eq!(writer.objects.len(), 1);
        assert_eq!(writer.objects[0].1, PdfObject::Integer(20));
    }

    #[test]
    fn test_set_object_new() {
        let mut writer = PdfWriter::new();
        writer.set_object(5, PdfObject::Integer(50));
        assert_eq!(writer.objects.len(), 1);
        assert_eq!(writer.objects[0].0, 5);
    }
}
