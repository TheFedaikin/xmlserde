use quote::quote;

use crate::container::{Container, EleType, FieldsSummary, Generic, StructField};

pub fn get_ser_enum_impl_block(container: Container) -> proc_macro2::TokenStream {
    let ident = &container.original.ident;
    let (impl_generics, type_generics, where_clause) = container.original.generics.split_for_impl();
    let branches = container.enum_variants.iter().map(|v| {
        let f = v.ident;
        let ele_ty = &v.ele_type;
        if v.ty.is_none() {
            let name = v.name.as_ref().expect("should have name");
            quote!{
                Self::#f => {
                    if tag == b"" {
                        let _t = String::from_utf8_lossy(#name);
                        let _ = writer.write_event(Event::Empty(BytesStart::new(_t)));
                    } else {
                        let _ = writer.write_event(Event::Start(BytesStart::new(String::from_utf8_lossy(tag))));
                        let _t = String::from_utf8_lossy(#name);
                        let _ = writer.write_event(Event::Empty(BytesStart::new(_t)));
                        let _ = writer.write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(tag))));
                    }
                }
            }
        } else if matches!(ele_ty, EleType::Text) {
            let field_ty = v.ty.expect("text variant should have a type");
            let generic_info = crate::container::get_generics(field_ty);
            if generic_info.is_boxed() {
                quote!{
                    Self::#f(c) => {
                        let _ = writer.write_event(Event::Text(BytesText::new(&(**c).serialize())));
                    }
                }
            } else {
                quote!{
                    Self::#f(c) => {
                        let _ = writer.write_event(Event::Text(BytesText::new(&c.serialize())));
                    }
                }
            }
        } else {
            let name = v.name.as_ref().expect("should have name");
            let field_ty = v.ty.expect("child variant should have a type");
            let generic_info = crate::container::get_generics(field_ty);
            if generic_info.is_boxed() {
                quote! {
                    Self::#f(c) => {
                        if tag == b"" {
                            (**c).serialize(#name, writer);
                        } else {
                            let _ = writer.write_event(Event::Start(BytesStart::new(String::from_utf8_lossy(tag))));
                            (**c).serialize(#name, writer);
                            let _ = writer.write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(tag))));
                        }
                    },
                }
            } else {
                quote! {
                    Self::#f(c) => {
                        if tag == b"" {
                            c.serialize(#name, writer);
                        } else {
                            let _ = writer.write_event(Event::Start(BytesStart::new(String::from_utf8_lossy(tag))));
                            c.serialize(#name, writer);
                            let _ = writer.write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(tag))));
                        }
                    },
                }
            }
        }
    });
    quote! {
        #[allow(unused_must_use)]
        impl #impl_generics ::xmlserde::XmlSerialize for #ident #type_generics #where_clause {
            fn serialize<W: std::io::Write>(
                &self,
                tag: &[u8],
                writer: &mut ::xmlserde::quick_xml::Writer<W>,
            ) {
                use ::xmlserde::quick_xml::events::*;
                match self {
                    #(#branches)*
                }
            }
        }
    }
}

pub fn get_ser_struct_impl_block(container: Container) -> proc_macro2::TokenStream {
    let write_ns = match &container.with_ns {
        | Some(ns) => {
            quote! {
                attrs.push(Attribute::from((b"xmlns".as_ref(), #ns.as_ref())));
            }
        },
        | None => quote! {},
    };
    let write_custom_ns = if container.custom_ns.is_empty() {
        quote! {}
    } else {
        let cns = container.custom_ns.iter().map(|(ns, value)| {
            quote! {
                let mut __vec = b"xmlns:".to_vec();
                __vec.extend(#ns.to_vec());
                attrs.push(Attribute::from((__vec.as_ref(), #value.as_ref())));
            }
        });
        quote! {#(#cns)*}
    };
    let FieldsSummary {
        children,
        text,
        attrs,
        self_closed_children,
        untagged_enums: untags,
        untagged_structs: _,
    } = FieldsSummary::from_fields(&container.struct_fields);
    if text.is_some()
        && (!children.is_empty() || !self_closed_children.is_empty() || !untags.is_empty())
    {
        panic!("Cannot have the text and children at the same time.")
    }
    let init = init_is_empty(&children, &self_closed_children, &untags, &text);
    let build_attr_and_push = attrs.iter().map(|attr| {
        let name = container
            .get_field_name(attr)
            .or_else(|| {
                // Try to use rename_all if possible
                container.get_field_name(attr)
            })
            .unwrap_or_else(|| {
                let ident = attr
                    .original
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "<unnamed>".to_string());
                panic!("No name or mapped_names or rename_all for field: {}", ident)
            });
        let ident = attr.original.ident.as_ref().unwrap();
        match &attr.generic {
            | Generic::Vec(_) => panic!("cannot use a vector in attribute"),
            | Generic::Opt(_) => {
                quote! {
                    let mut sr: String;
                    match &self.#ident {
                        Some(v) => {
                            sr = v.serialize();
                            attrs.push(Attribute::from((#name.as_ref(), sr.as_bytes())));
                        },
                        None => {},
                    }
                }
            },
            | Generic::Boxed(_) => {
                quote! { panic!("Attributes cannot be of type Box<T>"); }
            },
            | Generic::None => match &attr.default {
                | Some(path) => {
                    quote! {
                        let mut ser;
                        if #path() != self.#ident {
                            ser = self.#ident.serialize();
                            attrs.push(Attribute::from((#name.as_ref(), ser.as_bytes())));
                        }
                    }
                },
                | None => {
                    quote! {
                        let ser = self.#ident.serialize();
                        attrs.push(Attribute::from((#name.as_ref(), ser.as_bytes())));
                    }
                },
            },
        }
    });
    let write_text_or_children = if let Some(t) = text {
        let ident = t.original.ident.as_ref().unwrap();
        match &t.generic {
            | Generic::Opt(opt_inner_ty) => {
                let generic_of_opt_inner = crate::container::get_generics(opt_inner_ty);
                if generic_of_opt_inner.is_boxed() {
                    quote! {
                        match &self.#ident {
                            None => {},
                            Some(__d) => { // __d is Box<DeepValue>
                                let r = (*__d).serialize(); // XmlValue::serialize()
                                let event = BytesText::new(&r);
                                writer.write_event(Event::Text(event));
                            }
                        }
                    }
                } else {
                    // Option<Value>
                    quote! {
                        match &self.#ident {
                            None => {},
                            Some(__d) => { // __d is Value
                                let r = __d.serialize(); // XmlValue::serialize()
                                let event = BytesText::new(&r);
                                writer.write_event(Event::Text(event));
                            }
                        }
                    }
                }
            },
            | Generic::Boxed(_boxed_inner_ty) => {
                // self.#ident is Box<Value>
                quote! {
                    let r = (*self.#ident).serialize(); // XmlValue::serialize()
                    let event = BytesText::new(&r);
                    writer.write_event(Event::Text(event));
                }
            },
            | Generic::None => {
                // self.#ident is Value
                quote! {
                    let r = self.#ident.serialize(); // XmlValue::serialize()
                    let event = BytesText::new(&r);
                    writer.write_event(Event::Text(event));
                }
            },
            | Generic::Vec(_) => panic!("Vec cannot be text content"), // Should not happen
        }
    } else {
        let write_scf = self_closed_children.into_iter().map(|f| {
            let ident = f.original.ident.as_ref().unwrap();
            let name_owned = container.get_field_name(&f);
            let name_ref: &syn::LitByteStr = if let Some(n) = f.name.as_ref() {
                n
            } else if let Some(n) = name_owned.as_ref() {
                n
            } else {
                let ident = f
                    .original
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "<unnamed>".to_string());
                panic!("No name or mapped_names or rename_all for field: {}", ident)
            };
            quote! {
                if self.#ident {
                    let event = BytesStart::new(String::from_utf8_lossy(#name_ref.as_ref()));
                    writer.write_event(Event::Empty(event));
                }
            }
        });
        let write_children = children.into_iter().map(|f| {
            if f.skip_serializing {
                quote! {}
            } else {
                let ident = f.original.ident.as_ref().unwrap();
                let name_owned = container.get_field_name(&f);
                let name_ref: &syn::LitByteStr = if let Some(n) = f.name.as_ref() {
                    n
                } else if let Some(n) = name_owned.as_ref() {
                    n
                } else {
                    let ident = f
                        .original
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "<unnamed>".to_string());
                    panic!("No name or mapped_names or rename_all for field: {}", ident)
                };
                match &f.generic {
                    | Generic::Boxed(_) => {
                        quote! { (*self.#ident).serialize(#name_ref.as_ref(), writer); }
                    },
                    | _ => {
                        quote! { self.#ident.serialize(#name_ref.as_ref(), writer); }
                    },
                }
            }
        });
        let write_untags = untags.into_iter().map(|f| {
            let ident = f.original.ident.as_ref().expect("should have name"); // This was wrong, ident is f.original.ident...
                                                                              // let ident = f.original.ident.as_ref().unwrap();
            match &f.generic {
                | Generic::Boxed(_) => {
                    // Field is Box<UntaggedEnum>
                    quote! { (*self.#ident).serialize(b"", writer); }
                },
                | _ => {
                    quote! { self.#ident.serialize(b"", writer); }
                },
            }
        });
        quote! {
            #(#write_scf)*
            #(#write_children)*
            #(#write_untags)*
        }
    };
    let ident = &container.original.ident;
    let (impl_generics, type_generics, where_clause) = container.original.generics.split_for_impl();
    let write_event = quote! {
        if is_empty {
            writer.write_event(Event::Empty(start));
        } else if is_untagged {
            // Not to write the start event
            #write_text_or_children
        } else {
            writer.write_event(Event::Start(start));
            #write_text_or_children
            let end = BytesEnd::new(String::from_utf8_lossy(tag));
            writer.write_event(Event::End(end));
        }
    };
    let get_roots = if !container.roots.is_empty() {
        let roots = container.get_root_names();
        quote! {
            fn ser_roots() -> Vec<&'static [u8]> {
                vec![#(#roots),*]
            }
        }
    } else {
        quote! {}
    };
    quote! {
        #[allow(unused_must_use)]
        impl #impl_generics ::xmlserde::XmlSerialize for #ident #type_generics #where_clause {
            fn serialize<W: std::io::Write>(
                &self,
                tag: &[u8],
                writer: &mut ::xmlserde::quick_xml::Writer<W>,
            ) {
                use ::xmlserde::quick_xml::events::*;
                use ::xmlserde::quick_xml::events::attributes::Attribute;
                use ::xmlserde::XmlValue;
                let start = BytesStart::new(String::from_utf8_lossy(tag));
                let mut attrs = Vec::<Attribute>::new();
                let is_untagged = tag.len() == 0;
                #write_ns
                #write_custom_ns
                #(#build_attr_and_push)*
                let start = start.with_attributes(attrs);
                #init
                #write_event
            }
            #get_roots
        }
    }
}

fn init_is_empty(
    children: &[StructField],
    scf: &[StructField],
    untags: &[StructField],
    text: &Option<StructField>,
) -> proc_macro2::TokenStream {
    let children_init = children.iter().map(|c| {
        let ident = c.original.ident.as_ref().unwrap();
        match &c.generic {
            | Generic::Vec(_) => {
                quote! {
                    let #ident = self.#ident.len() > 0;
                }
            },
            | Generic::Opt(_) => {
                quote! {
                    let #ident = self.#ident.is_some();
                }
            },
            | Generic::Boxed(_) => match &c.default {
                | Some(d) => {
                    quote! {
                        let #ident = *self.#ident != #d();
                    }
                },
                | None => quote! {let #ident = true;},
            },
            | Generic::None => match &c.default {
                | Some(d) => {
                    quote! {
                        let #ident = self.#ident != #d();
                    }
                },
                | None => quote! {let #ident = true;},
            },
        }
    });
    let has_untag_fields = !untags.is_empty();
    let scf_init = scf.iter().map(|s| {
        let ident = s.original.ident.as_ref().unwrap();
        quote! {
            let #ident = self.#ident;
        }
    });
    let text_init = match text {
        | Some(tf) => {
            let ident = tf.original.ident.as_ref().unwrap();
            if tf.generic.is_opt() {
                quote! {
                    let mut has_text = true;
                    if self.#ident.is_none() {
                        has_text = false;
                    }
                }
            } else if tf.default.is_none() {
                quote! {let has_text = true;}
            } else {
                let path = tf.default.as_ref().unwrap();
                quote! {
                    let mut has_text = true;
                    if self.#ident == #path() {
                        has_text = false;
                    }
                }
            }
        },
        | None => quote! {let has_text = false;},
    };
    let is_empty = {
        let idents = children.iter().chain(scf.iter()).map(|c| {
            let ident = c.original.ident.as_ref().unwrap();
            quote! {#ident}
        });
        quote! {
            let has_child_to_write = #(#idents ||)* has_text;
            let is_empty = !has_child_to_write && !#has_untag_fields;
        }
    };
    quote! {
        #(#children_init)*
        #(#scf_init)*
        #text_init
        #is_empty
    }
}
