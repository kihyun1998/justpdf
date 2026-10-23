//! Where an annotation's appearance is drawn (ISO 32000-2 §12.5.5): which
//! appearance stream applies and the matrix that maps it onto `/Rect`.

use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::page::Rect;

/// A form XObject dictionary's `/Matrix` as `[a b c d e f]`, identity when
/// absent or short.
pub fn form_matrix(form: &PdfDict) -> [f64; 6] {
    match form.get_array(b"Matrix") {
        Some(m) if m.len() >= 6 => {
            let n = |i: usize, default: f64| m[i].as_f64().unwrap_or(default);
            [
                n(0, 1.0),
                n(1, 0.0),
                n(2, 0.0),
                n(3, 1.0),
                n(4, 0.0),
                n(5, 0.0),
            ]
        }
        _ => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    }
}

/// The matrix A of §12.5.5 as `[a b c d e f]`: it scales and translates the
/// appearance form's `/BBox`, transformed by its `/Matrix`, onto `rect`. The
/// form is then drawn with `Matrix × A` on top of the CTM. `None` without a
/// `/BBox`, or when that box has no width or height.
pub fn appearance_matrix(form: &PdfDict, rect: &Rect) -> Option<[f64; 6]> {
    let bbox = form.get_array(b"BBox").and_then(Rect::from_pdf_array)?;
    let [a, b, c, d, e, f] = form_matrix(form);
    let corners = [
        (bbox.llx, bbox.lly),
        (bbox.urx, bbox.lly),
        (bbox.llx, bbox.ury),
        (bbox.urx, bbox.ury),
    ]
    .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in corners {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
        return None;
    }
    let (sx, sy) = (rect.width() / w, rect.height() / h);
    Some([sx, 0.0, 0.0, sy, rect.llx - x0 * sx, rect.lly - y0 * sy])
}

/// The normal appearance of `annot`: `/AP /N` when it is a stream, else the
/// entry of that state dictionary its `/AS` names, as (its reference when
/// indirect, the stream). `None` when there is none. `resolve` resolves
/// references.
pub fn normal_appearance(
    annot: &PdfDict,
    resolve: impl Fn(&IndirectRef) -> Option<PdfObject>,
) -> Option<(Option<IndirectRef>, PdfObject)> {
    let direct = |o: &PdfObject| match o {
        PdfObject::Reference(r) => Some((Some(r.clone()), resolve(r)?)),
        other => Some((None, other.clone())),
    };
    let (_, ap) = direct(annot.get(b"AP")?)?;
    let (mut obj_ref, mut normal) = direct(ap.as_dict()?.get(b"N")?)?;
    if let PdfObject::Dict(states) = &normal {
        let state = states.get(annot.get_name(b"AS")?)?.clone();
        (obj_ref, normal) = direct(&state)?;
    }
    matches!(normal, PdfObject::Stream { .. }).then_some((obj_ref, normal))
}

/// Whether `annot`'s `/F` has Hidden (bit 2) or NoView (bit 6) set.
pub fn is_hidden(annot: &PdfDict) -> bool {
    let flags = annot.get_i64(b"F").unwrap_or(0);
    flags & 0x02 != 0 || flags & 0x20 != 0
}
