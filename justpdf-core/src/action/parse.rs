use crate::object::{PdfDict, PdfObject};
use crate::outline::types::Destination;

use super::types::*;

/// Parse an action from an action dictionary.
pub fn parse_action(dict: &PdfDict) -> Option<PdfAction> {
    let action_type = dict.get_name(b"S")?;

    match action_type {
        b"GoTo" => {
            let dest = dict.get(b"D").and_then(Destination::from_object)?;
            Some(PdfAction::GoTo { dest })
        }
        b"GoToR" => {
            let file = dict
                .get(b"F")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let dest = dict
                .get(b"D")
                .and_then(Destination::from_object)
                .unwrap_or(Destination::Named(String::new()));
            let new_window = dict.get(b"NewWindow").and_then(|o| o.as_bool());
            Some(PdfAction::GoToR { file, dest, new_window })
        }
        b"URI" => {
            let uri = dict
                .get(b"URI")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned())?;
            let is_map = dict
                .get(b"IsMap")
                .and_then(|o| o.as_bool())
                .unwrap_or(false);
            Some(PdfAction::URI { uri, is_map })
        }
        b"Named" => {
            let name = dict.get_name(b"N")?;
            Some(PdfAction::Named {
                name: NamedAction::from_name(name),
            })
        }
        b"Launch" => {
            let file = dict
                .get(b"F")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let new_window = dict.get(b"NewWindow").and_then(|o| o.as_bool());
            Some(PdfAction::Launch { file, new_window })
        }
        b"JavaScript" => {
            let script = dict
                .get(b"JS")
                .and_then(|o| match o {
                    PdfObject::String(s) => Some(String::from_utf8_lossy(s).into_owned()),
                    PdfObject::Stream { data, .. } => {
                        Some(String::from_utf8_lossy(data).into_owned())
                    }
                    _ => None,
                })?;
            Some(PdfAction::JavaScript { script })
        }
        b"SubmitForm" => {
            let url = dict
                .get(b"F")
                .and_then(|o| o.as_str())
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let flags = dict.get_i64(b"Flags").unwrap_or(0);
            Some(PdfAction::SubmitForm { url, flags })
        }
        b"ResetForm" => {
            let flags = dict.get_i64(b"Flags").unwrap_or(0);
            Some(PdfAction::ResetForm { flags })
        }
        _ => Some(PdfAction::Unknown {
            action_type: String::from_utf8_lossy(action_type).into_owned(),
        }),
    }
}

/// Parse a chain of actions (including /Next).
pub fn parse_action_chain(dict: &PdfDict) -> Vec<PdfAction> {
    let mut actions = Vec::new();
    if let Some(action) = parse_action(dict) {
        actions.push(action);
    }
    // Handle /Next (single action or array of actions)
    match dict.get(b"Next") {
        Some(PdfObject::Dict(next_dict)) => {
            actions.extend(parse_action_chain(next_dict));
        }
        Some(PdfObject::Array(arr)) => {
            for item in arr {
                if let PdfObject::Dict(next_dict) = item {
                    actions.extend(parse_action_chain(next_dict));
                }
            }
        }
        _ => {}
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{IndirectRef, PdfDict, PdfObject};

    fn page_ref(num: u32) -> IndirectRef {
        IndirectRef { obj_num: num, gen_num: 0 }
    }

    fn make_dest_array(page_num: u32) -> PdfObject {
        PdfObject::Array(vec![
            PdfObject::Reference(page_ref(page_num)),
            PdfObject::Name(b"Fit".to_vec()),
        ])
    }

    #[test]
    fn test_parse_goto() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GoTo".to_vec()));
        dict.insert(b"D".to_vec(), make_dest_array(5));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::GoTo {
                dest: Destination::Fit {
                    page_ref: page_ref(5),
                },
            }
        );
    }

    #[test]
    fn test_parse_goto_named_dest() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GoTo".to_vec()));
        dict.insert(b"D".to_vec(), PdfObject::String(b"chapter1".to_vec()));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::GoTo {
                dest: Destination::Named("chapter1".into()),
            }
        );
    }

    #[test]
    fn test_parse_gotor() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GoToR".to_vec()));
        dict.insert(b"F".to_vec(), PdfObject::String(b"other.pdf".to_vec()));
        dict.insert(b"D".to_vec(), PdfObject::String(b"chapter1".to_vec()));
        dict.insert(b"NewWindow".to_vec(), PdfObject::Bool(true));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::GoToR {
                file: "other.pdf".into(),
                dest: Destination::Named("chapter1".into()),
                new_window: Some(true),
            }
        );
    }

    #[test]
    fn test_parse_gotor_no_window() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GoToR".to_vec()));
        dict.insert(b"F".to_vec(), PdfObject::String(b"doc.pdf".to_vec()));
        dict.insert(b"D".to_vec(), make_dest_array(1));

        let action = parse_action(&dict).unwrap();
        match action {
            PdfAction::GoToR { new_window, .. } => assert_eq!(new_window, None),
            _ => panic!("expected GoToR"),
        }
    }

    #[test]
    fn test_parse_uri() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        dict.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com".to_vec()),
        );

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::URI {
                uri: "https://example.com".into(),
                is_map: false,
            }
        );
    }

    #[test]
    fn test_parse_uri_with_ismap() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        dict.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com/map".to_vec()),
        );
        dict.insert(b"IsMap".to_vec(), PdfObject::Bool(true));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::URI {
                uri: "https://example.com/map".into(),
                is_map: true,
            }
        );
    }

    #[test]
    fn test_parse_named() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        dict.insert(b"N".to_vec(), PdfObject::Name(b"NextPage".to_vec()));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::Named {
                name: NamedAction::NextPage,
            }
        );
    }

    #[test]
    fn test_parse_named_all_variants() {
        for (name_bytes, expected) in &[
            (&b"NextPage"[..], NamedAction::NextPage),
            (&b"PrevPage"[..], NamedAction::PrevPage),
            (&b"FirstPage"[..], NamedAction::FirstPage),
            (&b"LastPage"[..], NamedAction::LastPage),
        ] {
            let mut dict = PdfDict::new();
            dict.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
            dict.insert(b"N".to_vec(), PdfObject::Name(name_bytes.to_vec()));
            let action = parse_action(&dict).unwrap();
            assert_eq!(action, PdfAction::Named { name: expected.clone() });
        }
    }

    #[test]
    fn test_parse_launch() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"Launch".to_vec()));
        dict.insert(b"F".to_vec(), PdfObject::String(b"app.exe".to_vec()));
        dict.insert(b"NewWindow".to_vec(), PdfObject::Bool(false));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::Launch {
                file: "app.exe".into(),
                new_window: Some(false),
            }
        );
    }

    #[test]
    fn test_parse_javascript_string() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"JavaScript".to_vec()));
        dict.insert(
            b"JS".to_vec(),
            PdfObject::String(b"app.alert('hi')".to_vec()),
        );

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::JavaScript {
                script: "app.alert('hi')".into(),
            }
        );
    }

    #[test]
    fn test_parse_javascript_stream() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"JavaScript".to_vec()));
        dict.insert(
            b"JS".to_vec(),
            PdfObject::Stream {
                dict: PdfDict::new(),
                data: b"console.log('test')".to_vec(),
            },
        );

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::JavaScript {
                script: "console.log('test')".into(),
            }
        );
    }

    #[test]
    fn test_parse_submit_form() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"SubmitForm".to_vec()));
        dict.insert(
            b"F".to_vec(),
            PdfObject::String(b"https://example.com/submit".to_vec()),
        );
        dict.insert(b"Flags".to_vec(), PdfObject::Integer(4));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::SubmitForm {
                url: "https://example.com/submit".into(),
                flags: 4,
            }
        );
    }

    #[test]
    fn test_parse_reset_form() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"ResetForm".to_vec()));
        dict.insert(b"Flags".to_vec(), PdfObject::Integer(1));

        let action = parse_action(&dict).unwrap();
        assert_eq!(action, PdfAction::ResetForm { flags: 1 });
    }

    #[test]
    fn test_parse_reset_form_no_flags() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"ResetForm".to_vec()));

        let action = parse_action(&dict).unwrap();
        assert_eq!(action, PdfAction::ResetForm { flags: 0 });
    }

    #[test]
    fn test_parse_unknown_action() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"Thread".to_vec()));

        let action = parse_action(&dict).unwrap();
        assert_eq!(
            action,
            PdfAction::Unknown {
                action_type: "Thread".into(),
            }
        );
    }

    #[test]
    fn test_parse_no_action_type() {
        let dict = PdfDict::new();
        assert!(parse_action(&dict).is_none());
    }

    #[test]
    fn test_parse_action_chain_single() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        dict.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com".to_vec()),
        );

        let chain = parse_action_chain(&dict);
        assert_eq!(chain.len(), 1);
        assert_eq!(
            chain[0],
            PdfAction::URI {
                uri: "https://example.com".into(),
                is_map: false,
            }
        );
    }

    #[test]
    fn test_parse_action_chain_with_next_dict() {
        let mut next_dict = PdfDict::new();
        next_dict.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        next_dict.insert(b"N".to_vec(), PdfObject::Name(b"NextPage".to_vec()));

        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        dict.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com".to_vec()),
        );
        dict.insert(b"Next".to_vec(), PdfObject::Dict(next_dict));

        let chain = parse_action_chain(&dict);
        assert_eq!(chain.len(), 2);
        assert_eq!(
            chain[0],
            PdfAction::URI {
                uri: "https://example.com".into(),
                is_map: false,
            }
        );
        assert_eq!(
            chain[1],
            PdfAction::Named {
                name: NamedAction::NextPage,
            }
        );
    }

    #[test]
    fn test_parse_action_chain_with_next_array() {
        let mut next1 = PdfDict::new();
        next1.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        next1.insert(b"N".to_vec(), PdfObject::Name(b"FirstPage".to_vec()));

        let mut next2 = PdfDict::new();
        next2.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        next2.insert(b"N".to_vec(), PdfObject::Name(b"LastPage".to_vec()));

        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        dict.insert(
            b"URI".to_vec(),
            PdfObject::String(b"https://example.com".to_vec()),
        );
        dict.insert(
            b"Next".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Dict(next1),
                PdfObject::Dict(next2),
            ]),
        );

        let chain = parse_action_chain(&dict);
        assert_eq!(chain.len(), 3);
        assert_eq!(
            chain[1],
            PdfAction::Named {
                name: NamedAction::FirstPage,
            }
        );
        assert_eq!(
            chain[2],
            PdfAction::Named {
                name: NamedAction::LastPage,
            }
        );
    }

    #[test]
    fn test_parse_action_chain_nested_next() {
        // A -> B -> C via nested /Next dicts
        let mut dict_c = PdfDict::new();
        dict_c.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        dict_c.insert(b"N".to_vec(), PdfObject::Name(b"LastPage".to_vec()));

        let mut dict_b = PdfDict::new();
        dict_b.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        dict_b.insert(b"N".to_vec(), PdfObject::Name(b"PrevPage".to_vec()));
        dict_b.insert(b"Next".to_vec(), PdfObject::Dict(dict_c));

        let mut dict_a = PdfDict::new();
        dict_a.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        dict_a.insert(b"N".to_vec(), PdfObject::Name(b"NextPage".to_vec()));
        dict_a.insert(b"Next".to_vec(), PdfObject::Dict(dict_b));

        let chain = parse_action_chain(&dict_a);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], PdfAction::Named { name: NamedAction::NextPage });
        assert_eq!(chain[1], PdfAction::Named { name: NamedAction::PrevPage });
        assert_eq!(chain[2], PdfAction::Named { name: NamedAction::LastPage });
    }

    #[test]
    fn test_parse_uri_missing_uri_field() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"URI".to_vec()));
        // Missing /URI key
        assert!(parse_action(&dict).is_none());
    }

    #[test]
    fn test_parse_goto_missing_dest() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"GoTo".to_vec()));
        // Missing /D key
        assert!(parse_action(&dict).is_none());
    }

    #[test]
    fn test_parse_named_missing_name() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"Named".to_vec()));
        // Missing /N key
        assert!(parse_action(&dict).is_none());
    }

    #[test]
    fn test_parse_javascript_missing_js() {
        let mut dict = PdfDict::new();
        dict.insert(b"S".to_vec(), PdfObject::Name(b"JavaScript".to_vec()));
        // Missing /JS key
        assert!(parse_action(&dict).is_none());
    }

    #[test]
    fn test_parse_action_chain_empty_dict() {
        let dict = PdfDict::new();
        let chain = parse_action_chain(&dict);
        assert!(chain.is_empty());
    }
}
