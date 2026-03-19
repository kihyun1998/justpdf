use crate::object::{IndirectRef, PdfObject};
use crate::page::Rect;

/// AcroForm field type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Checkbox,
    RadioButton,
    PushButton,
    ComboBox,
    ListBox,
}

/// Field flags (from /Ff entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldFlags(pub u32);

impl FieldFlags {
    pub const READ_ONLY: u32 = 1;
    pub const REQUIRED: u32 = 1 << 1;
    pub const NO_EXPORT: u32 = 1 << 2;
    // Button-specific
    pub const NO_TOGGLE_TO_OFF: u32 = 1 << 14;
    pub const RADIO: u32 = 1 << 15;
    pub const PUSH_BUTTON: u32 = 1 << 16;
    // Text-specific
    pub const MULTILINE: u32 = 1 << 12;
    pub const PASSWORD: u32 = 1 << 13;
    pub const FILE_SELECT: u32 = 1 << 20;
    pub const DO_NOT_SPELL_CHECK: u32 = 1 << 22;
    pub const DO_NOT_SCROLL: u32 = 1 << 23;
    pub const COMB: u32 = 1 << 24;
    pub const RICH_TEXT: u32 = 1 << 25;
    // Choice-specific
    pub const COMBO: u32 = 1 << 17;
    pub const EDIT: u32 = 1 << 18;
    pub const SORT: u32 = 1 << 19;
    pub const MULTI_SELECT: u32 = 1 << 21;
    pub const COMMIT_ON_SEL_CHANGE: u32 = 1 << 26;

    pub fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }

    pub fn is_read_only(self) -> bool {
        self.has(Self::READ_ONLY)
    }

    pub fn is_required(self) -> bool {
        self.has(Self::REQUIRED)
    }
}

/// A form field.
#[derive(Debug, Clone)]
pub struct FormField {
    /// Fully qualified field name (dot-separated).
    pub name: String,
    /// Partial field name (/T).
    pub partial_name: String,
    /// Field type.
    pub field_type: FieldType,
    /// Current value (/V).
    pub value: Option<PdfObject>,
    /// Default value (/DV).
    pub default_value: Option<PdfObject>,
    /// Field flags (/Ff).
    pub flags: FieldFlags,
    /// Options for choice fields (/Opt).
    pub options: Vec<String>,
    /// Widget rectangle.
    pub rect: Option<Rect>,
    /// Default appearance string (/DA).
    pub default_appearance: Option<String>,
    /// Reference to the field object.
    pub field_ref: IndirectRef,
    /// Object number of the page containing the widget.
    pub page_obj_num: Option<u32>,
}

impl FormField {
    /// Get the field value as a string.
    pub fn value_as_string(&self) -> Option<String> {
        match &self.value {
            Some(PdfObject::String(s)) => Some(String::from_utf8_lossy(s).into_owned()),
            Some(PdfObject::Name(n)) => Some(String::from_utf8_lossy(n).into_owned()),
            _ => None,
        }
    }

    /// Check if a checkbox/radio button is checked.
    pub fn is_checked(&self) -> bool {
        match &self.value {
            Some(PdfObject::Name(n)) => n != b"Off",
            _ => false,
        }
    }
}

/// Parsed AcroForm.
#[derive(Debug, Clone)]
pub struct AcroForm {
    pub fields: Vec<FormField>,
    pub need_appearances: bool,
    pub sig_flags: u32,
    pub default_appearance: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_flags() {
        let flags = FieldFlags(FieldFlags::READ_ONLY | FieldFlags::REQUIRED);
        assert!(flags.is_read_only());
        assert!(flags.is_required());
        assert!(!flags.has(FieldFlags::MULTILINE));
    }

    #[test]
    fn test_field_type_determination() {
        // Button with PUSH_BUTTON flag
        let flags = FieldFlags(FieldFlags::PUSH_BUTTON);
        assert!(flags.has(FieldFlags::PUSH_BUTTON));

        // Button with RADIO flag
        let flags = FieldFlags(FieldFlags::RADIO);
        assert!(flags.has(FieldFlags::RADIO));
    }

    #[test]
    fn test_form_field_value() {
        let field = FormField {
            name: "test".to_string(),
            partial_name: "test".to_string(),
            field_type: FieldType::Text,
            value: Some(PdfObject::String(b"Hello".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: None,
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        assert_eq!(field.value_as_string(), Some("Hello".to_string()));
    }

    #[test]
    fn test_checkbox_checked() {
        let checked = FormField {
            name: "cb".to_string(),
            partial_name: "cb".to_string(),
            field_type: FieldType::Checkbox,
            value: Some(PdfObject::Name(b"Yes".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: None,
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        assert!(checked.is_checked());

        let unchecked = FormField {
            name: "cb".to_string(),
            partial_name: "cb".to_string(),
            field_type: FieldType::Checkbox,
            value: Some(PdfObject::Name(b"Off".to_vec())),
            default_value: None,
            flags: FieldFlags::default(),
            options: Vec::new(),
            rect: None,
            default_appearance: None,
            field_ref: IndirectRef { obj_num: 1, gen_num: 0 },
            page_obj_num: None,
        };
        assert!(!unchecked.is_checked());
    }
}
