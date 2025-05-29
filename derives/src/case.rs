use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use syn::LitStr;

pub enum Case {
    Lowercase,
    Uppercase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl Case {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            | "lowercase" => Some(Case::Lowercase),
            | "UPPERCASE" => Some(Case::Uppercase),
            | "PascalCase" => Some(Case::PascalCase),
            | "camelCase" => Some(Case::CamelCase),
            | "snake_case" => Some(Case::SnakeCase),
            | "SCREAMING_SNAKE_CASE" => Some(Case::ScreamingSnakeCase),
            | "kebab-case" => Some(Case::KebabCase),
            | "SCREAMING-KEBAB-CASE" => Some(Case::ScreamingKebabCase),
            | _ => None,
        }
    }

    pub fn convert(&self, s: &str) -> String {
        match self {
            | Case::Lowercase => s.to_lowercase(),
            | Case::Uppercase => s.to_uppercase(),
            | Case::PascalCase => s.to_upper_camel_case(),
            | Case::CamelCase => s.to_lower_camel_case(),
            | Case::SnakeCase => s.to_snake_case(),
            | Case::ScreamingSnakeCase => s.to_shouty_snake_case(),
            | Case::KebabCase => s.to_kebab_case(),
            | Case::ScreamingKebabCase => s.to_kebab_case().to_uppercase(),
        }
    }
}

pub fn parse_case(lit: &LitStr) -> Option<Case> {
    Case::from_str(&lit.value())
}
