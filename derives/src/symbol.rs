use std::fmt::{self, Display};

use syn::{Ident, Path};

#[derive(Copy, Clone)]
pub struct Symbol(&'static str);

impl Symbol {
    pub fn value(&self) -> &'static str {
        self.0
    }
}

pub const DENY_UNKNOWN: Symbol = Symbol("deny_unknown_fields");
pub const WITH_NS: Symbol = Symbol("with_ns");
pub const WITH_CUSTOM_NS: Symbol = Symbol("with_custom_ns");
pub const ROOT: Symbol = Symbol("root");
pub const XML_SERDE: Symbol = Symbol("xmlserde");
pub const NAME: Symbol = Symbol("name");
pub const TYPE: Symbol = Symbol("ty");
pub const SKIP_SERIALIZING: Symbol = Symbol("skip_serializing");
pub const VEC_SIZE: Symbol = Symbol("vec_size");
pub const DEFAULT: Symbol = Symbol("default");
pub const MAP: Symbol = Symbol("map");

// Type values
pub const TYPE_ATTR: Symbol = Symbol("attr");
pub const TYPE_CHILD: Symbol = Symbol("child");
pub const TYPE_TEXT: Symbol = Symbol("text");
pub const TYPE_SFC: Symbol = Symbol("sfc");
pub const TYPE_UNTAG: Symbol = Symbol("untag");
pub const TYPE_UNTAGGED_ENUM: Symbol = Symbol("untagged_enum");
pub const TYPE_UNTAGGED_STRUCT: Symbol = Symbol("untagged_struct");

// Enum-related attributes
pub const RENAME: Symbol = Symbol("rename");
pub const OTHER: Symbol = Symbol("other");

impl PartialEq<Symbol> for Ident {
    fn eq(&self, other: &Symbol) -> bool {
        self == other.0
    }
}

impl PartialEq<Symbol> for &'_ Ident {
    fn eq(&self, word: &Symbol) -> bool {
        *self == word.0
    }
}

impl PartialEq<Symbol> for Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl PartialEq<Symbol> for &'_ Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
