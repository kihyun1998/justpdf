use std::collections::HashMap;

use crate::object::{PdfDict, PdfObject};

/// Statistics from the cleanup process.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanStats {
    pub duplicate_objects_removed: usize,
    pub empty_objects_removed: usize,
    pub total_objects_before: usize,
    pub total_objects_after: usize,
}

/// Clean and optimize a PDF document's object list.
///
/// Performs the following operations:
/// 1. Removes duplicate objects (identical content), rewriting references
/// 2. Removes null/empty objects
/// 3. Compacts object numbers sequentially to eliminate gaps
///
/// Returns statistics about what was cleaned.
pub fn clean_objects(objects: &mut Vec<(u32, PdfObject)>) -> CleanStats {
    let total_before = objects.len();

    // Step 1: Remove duplicates by hashing object content
    let dups_removed = dedup_objects(objects);

    // Step 2: Remove null objects
    let nulls_removed = remove_null_objects(objects);

    // Step 3: Compact object numbers
    compact_object_numbers(objects);

    CleanStats {
        duplicate_objects_removed: dups_removed,
        empty_objects_removed: nulls_removed,
        total_objects_before: total_before,
        total_objects_after: objects.len(),
    }
}

/// Hash a PdfObject to a canonical string for deduplication.
/// This uses the Display representation which is deterministic for our types.
fn hash_object(obj: &PdfObject) -> String {
    // Use format! which delegates to Display
    format!("{}", obj)
}

/// Remove duplicate objects: when two objects have identical content,
/// keep the first and rewrite all references to point to it.
/// Returns the number of duplicates removed.
fn dedup_objects(objects: &mut Vec<(u32, PdfObject)>) -> usize {
    // Build a map from object hash -> first object number with that hash
    let mut hash_to_first: HashMap<String, u32> = HashMap::new();
    // Map from removed obj_num -> replacement obj_num
    let mut remap: HashMap<u32, u32> = HashMap::new();

    for (obj_num, obj) in objects.iter() {
        let h = hash_object(obj);
        match hash_to_first.get(&h) {
            Some(&first_num) if first_num != *obj_num => {
                remap.insert(*obj_num, first_num);
            }
            _ => {
                hash_to_first.insert(h, *obj_num);
            }
        }
    }

    if remap.is_empty() {
        return 0;
    }

    let removed = remap.len();

    // Remove the duplicate objects
    objects.retain(|(obj_num, _)| !remap.contains_key(obj_num));

    // Rewrite all references in remaining objects
    for (_, obj) in objects.iter_mut() {
        rewrite_references(obj, &remap);
    }

    removed
}

/// Recursively rewrite indirect references according to the remap table.
fn rewrite_references(obj: &mut PdfObject, remap: &HashMap<u32, u32>) {
    match obj {
        PdfObject::Reference(r) => {
            if let Some(&new_num) = remap.get(&r.obj_num) {
                r.obj_num = new_num;
            }
        }
        PdfObject::Array(items) => {
            for item in items.iter_mut() {
                rewrite_references(item, remap);
            }
        }
        PdfObject::Dict(dict) => {
            rewrite_references_in_dict(dict, remap);
        }
        PdfObject::Stream { dict, .. } => {
            rewrite_references_in_dict(dict, remap);
        }
        _ => {}
    }
}

/// Rewrite references within a dictionary.
fn rewrite_references_in_dict(dict: &mut PdfDict, remap: &HashMap<u32, u32>) {
    // We need to collect keys first since we can't mutate while iterating
    let keys: Vec<Vec<u8>> = dict.keys().cloned().collect();
    for key in keys {
        if let Some(val) = dict.get(&key) {
            let mut val = val.clone();
            rewrite_references(&mut val, remap);
            dict.insert(key, val);
        }
    }
}

/// Remove null objects from the list. Returns the number removed.
fn remove_null_objects(objects: &mut Vec<(u32, PdfObject)>) -> usize {
    // Collect object numbers that are referenced by other objects
    let mut referenced: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for (_, obj) in objects.iter() {
        collect_references(obj, &mut referenced);
    }

    let before = objects.len();

    // Remove null objects that are not referenced
    objects.retain(|(obj_num, obj)| {
        if obj.is_null() && !referenced.contains(obj_num) {
            false
        } else {
            true
        }
    });

    before - objects.len()
}

/// Collect all object numbers referenced by indirect references in an object.
fn collect_references(obj: &PdfObject, refs: &mut std::collections::HashSet<u32>) {
    match obj {
        PdfObject::Reference(r) => {
            refs.insert(r.obj_num);
        }
        PdfObject::Array(items) => {
            for item in items {
                collect_references(item, refs);
            }
        }
        PdfObject::Dict(dict) => {
            for (_, val) in dict.iter() {
                collect_references(val, refs);
            }
        }
        PdfObject::Stream { dict, .. } => {
            for (_, val) in dict.iter() {
                collect_references(val, refs);
            }
        }
        _ => {}
    }
}

/// Compact object numbers sequentially starting from 1, rewriting all references.
fn compact_object_numbers(objects: &mut Vec<(u32, PdfObject)>) {
    // Build a mapping from old obj_num -> new obj_num
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for (i, (obj_num, _)) in objects.iter().enumerate() {
        let new_num = (i + 1) as u32;
        if *obj_num != new_num {
            remap.insert(*obj_num, new_num);
        }
    }

    if remap.is_empty() {
        return;
    }

    // Renumber objects
    for (i, (obj_num, _)) in objects.iter_mut().enumerate() {
        *obj_num = (i + 1) as u32;
    }

    // Rewrite references
    for (_, obj) in objects.iter_mut() {
        rewrite_references(obj, &remap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{IndirectRef, PdfDict, PdfObject};

    #[test]
    fn test_clean_removes_duplicates() {
        let mut objects = vec![
            (1, PdfObject::Integer(42)),
            (2, PdfObject::Integer(42)), // duplicate of 1
            (3, PdfObject::Array(vec![
                PdfObject::Reference(IndirectRef { obj_num: 2, gen_num: 0 }),
            ])),
        ];

        let stats = clean_objects(&mut objects);

        assert_eq!(stats.duplicate_objects_removed, 1);
        assert_eq!(stats.total_objects_before, 3);
        assert_eq!(stats.total_objects_after, 2);

        // Object 2 should be removed, and the reference in object 3 should now point to 1
        assert_eq!(objects.len(), 2);

        // After compaction, the array (originally obj 3) should reference obj 1
        let arr_obj = objects.iter().find(|(_, obj)| obj.is_array());
        assert!(arr_obj.is_some());
        if let (_, PdfObject::Array(items)) = arr_obj.unwrap() {
            if let PdfObject::Reference(r) = &items[0] {
                assert_eq!(r.obj_num, 1); // points to the surviving duplicate
            } else {
                panic!("expected reference");
            }
        }
    }

    #[test]
    fn test_clean_removes_null_objects() {
        let mut objects = vec![
            (1, PdfObject::Integer(10)),
            (2, PdfObject::Null), // unreferenced null -> removed
            (3, PdfObject::String(b"hello".to_vec())),
        ];

        let stats = clean_objects(&mut objects);

        assert_eq!(stats.empty_objects_removed, 1);
        assert_eq!(stats.total_objects_after, 2);
    }

    #[test]
    fn test_clean_preserves_referenced_null() {
        let mut objects = vec![
            (1, PdfObject::Reference(IndirectRef { obj_num: 2, gen_num: 0 })),
            (2, PdfObject::Null), // referenced null -> kept
        ];

        let stats = clean_objects(&mut objects);

        assert_eq!(stats.empty_objects_removed, 0);
        assert_eq!(stats.total_objects_after, 2);
    }

    #[test]
    fn test_compact_renumbering() {
        let mut objects = vec![
            (1, PdfObject::Integer(10)),
            (5, PdfObject::Integer(20)),
            (10, PdfObject::Reference(IndirectRef { obj_num: 5, gen_num: 0 })),
        ];

        compact_object_numbers(&mut objects);

        // Should be renumbered to 1, 2, 3
        assert_eq!(objects[0].0, 1);
        assert_eq!(objects[1].0, 2);
        assert_eq!(objects[2].0, 3);

        // Reference should be updated: old 5 -> new 2
        if let PdfObject::Reference(r) = &objects[2].1 {
            assert_eq!(r.obj_num, 2);
        } else {
            panic!("expected reference");
        }
    }

    #[test]
    fn test_compact_already_sequential() {
        let mut objects = vec![
            (1, PdfObject::Integer(10)),
            (2, PdfObject::Integer(20)),
            (3, PdfObject::Integer(30)),
        ];

        compact_object_numbers(&mut objects);

        assert_eq!(objects[0].0, 1);
        assert_eq!(objects[1].0, 2);
        assert_eq!(objects[2].0, 3);
    }

    #[test]
    fn test_clean_stats_correct() {
        let mut objects = vec![
            (1, PdfObject::Integer(42)),
            (2, PdfObject::Integer(42)),  // dup
            (3, PdfObject::Null),          // unreferenced null
            (4, PdfObject::String(b"keep".to_vec())),
            (5, PdfObject::Integer(99)),
        ];

        let stats = clean_objects(&mut objects);

        assert_eq!(stats.total_objects_before, 5);
        assert_eq!(stats.duplicate_objects_removed, 1);
        assert_eq!(stats.empty_objects_removed, 1);
        assert_eq!(stats.total_objects_after, 3); // 5 - 1 dup - 1 null
    }

    #[test]
    fn test_dedup_dict_objects() {
        let mut d1 = PdfDict::new();
        d1.insert(b"Key".to_vec(), PdfObject::Integer(1));
        let mut d2 = PdfDict::new();
        d2.insert(b"Key".to_vec(), PdfObject::Integer(1));

        let mut objects = vec![
            (1, PdfObject::Dict(d1)),
            (2, PdfObject::Dict(d2)),
        ];

        let removed = dedup_objects(&mut objects);
        assert_eq!(removed, 1);
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn test_rewrite_references_nested() {
        let mut remap = HashMap::new();
        remap.insert(5u32, 1u32);

        let mut obj = PdfObject::Array(vec![
            PdfObject::Dict({
                let mut d = PdfDict::new();
                d.insert(
                    b"Ref".to_vec(),
                    PdfObject::Reference(IndirectRef { obj_num: 5, gen_num: 0 }),
                );
                d
            }),
        ]);

        rewrite_references(&mut obj, &remap);

        if let PdfObject::Array(items) = &obj {
            if let PdfObject::Dict(d) = &items[0] {
                if let Some(PdfObject::Reference(r)) = d.get(b"Ref") {
                    assert_eq!(r.obj_num, 1);
                } else {
                    panic!("expected reference");
                }
            }
        }
    }

    #[test]
    fn test_clean_empty_list() {
        let mut objects: Vec<(u32, PdfObject)> = vec![];
        let stats = clean_objects(&mut objects);

        assert_eq!(stats.total_objects_before, 0);
        assert_eq!(stats.total_objects_after, 0);
        assert_eq!(stats.duplicate_objects_removed, 0);
        assert_eq!(stats.empty_objects_removed, 0);
    }
}
