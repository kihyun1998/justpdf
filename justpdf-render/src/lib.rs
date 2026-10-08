#![doc(
    html_logo_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/icons/justpdf-icon-light-128.png"
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/kihyun1998/justpdf/master/logo/favicon/favicon-32.png"
)]

pub mod bbox_device;
pub mod device;
pub mod display_list;
pub mod error;
pub mod glyph;
pub mod glyph_cache;
pub mod graphics_state;
pub mod interpreter;
pub mod render;
mod resources;
pub mod shading;
mod substitute;
pub mod svg_device;

pub use bbox_device::compute_page_bbox;
pub use error::{RenderError, Result};
pub use render::{
    OutputFormat, RenderOptions, RenderedPixmap, render_page, render_page_to_pixmap,
    render_page_to_svg,
};

#[cfg(feature = "parallel")]
pub use render::{render_all_pages_parallel, render_pages_parallel};
