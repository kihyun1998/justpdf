use crate::object::{PdfDict, PdfObject};

use super::types::*;

/// Build a PDF action dictionary from a PdfAction.
pub fn build_action(action: &PdfAction) -> PdfDict {
    let mut dict = PdfDict::new();

    match action {
        PdfAction::GoTo { dest } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"GoTo".to_vec()));
            dict.insert(b"D".to_vec(), dest.to_pdf_array());
        }
        PdfAction::GoToR { file, dest, new_window } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"GoToR".to_vec()));
            dict.insert(b"F".to_vec(), PdfObject::String(file.as_bytes().to_vec()));
            dict.insert(b"D".to_vec(), dest.to_pdf_array());
            if let Some(nw) = new_window {
                dict.insert(b"NewWindow".to_vec(), PdfObject::Bool(*nw));
            }
        }
        PdfAction::URI { uri, is_map } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
            dict.insert(b"URI".to_vec(), PdfObject::String(uri.as_bytes().to_vec()));
            if *is_map {
                dict.insert(b"IsMap".to_vec(), PdfObject::Bool(true));
            }
        }
        PdfAction::Named { name } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
            dict.insert(b"N".to_vec(), PdfObject::Name(name.to_name()));
        }
        PdfAction::Launch { file, new_window } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"Launch".to_vec()));
            dict.insert(b"F".to_vec(), PdfObject::String(file.as_bytes().to_vec()));
            if let Some(nw) = new_window {
                dict.insert(b"NewWindow".to_vec(), PdfObject::Bool(*nw));
            }
        }
        PdfAction::JavaScript { script } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"JavaScript".to_vec()));
            dict.insert(b"JS".to_vec(), PdfObject::String(script.as_bytes().to_vec()));
        }
        PdfAction::SubmitForm { url, flags } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"SubmitForm".to_vec()));
            dict.insert(b"F".to_vec(), PdfObject::String(url.as_bytes().to_vec()));
            dict.insert(b"Flags".to_vec(), PdfObject::Integer(*flags));
        }
        PdfAction::ResetForm { flags } => {
            dict.insert(b"S".to_vec(), PdfObject::Name(b"ResetForm".to_vec()));
            dict.insert(b"Flags".to_vec(), PdfObject::Integer(*flags));
        }
        PdfAction::Unknown { action_type } => {
            dict.insert(
                b"S".to_vec(),
                PdfObject::Name(action_type.as_bytes().to_vec()),
            );
        }
    }

    dict
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::parse::parse_action;
    use crate::object::IndirectRef;

    fn page_ref(num: u32) -> IndirectRef {
        IndirectRef { obj_num: num, gen_num: 0 }
    }

    #[test]
    fn test_build_goto() {
        let action = PdfAction::GoTo {
            dest: Destination::Fit {
                page_ref: page_ref(3),
            },
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"GoTo".as_slice()));
        assert!(dict.get(b"D").is_some());
    }

    #[test]
    fn test_build_uri() {
        let action = PdfAction::URI {
            uri: "https://example.com".into(),
            is_map: false,
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"URI".as_slice()));
        assert_eq!(
            dict.get_string(b"URI"),
            Some(b"https://example.com".as_slice())
        );
        // is_map is false, so IsMap should not be present
        assert!(dict.get(b"IsMap").is_none());
    }

    #[test]
    fn test_build_uri_with_ismap() {
        let action = PdfAction::URI {
            uri: "https://example.com/map".into(),
            is_map: true,
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_bool(b"IsMap"), Some(true));
    }

    #[test]
    fn test_build_named() {
        let action = PdfAction::Named {
            name: NamedAction::PrevPage,
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"Named".as_slice()));
        assert_eq!(dict.get_name(b"N"), Some(b"PrevPage".as_slice()));
    }

    #[test]
    fn test_build_launch() {
        let action = PdfAction::Launch {
            file: "readme.txt".into(),
            new_window: Some(true),
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"Launch".as_slice()));
        assert_eq!(
            dict.get_string(b"F"),
            Some(b"readme.txt".as_slice())
        );
        assert_eq!(dict.get_bool(b"NewWindow"), Some(true));
    }

    #[test]
    fn test_build_launch_no_window() {
        let action = PdfAction::Launch {
            file: "readme.txt".into(),
            new_window: None,
        };
        let dict = build_action(&action);
        assert!(dict.get(b"NewWindow").is_none());
    }

    #[test]
    fn test_build_javascript() {
        let action = PdfAction::JavaScript {
            script: "app.alert('hello')".into(),
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"JavaScript".as_slice()));
        assert_eq!(
            dict.get_string(b"JS"),
            Some(b"app.alert('hello')".as_slice())
        );
    }

    #[test]
    fn test_build_submit_form() {
        let action = PdfAction::SubmitForm {
            url: "https://example.com/submit".into(),
            flags: 4,
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"SubmitForm".as_slice()));
        assert_eq!(
            dict.get_string(b"F"),
            Some(b"https://example.com/submit".as_slice())
        );
        assert_eq!(dict.get_i64(b"Flags"), Some(4));
    }

    #[test]
    fn test_build_reset_form() {
        let action = PdfAction::ResetForm { flags: 1 };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"ResetForm".as_slice()));
        assert_eq!(dict.get_i64(b"Flags"), Some(1));
    }

    #[test]
    fn test_build_unknown() {
        let action = PdfAction::Unknown {
            action_type: "Thread".into(),
        };
        let dict = build_action(&action);
        assert_eq!(dict.get_name(b"S"), Some(b"Thread".as_slice()));
    }

    // --- Roundtrip tests: build then parse ---

    #[test]
    fn test_roundtrip_goto_fit() {
        let action = PdfAction::GoTo {
            dest: Destination::Fit {
                page_ref: page_ref(10),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fith() {
        let action = PdfAction::GoTo {
            dest: Destination::FitH {
                page_ref: page_ref(10),
                top: Some(200.0),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_xyz() {
        let action = PdfAction::GoTo {
            dest: Destination::XYZ {
                page_ref: page_ref(42),
                left: Some(72.0),
                top: Some(720.0),
                zoom: Some(1.5),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fitr() {
        let action = PdfAction::GoTo {
            dest: Destination::FitR {
                page_ref: page_ref(7),
                left: 0.0,
                bottom: 0.0,
                right: 612.0,
                top: 792.0,
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fitb() {
        let action = PdfAction::GoTo {
            dest: Destination::FitB {
                page_ref: page_ref(2),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fitbh() {
        let action = PdfAction::GoTo {
            dest: Destination::FitBH {
                page_ref: page_ref(3),
                top: Some(100.0),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fitbv() {
        let action = PdfAction::GoTo {
            dest: Destination::FitBV {
                page_ref: page_ref(4),
                left: Some(50.0),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_fitv() {
        let action = PdfAction::GoTo {
            dest: Destination::FitV {
                page_ref: page_ref(6),
                left: Some(36.0),
            },
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_gotor() {
        let action = PdfAction::GoToR {
            file: "other.pdf".into(),
            dest: Destination::Named("intro".into()),
            new_window: Some(true),
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_gotor_no_window() {
        let action = PdfAction::GoToR {
            file: "doc.pdf".into(),
            dest: Destination::Fit {
                page_ref: page_ref(1),
            },
            new_window: None,
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_uri() {
        let action = PdfAction::URI {
            uri: "https://rust-lang.org".into(),
            is_map: false,
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_uri_ismap() {
        let action = PdfAction::URI {
            uri: "https://maps.example.com".into(),
            is_map: true,
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_named_actions() {
        let named_actions = vec![
            NamedAction::NextPage,
            NamedAction::PrevPage,
            NamedAction::FirstPage,
            NamedAction::LastPage,
            NamedAction::Other("Print".into()),
        ];
        for na in named_actions {
            let action = PdfAction::Named { name: na };
            let dict = build_action(&action);
            let parsed = parse_action(&dict).unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[test]
    fn test_roundtrip_launch() {
        let action = PdfAction::Launch {
            file: "notepad.exe".into(),
            new_window: Some(false),
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_javascript() {
        let action = PdfAction::JavaScript {
            script: "this.print()".into(),
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_submit_form() {
        let action = PdfAction::SubmitForm {
            url: "https://example.com/form".into(),
            flags: 28,
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_reset_form() {
        let action = PdfAction::ResetForm { flags: 3 };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_unknown() {
        let action = PdfAction::Unknown {
            action_type: "ImportData".into(),
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_roundtrip_goto_named_dest() {
        let action = PdfAction::GoTo {
            dest: Destination::Named("chapter5".into()),
        };
        let dict = build_action(&action);
        let parsed = parse_action(&dict).unwrap();
        assert_eq!(parsed, action);
    }
}
