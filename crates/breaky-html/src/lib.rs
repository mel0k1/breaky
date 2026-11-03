mod dom;
mod entities;
mod parser;

pub use dom::{Node, NodeKind, print_tree};
pub use entities::decode;
pub use parser::parse;
