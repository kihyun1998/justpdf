//! Resource scopes: the `/Resources` dictionaries a content stream's names
//! resolve in, searched innermost first down to the page's.

use std::collections::HashMap;
use std::sync::Arc;

use justpdf_core::PdfDocument;
use justpdf_core::object::{IndirectRef, PdfDict, PdfObject};
use justpdf_core::page::PageInfo;

use crate::error::Result;
use crate::graphics_state::{FontKey, PatternSelection};

/// A resource object found by name.
pub(crate) struct Resource {
    /// The object's reference; `None` for a direct object.
    pub obj_ref: Option<IndirectRef>,
    pub object: PdfObject,
}

/// The font `Tf` selected, with its dictionary when the caller does not
/// hold it yet.
pub(crate) struct FontSelection {
    pub key: FontKey,
    pub dict: Option<PdfDict>,
}

/// A resource dictionary entry as written, and the nearest indirect object
/// holding it.
struct Entry {
    value: PdfObject,
    holder: Option<IndirectRef>,
}

/// One `/Resources` dictionary, the nearest indirect object holding it,
/// and the fonts `Tf` has selected by name while it was innermost.
struct Scope {
    resources: PdfDict,
    holder: Option<IndirectRef>,
    fonts: HashMap<Vec<u8>, Option<FontKey>>,
}

/// The `/Resources` of the content streams being executed, outermost
/// (the page's) first.
#[derive(Default)]
pub(crate) struct ResourceScopes {
    scopes: Vec<Scope>,
}

impl ResourceScopes {
    /// The page's resources as the only scope.
    pub fn for_page(doc: &PdfDocument, page: &PageInfo) -> Self {
        let mut scopes = Self::default();
        if let Some((resources, holder)) = resolve_dict(doc, page.resources_ref.as_ref()) {
            scopes.scopes.push(Scope {
                resources,
                holder: holder.or_else(|| Some(page.page_ref.clone())),
                fonts: HashMap::new(),
            });
        }
        scopes
    }

    /// Enters the scope of the stream with dictionary `stream_dict`: its
    /// `/Resources`, when it has them. Returns whether a scope was entered.
    pub fn enter(
        &mut self,
        doc: &PdfDocument,
        stream_dict: &PdfDict,
        stream_ref: Option<&IndirectRef>,
    ) -> bool {
        match resolve_dict(doc, stream_dict.get(b"Resources")) {
            Some((resources, holder)) => {
                self.scopes.push(Scope {
                    resources,
                    holder: holder.or_else(|| stream_ref.cloned()),
                    fonts: HashMap::new(),
                });
                true
            }
            None => false,
        }
    }

    /// Leaves the scope last entered.
    pub fn leave(&mut self) {
        self.scopes.pop();
    }

    /// The object named `name` in the `category` dictionary (`/XObject`,
    /// `/Font`, …) of the innermost scope that names it.
    pub fn lookup(
        &self,
        doc: &PdfDocument,
        category: &[u8],
        name: &[u8],
    ) -> Result<Option<Resource>> {
        let Some(entry) = self.entry(doc, category, name) else {
            return Ok(None);
        };
        Ok(Some(match entry.value {
            PdfObject::Reference(r) => Resource {
                object: doc.resolve(&r)?,
                obj_ref: Some(r),
            },
            other => Resource {
                obj_ref: None,
                object: other,
            },
        }))
    }

    /// The pattern named `name`, as `scn`/`SCN` selects it.
    pub fn select_pattern(&self, doc: &PdfDocument, name: &[u8]) -> Option<PatternSelection> {
        let found = self.lookup(doc, b"Pattern", name).ok()??;
        Some(PatternSelection {
            obj_ref: found.obj_ref,
            object: Arc::new(found.object),
        })
    }

    /// The font named `name`, as `Tf` selects it. The key is recorded in
    /// the innermost scope; the font dictionary comes back only when
    /// `cached` does not hold the key.
    pub fn select_font(
        &mut self,
        doc: &PdfDocument,
        name: &[u8],
        cached: impl Fn(&FontKey) -> bool,
    ) -> Option<FontSelection> {
        if let Some(recorded) = self.scopes.last().and_then(|s| s.fonts.get(name)) {
            return recorded
                .clone()
                .map(|key| FontSelection { key, dict: None });
        }
        let selection = self.entry(doc, b"Font", name).and_then(|entry| {
            let (key, object) = match entry.value {
                PdfObject::Reference(r) => (FontKey::Object(r.clone()), None),
                direct => (
                    FontKey::Direct {
                        holder: entry.holder,
                        name: name.to_vec(),
                    },
                    Some(direct),
                ),
            };
            if cached(&key) {
                return Some(FontSelection { key, dict: None });
            }
            let object = match (object, &key) {
                (Some(direct), _) => direct,
                (None, FontKey::Object(r)) => doc.resolve(r).ok()?,
                (None, FontKey::Direct { .. }) => return None,
            };
            match object {
                PdfObject::Dict(dict) => Some(FontSelection {
                    key,
                    dict: Some(dict),
                }),
                _ => None,
            }
        });
        if let Some(scope) = self.scopes.last_mut() {
            let key = selection.as_ref().map(|s| s.key.clone());
            scope.fonts.insert(name.to_vec(), key);
        }
        selection
    }

    /// The entry named `name` in the `category` dictionary of the innermost
    /// scope that names it, unresolved. A category dictionary that does not
    /// resolve names nothing.
    fn entry(&self, doc: &PdfDocument, category: &[u8], name: &[u8]) -> Option<Entry> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| match scope.resources.get(category)? {
                PdfObject::Dict(d) => Some(Entry {
                    value: d.get(name)?.clone(),
                    holder: scope.holder.clone(),
                }),
                PdfObject::Reference(r) => match doc.resolve(r).ok()? {
                    PdfObject::Dict(d) => Some(Entry {
                        value: d.get(name)?.clone(),
                        holder: Some(r.clone()),
                    }),
                    _ => None,
                },
                _ => None,
            })
    }
}

/// `obj` as a dictionary, with its reference when it is one.
fn resolve_dict(
    doc: &PdfDocument,
    obj: Option<&PdfObject>,
) -> Option<(PdfDict, Option<IndirectRef>)> {
    match obj? {
        PdfObject::Dict(d) => Some((d.clone(), None)),
        PdfObject::Reference(r) => match doc.resolve(r).ok()? {
            PdfObject::Dict(d) => Some((d, Some(r.clone()))),
            _ => None,
        },
        _ => None,
    }
}
