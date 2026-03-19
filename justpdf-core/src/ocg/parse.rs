use crate::error::Result;
use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;

use super::types::*;

/// Read Optional Content Properties from the document catalog.
///
/// Parses Catalog -> /OCProperties which contains:
/// - /OCGs: array of OCG indirect refs
/// - /D: default viewing configuration dict
/// - /Configs: optional array of alternate configuration dicts
pub fn read_oc_properties(doc: &mut PdfDocument) -> Result<Option<OCProperties>> {
    // Get the catalog
    let catalog_ref = match doc.catalog_ref() {
        Some(r) => r.clone(),
        None => return Ok(None),
    };
    let catalog_obj = doc.resolve(&catalog_ref)?.clone();
    let catalog_dict = match catalog_obj.as_dict() {
        Some(d) => d.clone(),
        None => return Ok(None),
    };

    // Get /OCProperties - may be inline dict or indirect ref
    let oc_props_dict = match catalog_dict.get(b"OCProperties") {
        Some(PdfObject::Dict(d)) => d.clone(),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let resolved = doc.resolve(&r)?.clone();
            match resolved.as_dict() {
                Some(d) => d.clone(),
                None => return Ok(None),
            }
        }
        _ => return Ok(None),
    };

    // Parse /OCGs array
    let mut groups = Vec::new();
    let ocg_refs = get_array_from_dict(doc, &oc_props_dict, b"OCGs")?;
    for item in &ocg_refs {
        if let PdfObject::Reference(r) = item {
            let r = r.clone();
            let ocg_obj = doc.resolve(&r)?.clone();
            if let Some(ocg_dict) = ocg_obj.as_dict() {
                groups.push(parse_ocg(ocg_dict, r));
            }
        }
    }

    // Parse /D (default configuration)
    let default_config = match oc_props_dict.get(b"D") {
        Some(PdfObject::Dict(d)) => Some(parse_config(doc, d)?),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let resolved = doc.resolve(&r)?.clone();
            if let Some(d) = resolved.as_dict() {
                Some(parse_config(doc, d)?)
            } else {
                None
            }
        }
        _ => None,
    };

    // Parse /Configs (additional configurations)
    let mut configs = Vec::new();
    let configs_arr = get_array_from_dict(doc, &oc_props_dict, b"Configs")?;
    for item in &configs_arr {
        match item {
            PdfObject::Dict(d) => {
                configs.push(parse_config(doc, d)?);
            }
            PdfObject::Reference(r) => {
                let r = r.clone();
                let resolved = doc.resolve(&r)?.clone();
                if let Some(d) = resolved.as_dict() {
                    configs.push(parse_config(doc, d)?);
                }
            }
            _ => {}
        }
    }

    Ok(Some(OCProperties {
        groups,
        default_config,
        configs,
    }))
}

/// Helper to get an array from a dict, resolving indirect refs.
fn get_array_from_dict(
    doc: &mut PdfDocument,
    dict: &PdfDict,
    key: &[u8],
) -> Result<Vec<PdfObject>> {
    match dict.get(key) {
        Some(PdfObject::Array(arr)) => Ok(arr.clone()),
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let resolved = doc.resolve(&r)?.clone();
            match resolved.as_array() {
                Some(arr) => Ok(arr.to_vec()),
                None => Ok(Vec::new()),
            }
        }
        _ => Ok(Vec::new()),
    }
}

/// Parse a single OCG dictionary.
fn parse_ocg(dict: &PdfDict, obj_ref: IndirectRef) -> OCGroup {
    let name = dict
        .get(b"Name")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();

    let intent = match dict.get(b"Intent") {
        Some(PdfObject::Name(n)) => {
            vec![String::from_utf8_lossy(n).into_owned()]
        }
        Some(PdfObject::Array(arr)) => arr
            .iter()
            .filter_map(|o| o.as_name())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .collect(),
        _ => vec!["View".to_string()],
    };

    let usage = match dict.get_dict(b"Usage") {
        Some(u) => parse_usage(u),
        None => OCGUsage::default(),
    };

    OCGroup {
        obj_ref,
        name,
        intent,
        usage,
    }
}

/// Parse OCG usage dict.
fn parse_usage(dict: &PdfDict) -> OCGUsage {
    OCGUsage {
        print: parse_usage_state(dict, b"Print", b"PrintState"),
        view: parse_usage_state(dict, b"View", b"ViewState"),
        export: parse_usage_state(dict, b"Export", b"ExportState"),
    }
}

/// Parse a single usage category sub-dict.
/// E.g., /Print << /PrintState /ON >> or /View << /ViewState /OFF >>
fn parse_usage_state(dict: &PdfDict, category_key: &[u8], state_key: &[u8]) -> Option<OCGState> {
    let sub_dict = dict.get_dict(category_key)?;
    let state_name = sub_dict.get_name(state_key)?;
    match state_name {
        b"ON" => Some(OCGState::On),
        b"OFF" => Some(OCGState::Off),
        _ => None,
    }
}

/// Parse an OC configuration dict.
fn parse_config(doc: &mut PdfDocument, dict: &PdfDict) -> Result<OCConfig> {
    let name = dict
        .get(b"Name")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let creator = dict
        .get(b"Creator")
        .and_then(|o| o.as_str())
        .map(|b| String::from_utf8_lossy(b).into_owned());

    let base_state = match dict.get_name(b"BaseState") {
        Some(b"OFF") => OCGState::Off,
        _ => OCGState::On, // default is ON per PDF spec
    };

    let on_groups = collect_ref_array(doc, dict, b"ON")?;
    let off_groups = collect_ref_array(doc, dict, b"OFF")?;

    let order = match dict.get(b"Order") {
        Some(PdfObject::Array(arr)) => {
            let arr = arr.clone();
            parse_order(doc, &arr)?
        }
        Some(PdfObject::Reference(r)) => {
            let r = r.clone();
            let resolved = doc.resolve(&r)?.clone();
            match resolved.as_array() {
                Some(arr) => {
                    let arr = arr.to_vec();
                    parse_order(doc, &arr)?
                }
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };

    Ok(OCConfig {
        name,
        creator,
        base_state,
        on_groups,
        off_groups,
        order,
    })
}

/// Collect indirect references from an array entry in a dict.
fn collect_ref_array(
    doc: &mut PdfDocument,
    dict: &PdfDict,
    key: &[u8],
) -> Result<Vec<IndirectRef>> {
    let arr = get_array_from_dict(doc, dict, key)?;
    let mut refs = Vec::new();
    for item in &arr {
        if let Some(r) = item.as_reference() {
            refs.push(r.clone());
        }
    }
    Ok(refs)
}

/// Parse the /Order array into OCOrderItems.
///
/// The order array can contain:
/// - Indirect refs to OCGs
/// - Strings (labels for the following sub-array)
/// - Sub-arrays (grouped OCGs, optionally preceded by a label string)
fn parse_order(doc: &mut PdfDocument, arr: &[PdfObject]) -> Result<Vec<OCOrderItem>> {
    let mut items = Vec::new();
    let mut i = 0;

    while i < arr.len() {
        match &arr[i] {
            PdfObject::Reference(r) => {
                items.push(OCOrderItem::Group(r.clone()));
                i += 1;
            }
            PdfObject::String(label) => {
                // A string label may precede a sub-array
                let name = Some(String::from_utf8_lossy(label).into_owned());
                if i + 1 < arr.len() {
                    if let Some(sub_arr) = arr[i + 1].as_array() {
                        let sub_arr = sub_arr.to_vec();
                        let children = parse_order(doc, &sub_arr)?;
                        items.push(OCOrderItem::SubGroup { name, children });
                        i += 2;
                        continue;
                    }
                }
                // String without following array: create empty sub-group
                items.push(OCOrderItem::SubGroup {
                    name,
                    children: Vec::new(),
                });
                i += 1;
            }
            PdfObject::Array(sub_arr) => {
                let sub_arr = sub_arr.clone();
                let children = parse_order(doc, &sub_arr)?;
                items.push(OCOrderItem::SubGroup {
                    name: None,
                    children,
                });
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(items)
}

/// Determine visibility of a specific OCG given a configuration.
pub fn is_ocg_visible(ocg_ref: &IndirectRef, config: &OCConfig) -> bool {
    match config.base_state {
        OCGState::On => !config.off_groups.contains(ocg_ref),
        OCGState::Off => config.on_groups.contains(ocg_ref),
    }
}

/// Determine visibility of an OCMD given a configuration.
pub fn is_ocmd_visible(ocmd: &OCMembership, config: &OCConfig) -> bool {
    if ocmd.groups.is_empty() {
        // Per PDF spec, if no groups are specified, content is visible
        return true;
    }

    match ocmd.policy {
        VisibilityPolicy::AllOn => ocmd.groups.iter().all(|g| is_ocg_visible(g, config)),
        VisibilityPolicy::AnyOn => ocmd.groups.iter().any(|g| is_ocg_visible(g, config)),
        VisibilityPolicy::AllOff => ocmd.groups.iter().all(|g| !is_ocg_visible(g, config)),
        VisibilityPolicy::AnyOff => ocmd.groups.iter().any(|g| !is_ocg_visible(g, config)),
    }
}

/// Parse an OCMD (Optional Content Membership Dictionary).
///
/// An OCMD contains:
/// - /OCGs: a single OCG reference or an array of OCG references
/// - /P: visibility policy name (AllOn, AnyOn, AllOff, AnyOff)
pub fn parse_ocmd(dict: &PdfDict) -> Option<OCMembership> {
    // Verify this is an OCMD (optional check, some PDFs omit /Type)
    if let Some(type_name) = dict.get_name(b"Type") {
        if type_name != b"OCMD" {
            return None;
        }
    }

    // /OCGs can be a single reference or an array
    let groups = match dict.get(b"OCGs") {
        Some(PdfObject::Reference(r)) => vec![r.clone()],
        Some(PdfObject::Array(arr)) => arr
            .iter()
            .filter_map(|o| o.as_reference())
            .cloned()
            .collect(),
        _ => return None,
    };

    let policy = dict
        .get_name(b"P")
        .map(VisibilityPolicy::from_name)
        .unwrap_or(VisibilityPolicy::AnyOn);

    Some(OCMembership { groups, policy })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ref(num: u32) -> IndirectRef {
        IndirectRef {
            obj_num: num,
            gen_num: 0,
        }
    }

    fn make_default_on_config(off: Vec<IndirectRef>) -> OCConfig {
        OCConfig {
            name: None,
            creator: None,
            base_state: OCGState::On,
            on_groups: Vec::new(),
            off_groups: off,
            order: Vec::new(),
        }
    }

    fn make_default_off_config(on: Vec<IndirectRef>) -> OCConfig {
        OCConfig {
            name: None,
            creator: None,
            base_state: OCGState::Off,
            on_groups: on,
            off_groups: Vec::new(),
            order: Vec::new(),
        }
    }

    // --- is_ocg_visible tests ---

    #[test]
    fn test_ocg_visible_base_on_not_in_off() {
        let config = make_default_on_config(vec![make_ref(10)]);
        assert!(is_ocg_visible(&make_ref(5), &config));
    }

    #[test]
    fn test_ocg_not_visible_base_on_in_off() {
        let config = make_default_on_config(vec![make_ref(5)]);
        assert!(!is_ocg_visible(&make_ref(5), &config));
    }

    #[test]
    fn test_ocg_visible_base_off_in_on() {
        let config = make_default_off_config(vec![make_ref(5)]);
        assert!(is_ocg_visible(&make_ref(5), &config));
    }

    #[test]
    fn test_ocg_not_visible_base_off_not_in_on() {
        let config = make_default_off_config(vec![make_ref(10)]);
        assert!(!is_ocg_visible(&make_ref(5), &config));
    }

    // --- is_ocmd_visible tests ---

    #[test]
    fn test_ocmd_empty_groups_visible() {
        let config = make_default_on_config(Vec::new());
        let ocmd = OCMembership {
            groups: Vec::new(),
            policy: VisibilityPolicy::AllOn,
        };
        assert!(is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_all_on_all_visible() {
        let config = make_default_on_config(Vec::new());
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AllOn,
        };
        assert!(is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_all_on_one_off() {
        let config = make_default_on_config(vec![make_ref(2)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AllOn,
        };
        assert!(!is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_any_on_one_visible() {
        let config = make_default_on_config(vec![make_ref(1)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AnyOn,
        };
        assert!(is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_any_on_none_visible() {
        let config = make_default_on_config(vec![make_ref(1), make_ref(2)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AnyOn,
        };
        assert!(!is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_all_off() {
        let config = make_default_on_config(vec![make_ref(1), make_ref(2)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AllOff,
        };
        assert!(is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_all_off_one_on() {
        let config = make_default_on_config(vec![make_ref(1)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AllOff,
        };
        assert!(!is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_any_off() {
        let config = make_default_on_config(vec![make_ref(1)]);
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AnyOff,
        };
        assert!(is_ocmd_visible(&ocmd, &config));
    }

    #[test]
    fn test_ocmd_any_off_none_off() {
        let config = make_default_on_config(Vec::new());
        let ocmd = OCMembership {
            groups: vec![make_ref(1), make_ref(2)],
            policy: VisibilityPolicy::AnyOff,
        };
        assert!(!is_ocmd_visible(&ocmd, &config));
    }

    // --- parse_ocmd tests ---

    #[test]
    fn test_parse_ocmd_single_ref() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"OCMD".to_vec()));
        dict.insert(
            b"OCGs".to_vec(),
            PdfObject::Reference(make_ref(5)),
        );
        dict.insert(b"P".to_vec(), PdfObject::Name(b"AllOn".to_vec()));

        let ocmd = parse_ocmd(&dict).unwrap();
        assert_eq!(ocmd.groups.len(), 1);
        assert_eq!(ocmd.groups[0], make_ref(5));
        assert_eq!(ocmd.policy, VisibilityPolicy::AllOn);
    }

    #[test]
    fn test_parse_ocmd_array_refs() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"OCGs".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Reference(make_ref(5)),
                PdfObject::Reference(make_ref(6)),
            ]),
        );

        let ocmd = parse_ocmd(&dict).unwrap();
        assert_eq!(ocmd.groups.len(), 2);
        assert_eq!(ocmd.policy, VisibilityPolicy::AnyOn); // default
    }

    #[test]
    fn test_parse_ocmd_no_ocgs() {
        let dict = PdfDict::new();
        assert!(parse_ocmd(&dict).is_none());
    }

    #[test]
    fn test_parse_ocmd_wrong_type() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"OCG".to_vec()));
        dict.insert(
            b"OCGs".to_vec(),
            PdfObject::Reference(make_ref(1)),
        );
        assert!(parse_ocmd(&dict).is_none());
    }

    #[test]
    fn test_parse_ocmd_with_all_policies() {
        for (name, expected) in [
            (b"AllOn".as_slice(), VisibilityPolicy::AllOn),
            (b"AnyOn", VisibilityPolicy::AnyOn),
            (b"AllOff", VisibilityPolicy::AllOff),
            (b"AnyOff", VisibilityPolicy::AnyOff),
        ] {
            let mut dict = PdfDict::new();
            dict.insert(
                b"OCGs".to_vec(),
                PdfObject::Reference(make_ref(1)),
            );
            dict.insert(b"P".to_vec(), PdfObject::Name(name.to_vec()));

            let ocmd = parse_ocmd(&dict).unwrap();
            assert_eq!(ocmd.policy, expected);
        }
    }

    // --- parse_ocg tests ---

    #[test]
    fn test_parse_ocg_basic() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"OCG".to_vec()));
        dict.insert(b"Name".to_vec(), PdfObject::String(b"Layer 1".to_vec()));

        let ocg = parse_ocg(&dict, make_ref(10));
        assert_eq!(ocg.name, "Layer 1");
        assert_eq!(ocg.obj_ref, make_ref(10));
        assert_eq!(ocg.intent, vec!["View".to_string()]); // default
    }

    #[test]
    fn test_parse_ocg_with_intent_name() {
        let mut dict = PdfDict::new();
        dict.insert(b"Name".to_vec(), PdfObject::String(b"Design Layer".to_vec()));
        dict.insert(b"Intent".to_vec(), PdfObject::Name(b"Design".to_vec()));

        let ocg = parse_ocg(&dict, make_ref(1));
        assert_eq!(ocg.intent, vec!["Design".to_string()]);
    }

    #[test]
    fn test_parse_ocg_with_intent_array() {
        let mut dict = PdfDict::new();
        dict.insert(b"Name".to_vec(), PdfObject::String(b"Multi".to_vec()));
        dict.insert(
            b"Intent".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"View".to_vec()),
                PdfObject::Name(b"Design".to_vec()),
            ]),
        );

        let ocg = parse_ocg(&dict, make_ref(1));
        assert_eq!(ocg.intent, vec!["View".to_string(), "Design".to_string()]);
    }

    #[test]
    fn test_parse_ocg_with_usage() {
        let mut print_dict = PdfDict::new();
        print_dict.insert(b"PrintState".to_vec(), PdfObject::Name(b"ON".to_vec()));

        let mut view_dict = PdfDict::new();
        view_dict.insert(b"ViewState".to_vec(), PdfObject::Name(b"OFF".to_vec()));

        let mut usage_dict = PdfDict::new();
        usage_dict.insert(b"Print".to_vec(), PdfObject::Dict(print_dict));
        usage_dict.insert(b"View".to_vec(), PdfObject::Dict(view_dict));

        let mut dict = PdfDict::new();
        dict.insert(b"Name".to_vec(), PdfObject::String(b"PrintOnly".to_vec()));
        dict.insert(b"Usage".to_vec(), PdfObject::Dict(usage_dict));

        let ocg = parse_ocg(&dict, make_ref(1));
        assert_eq!(ocg.usage.print, Some(OCGState::On));
        assert_eq!(ocg.usage.view, Some(OCGState::Off));
        assert_eq!(ocg.usage.export, None);
    }

    // --- parse_order tests (via parse_config) ---

    #[test]
    fn test_parse_config_basic() {
        let mut dict = PdfDict::new();
        dict.insert(b"Name".to_vec(), PdfObject::String(b"Default".to_vec()));
        dict.insert(b"Creator".to_vec(), PdfObject::String(b"TestApp".to_vec()));
        dict.insert(b"BaseState".to_vec(), PdfObject::Name(b"ON".to_vec()));

        // Use a minimal PdfDocument to satisfy the signature
        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let config = parse_config(&mut doc, &dict).unwrap();
        assert_eq!(config.name, Some("Default".to_string()));
        assert_eq!(config.creator, Some("TestApp".to_string()));
        assert_eq!(config.base_state, OCGState::On);
        assert!(config.on_groups.is_empty());
        assert!(config.off_groups.is_empty());
        assert!(config.order.is_empty());
    }

    #[test]
    fn test_parse_config_base_state_off() {
        let mut dict = PdfDict::new();
        dict.insert(b"BaseState".to_vec(), PdfObject::Name(b"OFF".to_vec()));

        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let config = parse_config(&mut doc, &dict).unwrap();
        assert_eq!(config.base_state, OCGState::Off);
    }

    #[test]
    fn test_parse_config_default_base_state() {
        let dict = PdfDict::new();

        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let config = parse_config(&mut doc, &dict).unwrap();
        assert_eq!(config.base_state, OCGState::On); // default
    }

    #[test]
    fn test_parse_order_refs_only() {
        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let arr = vec![
            PdfObject::Reference(make_ref(5)),
            PdfObject::Reference(make_ref(6)),
        ];

        let items = parse_order(&mut doc, &arr).unwrap();
        assert_eq!(items.len(), 2);
        match &items[0] {
            OCOrderItem::Group(r) => assert_eq!(r, &make_ref(5)),
            _ => panic!("expected Group"),
        }
    }

    #[test]
    fn test_parse_order_with_sub_group() {
        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let arr = vec![
            PdfObject::String(b"Background".to_vec()),
            PdfObject::Array(vec![
                PdfObject::Reference(make_ref(5)),
                PdfObject::Reference(make_ref(6)),
            ]),
        ];

        let items = parse_order(&mut doc, &arr).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            OCOrderItem::SubGroup { name, children } => {
                assert_eq!(name.as_deref(), Some("Background"));
                assert_eq!(children.len(), 2);
            }
            _ => panic!("expected SubGroup"),
        }
    }

    #[test]
    fn test_parse_order_unnamed_sub_group() {
        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let arr = vec![PdfObject::Array(vec![
            PdfObject::Reference(make_ref(7)),
        ])];

        let items = parse_order(&mut doc, &arr).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            OCOrderItem::SubGroup { name, children } => {
                assert!(name.is_none());
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected SubGroup"),
        }
    }

    #[test]
    fn test_parse_order_label_without_array() {
        let pdf_bytes = build_minimal_pdf_with_ocg();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

        let arr = vec![
            PdfObject::String(b"Orphan Label".to_vec()),
            PdfObject::Reference(make_ref(1)),
        ];

        let items = parse_order(&mut doc, &arr).unwrap();
        // "Orphan Label" followed by a ref, not an array: creates empty sub-group + group
        assert_eq!(items.len(), 2);
        match &items[0] {
            OCOrderItem::SubGroup { name, children } => {
                assert_eq!(name.as_deref(), Some("Orphan Label"));
                assert!(children.is_empty());
            }
            _ => panic!("expected SubGroup"),
        }
    }

    // --- Integration: parse full OC properties ---

    #[test]
    fn test_read_oc_properties_none() {
        let pdf_bytes = build_minimal_pdf();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
        let props = read_oc_properties(&mut doc).unwrap();
        assert!(props.is_none());
    }

    #[test]
    fn test_read_oc_properties_with_ocg() {
        let pdf_bytes = build_pdf_with_oc_properties();
        let mut doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
        let props = read_oc_properties(&mut doc).unwrap();
        assert!(props.is_some());

        let props = props.unwrap();
        assert_eq!(props.groups.len(), 2);
        assert_eq!(props.groups[0].name, "Layer 1");
        assert_eq!(props.groups[1].name, "Layer 2");
        assert!(props.default_config.is_some());

        let config = props.default_config.unwrap();
        assert_eq!(config.base_state, OCGState::On);
    }

    // --- Test helpers ---

    /// Build a minimal valid PDF for tests that need a PdfDocument but don't
    /// read from it.
    fn build_minimal_pdf_with_ocg() -> Vec<u8> {
        build_minimal_pdf()
    }

    fn build_minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.5\n");

        let obj1_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        let obj2_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

        let obj3_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n0 4\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());

        pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
        pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());

        pdf
    }

    /// Build a PDF with OCProperties in the catalog for integration testing.
    fn build_pdf_with_oc_properties() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.5\n");

        // Object 4: OCG "Layer 1"
        let obj4_offset = pdf.len();
        pdf.extend_from_slice(
            b"4 0 obj\n<< /Type /OCG /Name (Layer 1) >>\nendobj\n",
        );

        // Object 5: OCG "Layer 2"
        let obj5_offset = pdf.len();
        pdf.extend_from_slice(
            b"5 0 obj\n<< /Type /OCG /Name (Layer 2) >>\nendobj\n",
        );

        // Object 1: Catalog with OCProperties
        let obj1_offset = pdf.len();
        pdf.extend_from_slice(
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [4 0 R 5 0 R] /D << /BaseState /ON /Order [4 0 R 5 0 R] >> >> >>\nendobj\n",
        );

        // Object 2: Pages
        let obj2_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

        // Object 3: Page
        let obj3_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n0 6\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj4_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj5_offset).as_bytes());

        pdf.extend_from_slice(b"trailer\n<< /Size 6 /Root 1 0 R >>\n");
        pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());

        pdf
    }
}
