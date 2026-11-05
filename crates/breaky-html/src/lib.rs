mod dom;
mod entities;
mod parser;
mod serialize;

pub use dom::{Node, NodeKind, print_tree};
pub use entities::decode;
pub use parser::parse;
pub use serialize::inner_html;
