use syn::LitStr;

use xmlserde_shared::Case;

pub fn parse_case(lit: &LitStr) -> Option<Case> {
    match lit.value().as_str() {
        | "none" => Some(Case::None),
        | "lowercase" => Some(Case::Lowercase),
        | "UPPERCASE" => Some(Case::Uppercase),
        | "camelCase" => Some(Case::CamelCase),
        | "PascalCase" => Some(Case::PascalCase),
        | "snake_case" => Some(Case::SnakeCase),
        | "kebab-case" => Some(Case::KebabCase),
        | "SCREAMING_SNAKE_CASE" => Some(Case::ShoutySnakeCase),
        | "SCREAMING-KEBAB-CASE" => Some(Case::ShoutyKebabCase),
        | _ => None,
    }
}
