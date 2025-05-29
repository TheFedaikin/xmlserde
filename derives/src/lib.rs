mod case;
mod container;
mod de;
mod enumeration;
mod ser;
mod symbol;

use container::{Container, Derive};
use de::get_de_impl_block;
use proc_macro::TokenStream;
use ser::{get_ser_enum_impl_block, get_ser_struct_impl_block};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(XmlDeserialize, attributes(xmlserde))]
pub fn derive_xml_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match Container::from_ast(&input, Derive::Deserialize) {
        | Ok(container) => {
            if let Err(e) = container.validate() {
                return syn::Error::new_spanned(&input, e.to_string())
                    .to_compile_error()
                    .into();
            }
            get_de_impl_block(input).into()
        },
        | Err(e) => syn::Error::new_spanned(&input, e.to_string())
            .to_compile_error()
            .into(),
    }
}

#[proc_macro_derive(XmlSerdeEnum, attributes(xmlserde))]
pub fn derive_xml_serde_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match enumeration::get_xml_serde_enum_impl_block(input) {
        | Ok(ts) => ts.into(),
        | Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(XmlSerialize, attributes(xmlserde))]
pub fn derive_xml_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match Container::from_ast(&input, Derive::Serialize) {
        | Ok(container) => {
            if let Err(e) = container.validate() {
                return syn::Error::new_spanned(&input, e.to_string())
                    .to_compile_error()
                    .into();
            }
            let result = if container.is_enum() {
                get_ser_enum_impl_block(container)
            } else {
                get_ser_struct_impl_block(container)
            };
            result.into()
        },
        | Err(e) => syn::Error::new_spanned(&input, e.to_string())
            .to_compile_error()
            .into(),
    }
}
