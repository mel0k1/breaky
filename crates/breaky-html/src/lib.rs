mod dom;
mod parser;

pub use dom::{Node, NodeKind, print_tree};
pub use parser::parse;
