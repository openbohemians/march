//! March7's system machine. No source reader, dictionary, or compiler lives here.
//! `symbols` is the formatter's text rewrite; March's reader applies the same
//! table itself.
pub mod code;
pub mod driver;
pub mod image;
pub mod machine;
pub mod symbols;

pub use code::{Blob, Cid, Op, Primitive};
pub use driver::Driver;
pub use image::Image;
pub use machine::{Error, Machine, Stats};
