pub mod bbox_device;
pub mod device;
pub mod display_list;
pub mod error;
pub mod glyph;
pub mod glyph_cache;
pub mod graphics_state;
pub mod interpreter;
pub mod render;
pub mod shading;
pub mod svg_device;

pub use bbox_device::compute_page_bbox;
pub use error::{RenderError, Result};
pub use render::{
    render_page, render_page_to_pixmap, render_page_to_svg, OutputFormat, RenderedPixmap,
    RenderOptions,
};

#[cfg(feature = "parallel")]
pub use render::{render_all_pages_parallel, render_pages_parallel};
