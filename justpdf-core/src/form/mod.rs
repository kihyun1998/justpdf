pub mod appearance;
pub mod fill;
pub mod flatten;
pub mod parse;
pub mod types;

pub use fill::{get_field_value, set_field_value, toggle_checkbox};
pub use flatten::flatten_form;
pub use parse::parse_acroform;
pub use types::*;
