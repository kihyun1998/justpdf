//! Visit budget for walks over PDF object trees.

use std::collections::HashMap;

use crate::error::{JustPdfError, Result};
use crate::object::{IndirectRef, PdfObject};

/// Times a walk may read, in total, the size of the distinct nodes it has
/// resolved.
const READS_PER_NODE: usize = 4;

/// Charges one tree walk's node visits by the size of each node read,
/// against the summed size of the distinct nodes it has resolved. A visit
/// that would pass `READS_PER_NODE` times that sum is refused and not
/// charged; a node already resolved is refused without resolving it again.
#[derive(Default)]
pub(crate) struct VisitBudget {
    sizes: HashMap<IndirectRef, usize>,
    resolved_size: usize,
    charged: usize,
}

impl VisitBudget {
    /// Visits `node`, resolving it with `resolve`. `None` when the visit is
    /// refused.
    pub(crate) fn visit(
        &mut self,
        node: &IndirectRef,
        resolve: impl FnOnce() -> Result<PdfObject>,
    ) -> Result<Option<PdfObject>> {
        if let Some(&size) = self.sizes.get(node) {
            if !self.charge(size) {
                return Ok(None);
            }
            return resolve().map(Some);
        }
        let obj = resolve()?;
        let size = object_size(&obj);
        self.sizes.insert(node.clone(), size);
        self.resolved_size += size;
        Ok(self.charge(size).then_some(obj))
    }

    /// Visits `node`, resolving it with `resolve`; `LimitExceeded` when the
    /// visit is refused.
    pub(crate) fn enter(
        &mut self,
        node: &IndirectRef,
        resolve: impl FnOnce() -> Result<PdfObject>,
    ) -> Result<PdfObject> {
        self.visit(node, resolve)?
            .ok_or(JustPdfError::LimitExceeded {
                obj_num: node.obj_num,
                gen_num: node.gen_num,
            })
    }

    /// Charges `size` when it keeps the walk within budget.
    fn charge(&mut self, size: usize) -> bool {
        let within = self.charged + size <= READS_PER_NODE * self.resolved_size;
        if within {
            self.charged += size;
        }
        within
    }
}

/// The number of objects in `obj`, counting itself and every array element
/// and dictionary value it holds directly or through nested arrays and
/// dictionaries. A stream counts its dictionary, not its data.
fn object_size(obj: &PdfObject) -> usize {
    1 + match obj {
        PdfObject::Array(items) => items.iter().map(object_size).sum(),
        PdfObject::Dict(dict) => dict.iter().map(|(_, value)| object_size(value)).sum(),
        PdfObject::Stream { dict, .. } => dict.iter().map(|(_, value)| object_size(value)).sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(obj_num: u32) -> IndirectRef {
        IndirectRef {
            obj_num,
            gen_num: 0,
        }
    }

    #[test]
    fn a_refused_revisit_does_not_resolve_the_node() {
        let big = PdfObject::Array(vec![PdfObject::Integer(0); 100]);
        let mut budget = VisitBudget::default();
        let mut resolved = 0;
        let mut refused = 0;
        for _ in 0..10 {
            let visit = budget.visit(&node(1), || {
                resolved += 1;
                Ok(big.clone())
            });
            if visit.unwrap().is_none() {
                refused += 1;
            }
        }
        assert_eq!((resolved, refused), (4, 6));
    }
}
