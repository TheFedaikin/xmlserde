use proc_macro2::{Group, Span, TokenStream, TokenTree};
use syn::parse::{self, Parse};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::Meta::{self, NameValue};
use syn::{Expr, ExprArray, ExprLit, Lit, Variant};

use crate::case::parse_case;

use crate::symbol::{
    DEFAULT, DENY_UNKNOWN, MAP, NAME, RENAME_ALL, ROOT, SKIP_SERIALIZING, TYPE, TYPE_ATTR,
    TYPE_CHILD, TYPE_SFC, TYPE_TEXT, TYPE_UNTAG, TYPE_UNTAGGED_ENUM, TYPE_UNTAGGED_STRUCT,
    VEC_SIZE, WITH_CUSTOM_NS, WITH_NS, XML_SERDE,
};

#[derive(Debug)]
pub enum ContainerError {
    UnionNotSupported,
    InvalidVariantAttributes(String),
    InvalidFieldAttributes(String),
    InvalidContainerAttributes(String),
    MissingTypeAttribute(String),
    InvalidTypeValue(String),
    InvalidAttributeName(String, String), // (field_name, invalid_attr_name)
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::UnionNotSupported => write!(f, "Only struct and enum types are supported, union is not supported"),
            ContainerError::InvalidVariantAttributes(msg) => write!(f, "Invalid variant attributes: {}", msg),
            ContainerError::InvalidFieldAttributes(msg) => write!(f, "Invalid field attributes: {}", msg),
            ContainerError::InvalidContainerAttributes(msg) => write!(f, "Invalid container attributes: {}", msg),
            ContainerError::MissingTypeAttribute(field) => write!(f, "Field '{}' is missing the required 'type' attribute. Please specify the type using #[xmlserde(ty = \"...\")]", field),
            ContainerError::InvalidTypeValue(field) => write!(f, "Field '{}' has an invalid type value. Valid types are: attr, child, text, untag, untagged_enum, untagged_struct", field),
            ContainerError::InvalidAttributeName(field, attr) => write!(f, "Field '{}' has an invalid attribute name '{}'. Did you mean 'name' instead of '{}'?", field, attr, attr),
        }
    }
}

impl std::error::Error for ContainerError {}

#[derive(Clone)]
pub struct Container<'a> {
    pub struct_fields: Vec<StructField<'a>>, // Struct fields
    pub enum_variants: Vec<EnumVariant<'a>>,
    pub original: &'a syn::DeriveInput,
    pub with_ns: Option<syn::LitByteStr>,
    pub custom_ns: Vec<(syn::LitByteStr, syn::LitByteStr)>,
    pub roots: Vec<syn::LitByteStr>,
    pub deny_unknown: bool,
    pub rename_all: Option<syn::LitStr>,
}

impl<'a> Container<'a> {
    pub fn is_enum(&self) -> bool {
        !self.enum_variants.is_empty()
    }

    pub fn get_root_names(&self) -> Vec<syn::LitByteStr> {
        if self.roots.is_empty() {
            return vec![];
        }

        // If rename_all is set, apply it to all root names
        if let Some(rename_all) = &self.rename_all {
            if let Some(case) = parse_case(rename_all) {
                return self
                    .roots
                    .iter()
                    .map(|root| {
                        let root_value = root.value();
                        let root_str = String::from_utf8_lossy(&root_value);
                        let converted = case.convert(&root_str);
                        syn::LitByteStr::new(converted.as_bytes(), root.span())
                    })
                    .collect();
            }
        }
        // If no rename_all or invalid case, return original roots
        self.roots.clone()
    }

    pub fn validate(&self) -> Result<(), ContainerError> {
        if !self.roots.is_empty() && self.is_enum() {
            return Err(ContainerError::InvalidContainerAttributes(
                "for clarity, enum should not have the root attribute. please use a struct to wrap the enum and set its type to untag".to_string()
            ));
        }
        if self.deny_unknown && self.is_enum() {
            return Err(ContainerError::InvalidContainerAttributes(
                "`deny_unknown_fields` is not supported in enum type".to_string(),
            ));
        }

        for field in &self.struct_fields {
            field.validate()?;
        }
        Ok(())
    }

    fn parse_with_ns(meta: &syn::Meta) -> Option<syn::LitByteStr> {
        let NameValue(m) = meta else { return None };
        if m.path != WITH_NS {
            return None;
        }
        get_lit_byte_str(&m.value).ok().cloned()
    }

    fn parse_roots(meta_item: &syn::Meta) -> Option<Vec<syn::LitByteStr>> {
        if let Meta::NameValue(nv) = meta_item {
            if nv.path == ROOT {
                match &nv.value {
                    | Expr::Lit(ExprLit {
                        lit: Lit::ByteStr(s),
                        ..
                    }) => {
                        return Some(vec![s.clone()]);
                    },
                    | Expr::Array(ExprArray { elems, .. }) => {
                        let mut roots = Vec::new();
                        for elem in elems {
                            if let Expr::Lit(ExprLit {
                                lit: Lit::ByteStr(s),
                                ..
                            }) = elem
                            {
                                roots.push(s.clone());
                            }
                        }
                        if !roots.is_empty() {
                            return Some(roots);
                        }
                    },
                    | _ => {},
                }
            }
        }
        None
    }

    fn parse_custom_ns(meta: &syn::Meta) -> Option<(syn::LitByteStr, syn::LitByteStr)> {
        let Meta::List(l) = meta else { return None };
        if l.path != WITH_CUSTOM_NS {
            return None;
        }
        let strs = l
            .parse_args_with(Punctuated::<syn::LitByteStr, Comma>::parse_terminated)
            .ok()?;
        let mut iter = strs.iter();
        let first = iter.next()?;
        let second = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some((first.clone(), second.clone()))
    }

    fn parse_rename_all(meta: &syn::Meta) -> Option<syn::LitStr> {
        let NameValue(m) = meta else { return None };
        if m.path != RENAME_ALL {
            return None;
        }
        get_lit_str(&m.value).ok().cloned()
    }

    fn parse_container_attrs(item: &'a syn::DeriveInput) -> ContainerAttrs {
        let mut with_ns = None;
        let mut custom_ns = Vec::new();
        let mut roots = Vec::new();
        let mut deny_unknown = false;
        let mut rename_all = None;

        for meta_item in item
            .attrs
            .iter()
            .flat_map(get_xmlserde_meta_items)
            .flatten()
        {
            if let Some(ns) = Self::parse_with_ns(&meta_item) {
                with_ns = Some(ns);
            }
            // Always check for both root and roots
            if let Some(r) = Self::parse_roots(&meta_item) {
                roots.extend(r);
            }
            if let Meta::Path(p) = &meta_item {
                if p == DENY_UNKNOWN {
                    deny_unknown = true;
                }
            } else if let Some(ns_pair) = Self::parse_custom_ns(&meta_item) {
                custom_ns.push(ns_pair);
            } else if let Some(rename) = Self::parse_rename_all(&meta_item) {
                rename_all = Some(rename);
            }
        }

        ContainerAttrs {
            with_ns,
            custom_ns,
            roots,
            deny_unknown,
            rename_all,
        }
    }

    pub fn from_ast(
        item: &'a syn::DeriveInput,
        _derive: Derive,
    ) -> Result<Container<'a>, ContainerError> {
        let attrs = Self::parse_container_attrs(item);

        match &item.data {
            | syn::Data::Struct(ds) => {
                let fields = ds
                    .fields
                    .iter()
                    .map(StructField::from_ast)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Container {
                    struct_fields: fields,
                    enum_variants: vec![],
                    original: item,
                    with_ns: attrs.with_ns,
                    custom_ns: attrs.custom_ns,
                    roots: attrs.roots,
                    deny_unknown: attrs.deny_unknown,
                    rename_all: attrs.rename_all,
                })
            },
            | syn::Data::Enum(de) => {
                let variants = de
                    .variants
                    .iter()
                    .map(EnumVariant::from_ast)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Container {
                    struct_fields: vec![],
                    enum_variants: variants,
                    original: item,
                    with_ns: attrs.with_ns,
                    custom_ns: attrs.custom_ns,
                    roots: attrs.roots,
                    deny_unknown: attrs.deny_unknown,
                    rename_all: attrs.rename_all,
                })
            },
            | syn::Data::Union(_) => Err(ContainerError::UnionNotSupported),
        }
    }

    pub fn get_field_name(&self, field: &StructField<'a>) -> Option<syn::LitByteStr> {
        // If field has an explicit name, use it directly
        if let Some(name) = &field.name {
            return Some(name.clone());
        }

        // If field has mapped names, use the first one
        if !field.mapped_names.is_empty() {
            return Some(field.mapped_names[0].clone());
        }

        // Only apply rename_all case conversion if there's no explicit name or mapped names
        if let Some(rename_all) = &self.rename_all {
            if let Some(case) = parse_case(rename_all) {
                // Defensive: field.original.ident may be None for unnamed fields
                if let Some(ident) = field.original.ident.as_ref() {
                    let field_name = ident.to_string();
                    let converted = case.convert(&field_name);
                    return Some(syn::LitByteStr::new(
                        converted.as_bytes(),
                        rename_all.span(),
                    ));
                }
            }
        }

        None
    }
}

pub struct FieldsSummary<'a> {
    pub children: Vec<StructField<'a>>,
    pub text: Option<StructField<'a>>,
    pub attrs: Vec<StructField<'a>>,
    pub self_closed_children: Vec<StructField<'a>>,
    pub untagged_enums: Vec<StructField<'a>>,
    pub untagged_structs: Vec<StructField<'a>>,
}

impl<'a> FieldsSummary<'a> {
    pub fn from_fields(fields: &[StructField<'a>]) -> Self {
        let fields = fields.to_vec();
        let mut result = FieldsSummary {
            children: vec![],
            text: None,
            attrs: vec![],
            self_closed_children: vec![],
            untagged_enums: vec![],
            untagged_structs: vec![],
        };
        fields.into_iter().for_each(|f| match f.ty {
            | EleType::Attr => result.attrs.push(f),
            | EleType::Child => result.children.push(f),
            | EleType::Text => result.text = Some(f),
            | EleType::SelfClosedChild => result.self_closed_children.push(f),
            | EleType::Untag => result.untagged_enums.push(f),
            | EleType::UntaggedEnum => result.untagged_enums.push(f),
            | EleType::UntaggedStruct => result.untagged_structs.push(f),
        });
        result
    }
}

#[derive(Clone)]
pub struct StructField<'a> {
    pub ty: EleType,
    pub name: Option<syn::LitByteStr>,
    pub mapped_names: Vec<syn::LitByteStr>,
    pub skip_serializing: bool,
    pub default: Option<syn::ExprPath>,
    pub original: &'a syn::Field,
    pub vec_size: Option<syn::Lit>,
    pub generic: Generic<'a>,
}

impl<'a> StructField<'a> {
    pub fn validate(&self) -> Result<(), ContainerError> {
        let untagged = matches!(
            self.ty,
            EleType::Untag | EleType::UntaggedEnum | EleType::UntaggedStruct
        );
        if untagged && self.name.is_some() {
            return Err(ContainerError::InvalidFieldAttributes(
                "untagged types doesn't need a name".to_string(),
            ));
        }
        Ok(())
    }

    fn parse_type(meta: &syn::Meta, field_name: &str) -> Result<EleType, ContainerError> {
        if let NameValue(m) = meta {
            if m.path == TYPE {
                if let Ok(s) = get_lit_str(&m.value) {
                    return match s.value().as_str() {
                        | s if s == TYPE_ATTR.value() => Ok(EleType::Attr),
                        | s if s == TYPE_CHILD.value() => Ok(EleType::Child),
                        | s if s == TYPE_TEXT.value() => Ok(EleType::Text),
                        | s if s == TYPE_SFC.value() => Ok(EleType::SelfClosedChild),
                        | s if s == TYPE_UNTAG.value() => Ok(EleType::Untag),
                        | s if s == TYPE_UNTAGGED_ENUM.value() => Ok(EleType::UntaggedEnum),
                        | s if s == TYPE_UNTAGGED_STRUCT.value() => Ok(EleType::UntaggedStruct),
                        | _ => Err(ContainerError::InvalidTypeValue(field_name.to_string())),
                    };
                }
            }
        }
        Err(ContainerError::MissingTypeAttribute(field_name.to_string()))
    }

    fn parse_vec_size(meta: &syn::Meta) -> Option<syn::Lit> {
        if let NameValue(m) = meta {
            if m.path == VEC_SIZE {
                if let syn::Expr::Lit(lit) = &m.value {
                    match &lit.lit {
                        | syn::Lit::Str(_) | syn::Lit::Int(_) => return Some(lit.lit.clone()),
                        | _ => return None,
                    }
                }
            }
        }
        None
    }

    fn parse_default(meta: &syn::Meta) -> Option<syn::ExprPath> {
        let NameValue(m) = meta else {
            return None;
        };
        if m.path != DEFAULT {
            return None;
        }
        parse_lit_into_expr_path(&m.value).ok()
    }

    fn parse_field_attrs(f: &'a syn::Field) -> Result<FieldAttrs, ContainerError> {
        let field_name = f
            .ident
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        let mut name = None;
        let mut mapped_names = Vec::new();
        let mut skip_serializing = false;
        let mut default = None;
        let mut ty = None;
        let mut vec_size = None;

        for meta_item in f.attrs.iter().flat_map(get_xmlserde_meta_items).flatten() {
            match &meta_item {
                | Meta::NameValue(m) => {
                    if m.path == NAME {
                        if let Ok(s) = get_lit_byte_str(&m.value) {
                            name = Some(s.clone());
                        }
                    } else if m.path == MAP {
                        if let syn::Expr::Array(array) = &m.value {
                            for elem in &array.elems {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::ByteStr(s),
                                    ..
                                }) = elem
                                {
                                    mapped_names.push(s.clone());
                                } else {
                                    return Err(ContainerError::InvalidFieldAttributes(
                                        "map values must be byte string literals".to_string(),
                                    ));
                                }
                            }
                        } else {
                            return Err(ContainerError::InvalidFieldAttributes(
                                "map attribute must be an array of byte string literals"
                                    .to_string(),
                            ));
                        }
                    } else if m.path == TYPE {
                        if let Ok(t) = Self::parse_type(&meta_item, &field_name) {
                            ty = Some(t);
                        }
                    } else if m.path == VEC_SIZE {
                        if let Some(vs) = Self::parse_vec_size(&meta_item) {
                            vec_size = Some(vs);
                        }
                    } else if m.path == DEFAULT {
                        if let Some(d) = Self::parse_default(&meta_item) {
                            default = Some(d);
                        }
                    } else {
                        // Check for common typos
                        let attr_name = m.path.get_ident().map(|i| i.to_string());
                        if let Some(attr) = attr_name {
                            if attr == "names" {
                                return Err(ContainerError::InvalidAttributeName(field_name, attr));
                            }
                        }
                    }
                },
                | Meta::Path(p) if *p == SKIP_SERIALIZING => {
                    skip_serializing = true;
                },
                | _ => {},
            }
        }

        // Defensive: If ty is missing, return a clear error
        let ty = ty.ok_or_else(|| ContainerError::MissingTypeAttribute(field_name.clone()))?;
        Ok(FieldAttrs {
            name,
            mapped_names,
            skip_serializing,
            default,
            ty,
            vec_size,
        })
    }

    pub fn from_ast(f: &'a syn::Field) -> Result<Self, ContainerError> {
        let attrs = Self::parse_field_attrs(f)?;
        let generic = get_generics(&f.ty);

        // Remove fallback name assignment: do not assign a name if neither name nor mapped_names are present.
        // Let get_field_name handle rename_all case conversion at runtime.
        let name = attrs.name;
        let mapped_names = attrs.mapped_names;

        Ok(StructField {
            ty: attrs.ty,
            name,
            mapped_names,
            skip_serializing: attrs.skip_serializing,
            default: attrs.default,
            original: f,
            vec_size: attrs.vec_size,
            generic,
        })
    }

    pub fn is_required(&self) -> bool {
        if matches!(self.ty, EleType::Untag) || matches!(self.ty, EleType::UntaggedEnum) {
            return match self.generic {
                | Generic::Vec(_) => false,
                | Generic::Opt(_) => false,
                | Generic::Boxed(_) => false,
                | Generic::None => true,
            };
        }
        self.default.is_none()
            && matches!(self.generic, Generic::None)
            && !matches!(self.ty, EleType::SelfClosedChild)
    }
}

#[derive(Clone)]
pub struct EnumVariant<'a> {
    pub name: Option<syn::LitByteStr>,
    pub ident: &'a syn::Ident,
    pub ty: Option<&'a syn::Type>,
    pub ele_type: EleType,
}

impl<'a> EnumVariant<'a> {
    fn parse_type(meta: &syn::Meta) -> Option<EleType> {
        if let NameValue(m) = meta {
            if m.path == TYPE {
                if let Ok(s) = get_lit_str(&m.value) {
                    return match s.value().as_str() {
                        | s if s == TYPE_CHILD.value() => Some(EleType::Child),
                        | s if s == TYPE_TEXT.value() => Some(EleType::Text),
                        | _ => None,
                    };
                }
            }
        }
        None
    }

    fn validate_variant_fields(
        fields: &syn::Fields,
        ele_type: &EleType,
        name: Option<&syn::LitByteStr>,
    ) -> Result<(), String> {
        if fields.len() > 1 {
            return Err("only support 1 field".to_string());
        }

        match ele_type {
            | EleType::Text => {
                if name.is_some() {
                    return Err("should omit the `name`".to_string());
                }
            },
            | _ => {
                if name.is_none() {
                    return Err("should have name".to_string());
                }
            },
        }

        Ok(())
    }

    fn parse_variant_attrs(v: &'a Variant) -> Result<(Option<syn::LitByteStr>, EleType), String> {
        let mut name = None;
        let mut ele_type = EleType::Child;

        for meta_item in v.attrs.iter().flat_map(get_xmlserde_meta_items).flatten() {
            match &meta_item {
                | Meta::NameValue(m) => {
                    if m.path == NAME {
                        if let Ok(s) = get_lit_byte_str(&m.value) {
                            name = Some(s.clone());
                        }
                    } else if m.path == TYPE {
                        if let Some(t) = Self::parse_type(&meta_item) {
                            ele_type = t;
                        }
                    } else {
                        // Check for common typos
                        let attr_name = m.path.get_ident().map(|i| i.to_string());
                        if let Some(attr) = attr_name {
                            if attr == "names" {
                                return Err(format!("Invalid attribute name '{}'. Did you mean 'name' instead of '{}'?", attr, attr));
                            }
                        }
                    }
                },
                | _ => {},
            }
        }

        Self::validate_variant_fields(&v.fields, &ele_type, name.as_ref())?;
        Ok((name, ele_type))
    }

    pub fn from_ast(v: &'a Variant) -> Result<Self, ContainerError> {
        let (name, ele_type) =
            Self::parse_variant_attrs(v).map_err(ContainerError::InvalidVariantAttributes)?;
        let field = v.fields.iter().next();
        let ty = field.map(|t| &t.ty);
        let ident = &v.ident;

        Ok(EnumVariant {
            name,
            ty,
            ident,
            ele_type,
        })
    }
}

#[derive(Clone)]
pub enum EleType {
    Attr,
    Child,
    Text,
    ///
    /// ```
    /// struct Font {
    ///     bold: bool,
    ///     italic: bool,
    /// }
    /// ```
    /// In the xml, it is like
    /// <font>
    ///     <b/>
    ///     <i/>
    /// </font>
    /// In this case, </b> indicates the field *bold* is true and <i/> indicates *italic* is true.
    SelfClosedChild,
    /// Deprecated, use `UntaggedEnum`
    Untag,
    UntaggedEnum,
    UntaggedStruct,
}

pub enum Derive {
    Serialize,
    Deserialize,
}

fn get_xmlserde_meta_items(attr: &syn::Attribute) -> Result<Vec<syn::Meta>, ()> {
    if attr.path() != XML_SERDE {
        return Ok(Vec::new());
    }

    match attr.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated) {
        | Ok(meta) => Ok(meta.into_iter().collect()),
        | Err(_) => Err(()),
    }
}

fn get_lit_byte_str(expr: &syn::Expr) -> Result<&syn::LitByteStr, ()> {
    if let syn::Expr::Lit(lit) = expr {
        if let syn::Lit::ByteStr(l) = &lit.lit {
            return Ok(l);
        }
    }
    Err(())
}

fn get_lit_str(lit: &syn::Expr) -> Result<&syn::LitStr, ()> {
    if let syn::Expr::Lit(lit) = lit {
        if let syn::Lit::Str(l) = &lit.lit {
            return Ok(l);
        }
    }
    Err(())
}

pub fn parse_lit_into_expr_path(value: &syn::Expr) -> Result<syn::ExprPath, ()> {
    let l = get_lit_str(value)?;
    parse_lit_str(l).map_err(|_| ())
}

pub fn parse_lit_str<T>(s: &syn::LitStr) -> parse::Result<T>
where
    T: Parse,
{
    let tokens = spanned_tokens(s)?;
    syn::parse2(tokens)
}

fn spanned_tokens(s: &syn::LitStr) -> parse::Result<TokenStream> {
    let stream = syn::parse_str(&s.value())?;
    Ok(respan(stream, s.span()))
}

fn respan(stream: TokenStream, span: Span) -> TokenStream {
    stream
        .into_iter()
        .map(|token| respan_token(token, span))
        .collect()
}

fn respan_token(mut token: TokenTree, span: Span) -> TokenTree {
    if let TokenTree::Group(g) = &mut token {
        *g = Group::new(g.delimiter(), respan(g.stream(), span));
    }
    token.set_span(span);
    token
}

fn get_generic_type_from_args(
    args: &Punctuated<syn::GenericArgument, Comma>,
) -> Option<&syn::Type> {
    if args.len() != 1 {
        return None;
    }
    if let Some(syn::GenericArgument::Type(t)) = args.first() {
        Some(t)
    } else {
        None
    }
}

fn get_generic_type<'a>(path: &'a syn::Path, type_name: &str) -> Option<&'a syn::Type> {
    let seg = path.segments.last()?;
    if seg.ident != type_name {
        return None;
    }
    match &seg.arguments {
        | syn::PathArguments::AngleBracketed(a) => get_generic_type_from_args(&a.args),
        | _ => None,
    }
}

pub(crate) fn get_generics(t: &syn::Type) -> Generic {
    let path = match t {
        | syn::Type::Path(p) => &p.path,
        | _ => return Generic::None,
    };

    if let Some(ty) = get_generic_type(path, "Vec") {
        return Generic::Vec(ty);
    }
    if let Some(ty) = get_generic_type(path, "Option") {
        return Generic::Opt(ty);
    }
    if let Some(ty) = get_generic_type(path, "Box") {
        return Generic::Boxed(ty);
    }
    Generic::None
}

#[derive(Clone)]
pub enum Generic<'a> {
    Vec(&'a syn::Type),
    Opt(&'a syn::Type),
    Boxed(&'a syn::Type),
    None,
}

impl Generic<'_> {
    pub fn is_vec(&self) -> bool {
        match self {
            | Generic::Vec(_) => true,
            | _ => false,
        }
    }

    pub fn is_opt(&self) -> bool {
        match self {
            | Generic::Opt(_) => true,
            | _ => false,
        }
    }

    pub fn is_boxed(&self) -> bool {
        match self {
            | Generic::Boxed(_) => true,
            | _ => false,
        }
    }

    pub fn get_vec(&self) -> Option<&syn::Type> {
        match self {
            | Generic::Vec(v) => Some(v),
            | _ => None,
        }
    }

    pub fn get_opt(&self) -> Option<&syn::Type> {
        match self {
            | Generic::Opt(v) => Some(v),
            | _ => None,
        }
    }

    pub fn get_boxed(&self) -> Option<&syn::Type> {
        match self {
            | Generic::Boxed(v) => Some(v),
            | _ => None,
        }
    }
}

// Define struct for container attributes
pub struct ContainerAttrs {
    pub with_ns: Option<syn::LitByteStr>,
    pub custom_ns: Vec<(syn::LitByteStr, syn::LitByteStr)>,
    pub roots: Vec<syn::LitByteStr>,
    pub deny_unknown: bool,
    pub rename_all: Option<syn::LitStr>,
}

// Define struct for field attributes
pub struct FieldAttrs {
    pub name: Option<syn::LitByteStr>,
    pub mapped_names: Vec<syn::LitByteStr>,
    pub skip_serializing: bool,
    pub default: Option<syn::ExprPath>,
    pub ty: EleType,
    pub vec_size: Option<syn::Lit>,
}
