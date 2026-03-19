// Re-export Destination so that users of the action module can access it directly.
pub use crate::outline::types::Destination;

/// PDF action types (section 12.6 of the PDF spec).
#[derive(Debug, Clone, PartialEq)]
pub enum PdfAction {
    /// GoTo: navigate to a destination within the same document.
    GoTo { dest: Destination },
    /// GoToR: navigate to a destination in another PDF file.
    GoToR {
        file: String,
        dest: Destination,
        new_window: Option<bool>,
    },
    /// URI: open a URI.
    URI {
        uri: String,
        is_map: bool,
    },
    /// Named action (NextPage, PrevPage, FirstPage, LastPage).
    Named { name: NamedAction },
    /// Launch an application or open a file.
    Launch {
        file: String,
        new_window: Option<bool>,
    },
    /// JavaScript action.
    JavaScript { script: String },
    /// SubmitForm action.
    SubmitForm { url: String, flags: i64 },
    /// ResetForm action.
    ResetForm { flags: i64 },
    /// Unknown/unsupported action type.
    Unknown { action_type: String },
}

/// Standard named actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedAction {
    NextPage,
    PrevPage,
    FirstPage,
    LastPage,
    Other(String),
}

impl NamedAction {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"NextPage" => Self::NextPage,
            b"PrevPage" => Self::PrevPage,
            b"FirstPage" => Self::FirstPage,
            b"LastPage" => Self::LastPage,
            _ => Self::Other(String::from_utf8_lossy(name).into_owned()),
        }
    }

    pub fn to_name(&self) -> Vec<u8> {
        match self {
            Self::NextPage => b"NextPage".to_vec(),
            Self::PrevPage => b"PrevPage".to_vec(),
            Self::FirstPage => b"FirstPage".to_vec(),
            Self::LastPage => b"LastPage".to_vec(),
            Self::Other(s) => s.as_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_action_roundtrip() {
        let actions = [
            NamedAction::NextPage,
            NamedAction::PrevPage,
            NamedAction::FirstPage,
            NamedAction::LastPage,
            NamedAction::Other("Print".into()),
        ];
        for action in &actions {
            let bytes = action.to_name();
            let parsed = NamedAction::from_name(&bytes);
            assert_eq!(&parsed, action);
        }
    }

    #[test]
    fn test_named_action_from_name() {
        assert_eq!(NamedAction::from_name(b"NextPage"), NamedAction::NextPage);
        assert_eq!(NamedAction::from_name(b"PrevPage"), NamedAction::PrevPage);
        assert_eq!(NamedAction::from_name(b"FirstPage"), NamedAction::FirstPage);
        assert_eq!(NamedAction::from_name(b"LastPage"), NamedAction::LastPage);
        assert_eq!(
            NamedAction::from_name(b"CustomAction"),
            NamedAction::Other("CustomAction".into())
        );
    }

    #[test]
    fn test_named_action_to_name() {
        assert_eq!(NamedAction::NextPage.to_name(), b"NextPage");
        assert_eq!(NamedAction::PrevPage.to_name(), b"PrevPage");
        assert_eq!(NamedAction::FirstPage.to_name(), b"FirstPage");
        assert_eq!(NamedAction::LastPage.to_name(), b"LastPage");
        assert_eq!(NamedAction::Other("Print".into()).to_name(), b"Print");
    }

    #[test]
    fn test_pdf_action_clone_eq() {
        let action = PdfAction::URI {
            uri: "https://example.com".into(),
            is_map: false,
        };
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_pdf_action_debug() {
        let action = PdfAction::Named {
            name: NamedAction::NextPage,
        };
        let debug = format!("{:?}", action);
        assert!(debug.contains("NextPage"));
    }
}
