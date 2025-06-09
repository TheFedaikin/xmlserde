use crate::symbol::{MAP, OTHER, RENAME};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Ident, Type};

struct EnumVariantInfo {
    ident: Ident,
    xml_value: String,
    is_other: bool,
    other_type: Option<Type>,
    mapped_values: Vec<String>,
}

pub fn get_xml_serde_enum_impl_block(input: DeriveInput) -> Result<TokenStream, syn::Error> {
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
        let mut mapped_values = Vec::new();

        for attr in &variant.attrs {
            if attr.path().is_ident("xmlserde") {
                // Parse #[xmlserde(rename = "Value")] or #[xmlserde(other)] or #[xmlserde(map = ["value1", "value2"])]
                attr.parse_nested_meta(|meta| {
                    if meta.path == RENAME {
                        let value = meta.value()?;
                        let lit_str: syn::LitStr = value.parse()?;
                        xml_value_str = lit_str.value();
                    } else if meta.path == OTHER {
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
                    } else if meta.path == MAP {
                        let value = meta.value()?;
                        let list: syn::ExprArray = value.parse()?;
                        for elem in list.elems {
                            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = elem {
                                mapped_values.push(s.value());
                            } else {
                                return Err(Error::new_spanned(elem, "map values must be string literals"));
                            }
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
            mapped_values,
        });
    }

    let mut serialize_arms = Vec::new();
    let mut deserialize_arms = Vec::new();
    let mut other_arm_deserialize: Option<proc_macro2::TokenStream> = None;

    for variant in &parsed_variants {
        let ident = &variant.ident;
        let xml_value = &variant.xml_value;
        let mapped_values = &variant.mapped_values;

        // Add serialize arm
        if variant.is_other {
            serialize_arms.push(quote! {
                Self::#ident(s) => s.clone(),
            });
        } else {
            // Use first mapped value if available, otherwise use variant name
            let xml_value = if !variant.mapped_values.is_empty() {
                &variant.mapped_values[0]
            } else {
                &variant.xml_value
            };
            serialize_arms.push(quote! {
                Self::#ident => #xml_value.to_string(),
            });
        }

        // Add deserialize arm
        if variant.is_other {
            let other_type = variant.other_type.as_ref().unwrap();
            other_arm_deserialize = Some(quote! {
                _ => Self::#ident(<#other_type as ::xmlserde::XmlValue>::deserialize(s).unwrap()),
            });
        } else {
            let mut match_arms = vec![quote! {
                #xml_value => Self::#ident,
            }];

            // Add mapped values
            for mapped_value in mapped_values {
                match_arms.push(quote! {
                    #mapped_value => Self::#ident,
                });
            }

            deserialize_arms.push(quote! {
                #(#match_arms)*
            });
        }
    }

    let deserialize_arms = if let Some(other_arm) = other_arm_deserialize {
        quote! {
            #(#deserialize_arms)*
            #other_arm
        }
    } else {
        quote! {
            #(#deserialize_arms)*
            _ => panic!("unknown variant"),
        }
    };

    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::xmlserde::XmlValue for #ident #type_generics #where_clause {
            fn serialize(&self) -> String {
                match self {
                    #(#serialize_arms)*
                }
            }

            fn deserialize(s: &str) -> Result<Self, String> {
                Ok(match s {
                    #deserialize_arms
                })
            }
        }
    })
}
