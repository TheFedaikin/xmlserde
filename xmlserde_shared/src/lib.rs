use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
    ToUpperCamelCase,
};

#[derive(Debug, Clone, Copy)]
pub enum Case {
    None,
    Lowercase,
    Uppercase,
    CamelCase,
    PascalCase,
    SnakeCase,
    KebabCase,
    ShoutySnakeCase,
    ShoutyKebabCase,
}

impl From<&str> for Case {
    fn from(s: &str) -> Self {
        match s {
            | "lowercase" => Case::Lowercase,
            | "UPPERCASE" => Case::Uppercase,
            | "PascalCase" => Case::PascalCase,
            | "camelCase" => Case::CamelCase,
            | "snake_case" => Case::SnakeCase,
            | "kebab-case" => Case::KebabCase,
            | "SCREAMING_SNAKE_CASE" => Case::ShoutySnakeCase,
            | "SCREAMING-KEBAB-CASE" => Case::ShoutyKebabCase,
            | _ => Case::None,
        }
    }
}

impl Case {
    pub fn to_rename_all_variant(&self) -> &'static str {
        match self {
            | Case::None => "None",
            | Case::Lowercase => "Lowercase",
            | Case::Uppercase => "Uppercase",
            | Case::CamelCase => "CamelCase",
            | Case::PascalCase => "PascalCase",
            | Case::SnakeCase => "SnakeCase",
            | Case::KebabCase => "KebabCase",
            | Case::ShoutySnakeCase => "ShoutySnakeCase",
            | Case::ShoutyKebabCase => "ShoutyKebabCase",
        }
    }

    pub fn transform(&self, name: &[u8]) -> Vec<u8> {
        let name_str = String::from_utf8_lossy(name);
        let transformed = match self {
            | Case::None => name_str.to_string(),
            | Case::Lowercase => name_str.to_lowercase(),
            | Case::Uppercase => name_str.to_uppercase(),
            | Case::CamelCase => name_str.to_lower_camel_case(),
            | Case::PascalCase => name_str.to_upper_camel_case(),
            | Case::SnakeCase => name_str.to_snake_case(),
            | Case::KebabCase => name_str.to_kebab_case(),
            | Case::ShoutySnakeCase => name_str.to_shouty_snake_case(),
            | Case::ShoutyKebabCase => name_str.to_shouty_kebab_case(),
        };
        transformed.into_bytes()
    }

    pub fn convert(&self, input: &str) -> String {
        match self {
            | Case::None => input.to_string(),
            | Case::Lowercase => input.to_lowercase(),
            | Case::Uppercase => input.to_uppercase(),
            | Case::CamelCase => input.to_lower_camel_case(),
            | Case::PascalCase => input.to_upper_camel_case(),
            | Case::SnakeCase => input.to_snake_case(),
            | Case::KebabCase => input.to_kebab_case(),
            | Case::ShoutySnakeCase => input.to_shouty_snake_case(),
            | Case::ShoutyKebabCase => input.to_shouty_kebab_case(),
        }
    }
}
