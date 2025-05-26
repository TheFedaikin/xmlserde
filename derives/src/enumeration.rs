use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Ident, Lit, Type};

struct EnumVariantInfo {
    ident: Ident,
    xml_value: String,
    is_other: bool,
    other_type: Option<Type>,
}

pub fn get_xml_serde_enum_impl_block(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let enum_name = &input.ident;

    let variants = match &input.data {
        | Data::Enum(data_enum) => &data_enum.variants,
        | _ => {
            return Err(Error::new_spanned(
                input,
                "XmlSerdeEnum can only be derived for enums",
            ))
        },
    };

    let mut parsed_variants = Vec::new();

    for variant in variants {
        let variant_ident = variant.ident.clone();
        let mut xml_value_str = variant_ident.to_string();
        let mut is_other_attr = false;
        let mut other_inner_type: Option<Type> = None;

        for attr in &variant.attrs {
            if attr.path().is_ident("xmlserde") {
                // Parse #[xmlserde(rename = "Value")] or #[xmlserde(other)]
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        let value = meta.value()?;
                        let lit_str: syn::LitStr = value.parse()?;
                        xml_value_str = lit_str.value();
                    } else if meta.path.is_ident("other") {
                        is_other_attr = true;
                        // Check if it has a single unnamed field for the String
                        if let Fields::Unnamed(fields_unnamed) = &variant.fields {
                            if fields_unnamed.unnamed.len() == 1 {
                                other_inner_type = Some(fields_unnamed.unnamed.first().unwrap().ty.clone());
                            } else {
                                return Err(Error::new_spanned(&variant.fields, "#[xmlserde(other)] variant must have exactly one unnamed field."));
                            }
                        } else {
                            return Err(Error::new_spanned(&variant.fields, "#[xmlserde(other)] variant must have unnamed fields."));
                        }
                    }
                    Ok(())
                })?;
            }
        }

        parsed_variants.push(EnumVariantInfo {
            ident: variant_ident,
            xml_value: xml_value_str,
            is_other: is_other_attr,
            other_type: other_inner_type,
        });
    }

    let mut serialize_arms = Vec::new();
    let mut deserialize_arms = Vec::new();
    let mut other_arm_deserialize: Option<proc_macro2::TokenStream> = None;

    for pv in &parsed_variants {
        let variant_ident = &pv.ident;
        let xml_value_lit = Lit::Str(syn::LitStr::new(&pv.xml_value, variant_ident.span()));

        if pv.is_other {
            if let Some(ref inner_ty) = pv.other_type {
                let type_is_string = match inner_ty {
                    | Type::Path(type_path) => type_path
                        .path
                        .segments
                        .last()
                        .map_or(false, |seg| seg.ident == "String"),
                    | _ => false,
                };

                if type_is_string {
                    serialize_arms.push(quote! { Self::#variant_ident(s_val) => s_val.clone(), });
                    other_arm_deserialize =
                        Some(quote! { _ => Ok(Self::#variant_ident(s.to_string())) });
                } else {
                    // Assumes the type implements ToString for serialize and FromStr for deserialize
                    serialize_arms
                        .push(quote! { Self::#variant_ident(s_val) => s_val.to_string(), });
                    other_arm_deserialize = Some(
                        quote! { _ => Ok(Self::#variant_ident(s.parse::<#inner_ty>().map_err(|e| format!("Failed to parse '{}' as {}: {}", s, stringify!(#inner_ty), e.to_string()))?)) },
                    );
                }
            } else {
                // This case should have been caught earlier by the parser for `#[xmlserde(other)]`
                return Err(Error::new_spanned(
                    variant_ident,
                    "#[xmlserde(other)] variant lacks a defined inner type.",
                ));
            }
        } else {
            serialize_arms.push(quote! { Self::#variant_ident => String::from(#xml_value_lit), });
            deserialize_arms.push(quote! { #xml_value_lit => Ok(Self::#variant_ident), });
        }
    }

    let deserialize_match_arms = if let Some(other_arm) = other_arm_deserialize {
        quote! {
            #(#deserialize_arms)*
            #other_arm
        }
    } else {
        // If no `other` arm, any unknown value is an error.
        quote! {
            #(#deserialize_arms)*
            _ => Err(format!("Unknown value for {}: {}", stringify!(#enum_name), s)),
        }
    };

    let expanded = quote! {
        impl xmlserde::XmlValue for #enum_name {
            fn serialize(&self) -> String {
                match self {
                    #(#serialize_arms)*
                }
            }

            fn deserialize(s: &str) -> Result<Self, String> {
                match s {
                    #deserialize_match_arms
                }
            }
        }
    };

    Ok(expanded)
}
