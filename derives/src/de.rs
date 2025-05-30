use quote::{format_ident, quote};
use syn::DeriveInput;

use crate::{
    case::parse_case,
    container::{self, Container, EleType, FieldsSummary, Generic, StructField},
};

pub fn get_de_impl_block(input: DeriveInput) -> proc_macro2::TokenStream {
    let container = Container::from_ast(&input, container::Derive::Deserialize)
        .expect("Failed to parse container");
    if let Err(e) = container.validate() {
        return syn::Error::new_spanned(&input, e.to_string()).to_compile_error();
    }
    if container.is_enum() {
        get_de_enum_impl_block(container)
    } else {
        get_de_struct_impl_block(container)
    }
}

pub fn get_de_enum_impl_block(container: Container) -> proc_macro2::TokenStream {
    macro_rules! children_branches {
        ($attrs:expr, $b:expr) => {
            container.enum_variants.iter().map(|v| {
                if matches!(&v.ele_type, EleType::Text) {
                    return quote! {};
                }
                let name = v.name.as_ref().expect("should have name");
                let ty_opt = v.ty;
                let ident = v.ident;
                if let Some(field_ty) = ty_opt {
                    let generic_info = crate::container::get_generics(field_ty);

                    if generic_info.is_boxed() {
                        let inner_ty = generic_info.get_boxed().expect("Boxed type should have an inner type");
                        quote! {
                            #name => {
                                let _r = <#inner_ty as ::xmlserde::XmlDeserialize>::deserialize(#name, reader, $attrs, $b);
                                return Self::#ident(Box::new(_r));
                            }
                        }
                    } else {
                        quote! {
                            #name => {
                                let _r = <#field_ty as ::xmlserde::XmlDeserialize>::deserialize(#name, reader, $attrs, $b);
                                return Self::#ident(_r);
                            }
                        }
                    }
                } else {
                    quote! {
                        #name => {
                            return Self::#ident;
                        }
                    }
                }
            })
        };
    }
    let mut text_opt = None;
    let mut text_ident = None;
    container.enum_variants.iter().for_each(|v| {
        if !matches!(&v.ele_type, EleType::Text) {
            return;
        }

        if text_opt.is_some() {
            panic!("should only have one `text` type")
        }

        text_opt = Some(v.ty.expect("expect type"));
        text_ident = Some(v.ident);
    });

    let text_function = if let Some(text_ty) = text_opt {
        let ident = text_ident.expect("should have ident for text");
        quote! {
            fn __deserialize_from_text(s: &str) -> Option<Self> {
                Some(Self::#ident(<#text_ty as ::xmlserde::XmlValue>::deserialize(s).unwrap()))
            }
        }
    } else {
        quote! {}
    };
    let ident = &container.original.ident;
    let (impl_generics, type_generics, where_clause) = container.original.generics.split_for_impl();
    let event_start_branches = children_branches!(_s.attributes(), false);
    let event_empty_branches = children_branches!(_s.attributes(), true);
    let children_tags = container
        .enum_variants
        .iter()
        .filter(|v| matches!(v.ele_type, EleType::Child))
        .map(|v| {
            let name = v.name.as_ref().expect("should have `name` for `child`");
            quote! {#name}
        });
    let exact_tags = children_branches!(attrs, is_empty);
    let get_roots = if !container.roots.is_empty() {
        let roots = container.get_root_names();
        quote! {
            fn de_roots() -> Vec<&'static [u8]> {
                vec![#(#roots),*]
            }
        }
    } else {
        quote! {}
    };
    let rename_all = rename_all(&container);
    quote! {
        #[allow(unused_assignments)]
        impl #impl_generics ::xmlserde::XmlDeserialize for #ident #type_generics #where_clause {
            fn deserialize<B: std::io::BufRead>(
                tag: &[u8],
                reader: &mut ::xmlserde::quick_xml::Reader<B>,
                attrs: ::xmlserde::quick_xml::events::attributes::Attributes,
                is_empty: bool,
            ) -> Self {
                use ::xmlserde::quick_xml::events::*;
                match tag {
                    #(#exact_tags)*
                    _ => {},
                }
                let mut buf = Vec::<u8>::new();
                let mut result = Option::<Self>::None;
                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::End(e)) if e.name().into_inner() == tag => {
                            break
                        },
                        Ok(Event::Start(_s)) => match _s.name().into_inner() {
                            #(#event_start_branches)*
                            _ => {},
                        },
                        Ok(Event::Empty(_s)) => match _s.name().into_inner() {
                            #(#event_empty_branches)*
                            _ => {},
                        }
                        Ok(Event::Eof) => break,
                        Err(_) => break,
                        _ => {},
                    }
                }
                result.expect("did not find any tag")
            }

            fn __get_children_tags() -> Vec<&'static [u8]> {
                vec![#(#children_tags,)*]
            }

            #text_function

            fn __is_enum() -> bool {
                true
            }

            #get_roots
            #rename_all
        }
    }
}

pub fn get_de_struct_impl_block(container: Container) -> proc_macro2::TokenStream {
    let result = get_result(&container.struct_fields);
    let summary = FieldsSummary::from_fields(&container.struct_fields);
    let fields_init = get_fields_init(&summary);
    let result_untagged_structs = get_untagged_struct_fields_result(&summary.untagged_structs);
    let FieldsSummary {
        children,
        text,
        attrs,
        self_closed_children,
        untagged_enums,
        untagged_structs,
    } = summary;
    let get_children_tags = if !children.is_empty() || !untagged_enums.is_empty() {
        let container = container.clone();
        let names = children.iter().map(|f| {
            let n = container.get_field_name(f).unwrap_or_else(|| {
                let ident = f
                    .original
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "<unnamed>".to_string());
                panic!("No name or mapped_names for field: {}", ident)
            });
            quote! {#n}
        });
        let untagged_enums = untagged_enums.iter().map(|f| {
            let ty = match &f.generic {
                | Generic::Vec(t) => t,
                | Generic::Opt(t) => t,
                | Generic::Boxed(t) => t,
                | Generic::None => &f.original.ty,
            };
            quote! {#ty::__get_children_tags()}
        });
        quote! {
            fn __get_children_tags() -> Vec<&'static [u8]> {
                let mut r: Vec<&'static [u8]> = vec![#(#names,)*];
                #(r.extend(#untagged_enums.into_iter());)*
                r
            }
        }
    } else {
        quote! {}
    };
    let attr_len = attrs.len();
    let sfc_len = self_closed_children.len();
    let vec_init = get_vec_init(&children);
    let attr_branches = attrs.into_iter().map(|a| get_attr_branch(&container, &a));
    let child_branches =
        children_match_branch(&container, &children, &untagged_enums, &untagged_structs);
    let sfc_branch = sfc_match_branch(self_closed_children);
    let ident = &container.original.ident;
    let (impl_generics, type_generics, where_clause) = container.original.generics.split_for_impl();
    let text_branch = text.map(text_match_branch);
    // Only those structs with only children can be untagged
    let deserialize_from_unparsed =
        if !children.is_empty() && attr_len == 0 && sfc_len == 0 && untagged_enums.is_empty() {
            get_deserialize_from_unparsed(&children)
        } else {
            quote! {}
        };
    let encounter_unknown = if container.deny_unknown {
        quote! {
            let _field = std::str::from_utf8(_field).unwrap();
            panic!("encoutnering unknown field: {:#?}", _field)
        }
    } else {
        quote! {}
    };
    let encounter_unknown_branch = quote! {
        Ok(Event::Empty(_s)) => {
            let _field = _s.name().into_inner();
            #encounter_unknown
        }
        Ok(Event::Start(_s)) => {
            let _field = _s.name().into_inner();
            #encounter_unknown
        }
    };

    // Get the transformed root names for comparison
    let root_names = container.get_root_names();
    let always_emit_roots = !container.roots.is_empty();
    let root_comparison = if always_emit_roots {
        let root_names_refs: Vec<_> = root_names.iter().collect();
        quote! {
            let valid_roots: &[&[u8]] = &[#(#root_names_refs),*];
            if !valid_roots.iter().any(|r| r.eq_ignore_ascii_case(tag)) {
                let valid_roots_str: Vec<String> = valid_roots.iter()
                    .map(|r| String::from_utf8_lossy(r).to_string())
                    .collect();
                let received_tag = String::from_utf8_lossy(tag).to_string();
                panic!("Expected one of root tags {:?}, got {:?}", valid_roots_str, received_tag);
            }
        }
    } else {
        quote! {}
    };
    let get_roots = if always_emit_roots {
        let root_names_refs: Vec<_> = root_names.iter().collect();
        quote! {
            fn de_roots() -> Vec<&'static [u8]> {
                vec![#(#root_names_refs),*]
            }
        }
    } else {
        quote! {}
    };

    let rename_all = rename_all(&container);

    quote! {
        #[allow(unused_assignments)]
        impl #impl_generics ::xmlserde::XmlDeserialize for #ident #type_generics #where_clause {
            fn deserialize<B: std::io::BufRead>(
                tag: &[u8],
                reader: &mut ::xmlserde::quick_xml::Reader<B>,
                attrs: ::xmlserde::quick_xml::events::attributes::Attributes,
                is_empty: bool,
            ) -> Self {
                #root_comparison
                #fields_init
                attrs.into_iter().for_each(|attr| {
                    if let Ok(attr) = attr {
                        match attr.key.into_inner() {
                            #(#attr_branches)*
                            _ => {
                                let _field = attr.key.into_inner();
                                #encounter_unknown;
                            },
                        }
                    }
                });
                let mut buf = Vec::<u8>::new();
                use ::xmlserde::quick_xml::events::Event;
                #vec_init
                if is_empty {} else {
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::End(e)) if e.name().into_inner() == tag => {
                                break
                            },
                            #sfc_branch
                            #child_branches
                            #text_branch
                            #encounter_unknown_branch
                            Ok(Event::Eof) => break,
                            Err(_) => break,
                            _ => {},
                        }
                    }
                }
                #result_untagged_structs
                Self {
                    #result
                }
            }
            #get_roots
            #rename_all
            #get_children_tags
            #deserialize_from_unparsed
        }
    }
}

fn get_untagged_struct_fields_result(fileds: &[StructField]) -> proc_macro2::TokenStream {
    let branch = fileds.iter().map(|f| {
    let ident = f.original.ident.as_ref().unwrap();
    let ty = &f.original.ty;
    let ident_unparsed_array = format_ident!("{}_unparseds", ident);
    let ident_opt_unparsed_array = format_ident!("{}_opt_unparseds", ident);
    match f.generic {
      | Generic::Vec(_) => unreachable!(),
      | Generic::Opt(_t) => quote! {
          if #ident_opt_unparsed_array.len() > 0 {
              #ident = Some(#_t::__deserialize_from_unparsed_array(#ident_opt_unparsed_array));
          }
      },
      | Generic::Boxed(inner_ty) => quote! {
          if #ident_unparsed_array.len() > 0 {
              #ident = Some(Box::new(#inner_ty::__deserialize_from_unparsed_array(#ident_unparsed_array)));
          }
      },
      | Generic::None => quote! {
          if #ident_unparsed_array.len() > 0 {
              #ident = Some(#ty::__deserialize_from_unparsed_array(#ident_unparsed_array));
          }
      },
    }
  });

    quote! {#(#branch)*}
}

fn get_result(fields: &[StructField]) -> proc_macro2::TokenStream {
    let branch = fields.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        if f.is_required() {
            quote! {
                #ident: #ident.unwrap(),
            }
        } else {
            quote! {
                #ident,
            }
        }
    });
    quote! {#(#branch)*}
}

fn get_fields_init(fields: &FieldsSummary) -> proc_macro2::TokenStream {
    let attrs_inits = fields.attrs.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        let ty = &f.original.ty;
        match &f.default {
            | Some(p) => {
                quote! {let mut #ident = #p();}
            },
            | None => {
                if let Some(opt) = f.generic.get_opt() {
                    quote! {
                        let mut #ident = Option::<#opt>::None;
                    }
                } else {
                    quote! {let mut #ident = Option::<#ty>::None;}
                }
            },
        }
    });
    let children_inits = fields.children.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        let ty = &f.original.ty;
        match &f.default {
            | Some(p) => {
                quote! {
                    let mut #ident = #p();
                }
            },
            | None => match f.generic {
                | Generic::Vec(v) => {
                    quote! {
                        let mut #ident = Vec::<#v>::new();
                    }
                },
                | Generic::Opt(opt) => {
                    quote! {
                        let mut #ident = Option::<#opt>::None;
                    }
                },
                | Generic::Boxed(inner_ty) => {
                    quote! {
                        let mut #ident = Option::<Box<#inner_ty>>::None;
                    }
                },
                | Generic::None => {
                    quote! {
                        let mut #ident = Option::<#ty>::None;
                    }
                },
            },
        }
    });
    let text_init = match &fields.text {
        | Some(f) => {
            let ident = f.original.ident.as_ref().unwrap();
            let ty = match f.generic {
                | Generic::Vec(_) => panic!("text element should not be Vec<T>"),
                | Generic::Opt(t) => t,
                | Generic::Boxed(t) => t,
                | Generic::None => &f.original.ty,
            };
            // let ty = &f.original.ty;
            match &f.default {
                | Some(e) => {
                    quote! {
                            let mut #ident = #e();
                    }
                },
                | None => {
                    quote! {
                        let mut #ident = Option::<#ty>::None;
                    }
                },
            }
        },
        | None => quote! {},
    };
    let sfc_init = fields.self_closed_children.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        quote! {
            let mut #ident = false;
        }
    });
    let untagged_enums_init = fields.untagged_enums.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();

        if let Some(path) = &f.default {
            return quote! {let mut #ident = #path();};
        }

        let ty = &f.original.ty;
        match f.generic {
            | Generic::Vec(t) => {
                quote! {
                    let mut #ident = Vec::<#t>::new();
                }
            },
            | Generic::Opt(t) => {
                quote! {
                    let mut #ident = Option::<#t>::None;
                }
            },
            | Generic::Boxed(inner_ty) => {
                quote! {
                    let mut #ident = Option::<Box<#inner_ty>>::None;
                }
            },
            | Generic::None => {
                quote! {
                    let mut #ident = Option::<#ty>::None;
                }
            },
        }
    });

    let untagged_structs_init = fields.untagged_structs.iter().map(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        if let Some(path) = &f.default {
            return quote! {let mut #ident = #path();};
        }
        let ident_unparsed_array = format_ident!("{}_unparseds", ident);
        let ident_opt_unparsed_array = format_ident!("{}_opt_unparseds", ident);

        let ty = &f.original.ty;
        match f.generic {
            | Generic::Vec(_t) => {
                quote! {
                    unreachable!()
                }
            },
            | Generic::Opt(t) => {
                quote! {
                    let mut #ident = Option::<#t>::None;
                    let mut #ident_opt_unparsed_array = Vec::new();
                }
            },
            | Generic::Boxed(inner_ty) => {
                quote! {
                    let mut #ident = Option::<Box<#inner_ty>>::None;
                    let mut #ident_unparsed_array = Vec::new();
                }
            },
            | Generic::None => {
                quote! {
                    let mut #ident = Option::<#ty>::None;
                    let mut #ident_unparsed_array = Vec::new();
                }
            },
        }
    });
    quote! {
        #(#attrs_inits)*
        #(#sfc_init)*
        #(#children_inits)*
        #text_init
        #(#untagged_enums_init)*
        #(#untagged_structs_init)*
    }
}

fn get_deserialize_from_unparsed(children: &[StructField]) -> proc_macro2::TokenStream {
    let init = children.iter().map(|c| {
        let ident = c.original.ident.as_ref().unwrap();
        if let Some(path) = &c.default {
            return quote! {
                let mut #ident = #path();
            };
        }
        match &c.generic {
            | Generic::Vec(_) => quote! {let mut #ident = vec![];},
            | Generic::Opt(_) => quote! {let mut #ident = None;},
            | Generic::Boxed(_) => quote! {let mut #ident = None;},
            | Generic::None => quote! {let mut #ident = None;},
        }
    });
    let body = children.iter().map(|c| {
        let name = c.name.as_ref().unwrap_or_else(|| &c.mapped_names[0]);
        let original_type = &c.original.ty;
        let ident = c.original.ident.as_ref().unwrap();
        match &c.generic {
            | Generic::Vec(t) => {
                quote! {
                    #name => {
                        #ident.push(content.deserialize_to::<#t>().unwrap());
                    }
                }
            },
            | Generic::Opt(t) => {
                quote! {
                    #name => {
                        #ident = Some(content.deserialize_to::<#t>().unwrap());
                    }
                }
            },
            | Generic::Boxed(t) => {
                quote! {
                    #name => {
                        #ident = Some(Box::new(content.deserialize_to::<#t>().unwrap()));
                    }
                }
            },
            | Generic::None => {
                if c.default.is_some() {
                    quote! {
                        #name => {
                            #ident = content.deserialize_to::<#original_type>().unwrap();
                        }
                    }
                } else {
                    quote! {
                        #name => {
                            #ident = Some(content.deserialize_to::<#original_type>().unwrap());
                        }
                    }
                }
            },
        }
    });
    let result = {
        let idents = children.iter().map(|c| {
            let ident = c.original.ident.as_ref().unwrap();
            if c.is_required() {
                quote! {
                    #ident: #ident.expect("missing field")
                }
            } else {
                quote! {
                    #ident
                }
            }
        });
        quote! {
            Self {
                #(#idents),*
            }
        }
    };
    quote! {
        fn __deserialize_from_unparsed_array(array: Vec<(&'static [u8], ::xmlserde::Unparsed)>) -> Self {
            #(#init)*
            array.into_iter().for_each(|(tag, content)| {
                match tag {
                    #(#body),*
                    _ => {},
                }
            });
            #result
        }
    }
}

fn get_vec_init(children: &[StructField]) -> proc_macro2::TokenStream {
    let vec_inits = children
        .iter()
        .filter(|c| c.generic.is_vec())
        .map(|c| match &c.vec_size {
            | Some(lit) => {
                let vec_ty = &c.generic.get_vec().unwrap();
                let ident = c.original.ident.as_ref().unwrap();
                match lit {
                    | syn::Lit::Str(s) => {
                        let path = container::parse_lit_str::<syn::Expr>(s).unwrap();
                        quote! {
                            #ident = Vec::<#vec_ty>::with_capacity(#path as usize);
                        }
                    },
                    | syn::Lit::Int(i) => {
                        quote! {
                            #ident = Vec::<#vec_ty>::with_capacity(#i);
                        }
                    },
                    | _ => panic!(""),
                }
            },
            | None => {
                quote! {}
            },
        });
    quote! {
        #(#vec_inits)*
    }
}

fn sfc_match_branch(fields: Vec<StructField>) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {};
    }
    let mut idents = vec![];
    let mut tags = vec![];
    fields.iter().for_each(|f| {
        if !matches!(f.ty, EleType::SelfClosedChild) {
            panic!("")
        }
        let tag = f.name.as_ref().unwrap();
        tags.push(tag);
        let ident = f.original.ident.as_ref().unwrap();
        idents.push(ident);
    });
    quote! {
        #(Ok(Event::Empty(__s)) if __s.name().into_inner() == #tags => {
            #idents = true;
        })*
    }
}

fn get_attr_branch(container: &Container, field: &StructField) -> proc_macro2::TokenStream {
    let ident = field.original.ident.as_ref().unwrap();
    let t = &field.original.ty;
    let tag_owned;
    let tag = if !matches!(
        field.ty,
        EleType::Untag | EleType::UntaggedEnum | EleType::UntaggedStruct
    ) {
        tag_owned = container
            .get_field_name(field)
            .expect("Field must have a name, mapped_names, or be covered by rename_all");
        &tag_owned
    } else {
        field
            .name
            .as_ref()
            .or_else(|| field.mapped_names.first())
            .expect("Untagged types must have a name or mapped_names")
    };
    let mapped_tags = if field.name.is_some() {
        field.mapped_names.iter().collect::<Vec<_>>()
    } else if field.mapped_names.len() > 1 {
        field.mapped_names[1..].iter().collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if field.generic.is_opt() {
        let opt_ty = field.generic.get_opt().unwrap();
        quote! {
            #tag #(| #mapped_tags)* | _ if attr.key.into_inner().eq_ignore_ascii_case(#tag) #(| attr.key.into_inner().eq_ignore_ascii_case(#mapped_tags))* => {
                use xmlserde::{XmlValue, XmlDeserialize};
                let s = String::from_utf8(attr.value.into_iter().map(|c| *c).collect()).unwrap();
                match #opt_ty::deserialize(&s) {
                    Ok(__v) => {
                        #ident = Some(__v);
                    },
                    Err(_) => {
                        panic!("deserialize failed in attr opt")
                    },
                }
            }
        }
    } else {
        let tt = if field.is_required() {
            quote! {#ident = Some(__v);}
        } else {
            quote! {#ident = __v;}
        };
        quote! {
            #tag #(| #mapped_tags)* | _ if attr.key.into_inner().eq_ignore_ascii_case(#tag) #(| attr.key.into_inner().eq_ignore_ascii_case(#mapped_tags))* => {
                use xmlserde::{XmlValue, XmlDeserialize};
                let __s = String::from_utf8(attr.value.into_iter().map(|c| *c).collect()).unwrap();
                match #t::deserialize(&__s) {
                    Ok(__v) => {
                        #tt
                    },
                    Err(_) => {
                        panic!("deserialize failed in attr")
                    },
                }
            }
        }
    }
}

fn text_match_branch(field: StructField) -> proc_macro2::TokenStream {
    if !matches!(field.ty, EleType::Text) {
        panic!("")
    }
    let ident = field.original.ident.as_ref().expect("should have idnet");
    // let t = &field.original.ty;
    let (t, is_opt) = match field.generic {
        | Generic::Vec(_) => panic!("text element should not be Vec<T>"),
        | Generic::Opt(ty) => (ty, true),
        | Generic::Boxed(t) => (t, true),
        | Generic::None => (&field.original.ty, false),
    };
    let tt = if field.is_required() || is_opt {
        quote! {#ident = Some(__v);}
    } else {
        quote! {#ident = __v;}
    };
    quote! {
        Ok(Event::Text(__s)) => {
            use ::xmlserde::{XmlValue, XmlDeserialize};
            let __r = __s.unescape().unwrap();
            match #t::deserialize(&__r) {
                Ok(__v) => {
                    // #ident = v;
                    #tt
                },
                Err(_) => {
                    panic!("deserialize failed in text element")
                }
            }
        },
    }
}

fn untag_text_enum_branches(untags: &[StructField]) -> proc_macro2::TokenStream {
    if untags.is_empty() {
        return quote! {};
    }

    let mut branches: Vec<proc_macro2::TokenStream> = vec![];
    untags.iter().for_each(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        let ty = &f.original.ty;
        let branch = match f.generic {
            | Generic::Vec(ty) => {
                quote! {
                    if let Some(t) = #ty::__deserialize_from_text(&_str) {
                        #ident.push(t);
                    }
                }
            },
            | Generic::Opt(ty) => {
                quote! {
                    if let Some(t) = #ty::__deserialize_from_text(&_str) {
                        #ident = Some(t);
                    }
                }
            },
            | Generic::Boxed(inner_ty) => {
                quote! {
                    if let Some(t) = #inner_ty::__deserialize_from_text(&_str) {
                        #ident = Some(Box::new(t));
                    }
                }
            },
            | Generic::None => {
                quote! {
                    if let Some(t) = #ty::__deserialize_from_text(&_str) {
                        #ident = Some(t);
                    }
                }
            },
        };
        branches.push(branch);
    });

    quote! {#(#branches)*}
}

fn untag_enums_match_branch(fields: &[StructField]) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {};
    }
    let mut branches: Vec<proc_macro2::TokenStream> = vec![];
    fields.iter().for_each(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        let ty = &f.original.ty;
        let branch = match f.generic {
            | Generic::Vec(ty) => {
                quote! {
                    _ty if #ty::__get_children_tags().contains(&_ty) => {
                        #ident.push(#ty::deserialize(_ty, reader, s.attributes(), is_empty));
                    }
                }
            },
            | Generic::Opt(ty) => {
                quote! {
                    _ty if #ty::__get_children_tags().contains(&_ty) => {
                        #ident = Some(#ty::deserialize(_ty, reader, s.attributes(), is_empty));
                    }
                }
            },
            | Generic::Boxed(inner_ty) => {
                quote! {
                    _ty if #inner_ty::__get_children_tags().contains(&_ty) => {
                        #ident = Some(Box::new(#inner_ty::deserialize(_ty, reader, s.attributes(), is_empty)));
                    }
                }
            },
            | Generic::None => {
                quote! {
                    _t if #ty::__get_children_tags().contains(&_t) => {
                        #ident = Some(#ty::deserialize(_t, reader, s.attributes(), is_empty));
                    }
                }
            },
        };
        branches.push(branch);
    });
    quote! {
        #(#branches)*
    }
}

fn untag_structs_match_branch(fields: &[StructField]) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {};
    }
    let mut branches: Vec<proc_macro2::TokenStream> = vec![];

    fields.iter().for_each(|f| {
    let ident = f.original.ident.as_ref().unwrap();
    let ty = &f.original.ty;
    let ident_unparsed_array = format_ident!("{}_unparseds", ident);
    let ident_opt_unparsed_array = format_ident!("{}_opt_unparseds", ident);
    // let name = f.name.as_ref().expect("should have `name` for `child` type");
    let branch = match f.generic {
      | Generic::Vec(_) => unreachable!(),
      | Generic::Opt(t) => quote! {
          _t if #t::__get_children_tags().contains(&_t) => {
              let _r = ::xmlserde::Unparsed::deserialize(_t, reader, s.attributes(), is_empty);
              let _tags = #t::__get_children_tags();
              if !_tags.is_empty() {
                  let idx = _tags.binary_search(&_t).unwrap();
                  #ident_opt_unparsed_array.push((_tags[idx], _r));
              }
          }
      },
      | Generic::Boxed(inner_ty) => quote! {
          _t if #inner_ty::__get_children_tags().contains(&_t) => {
              let _r = ::xmlserde::Unparsed::deserialize(_t, reader, s.attributes(), is_empty);
              let _tags = #inner_ty::__get_children_tags();
              if !_tags.is_empty() {
                  let idx = _tags.binary_search(&_t).unwrap();
                  #ident_unparsed_array.push((_tags[idx], _r));
              }
          }
      },
      | Generic::None => quote! {
          _t if #ty::__get_children_tags().contains(&_t) => {
              let _r = ::xmlserde::Unparsed::deserialize(_t, reader, s.attributes(), is_empty);
              let _tags = #ty::__get_children_tags();
              if !_tags.is_empty() {
                  let idx = _tags.binary_search(&_t).unwrap();
                  #ident_unparsed_array.push((_tags[idx], _r));
              }
          }
      },
    };
    branches.push(branch);
  });
    quote! {
        #(#branches)*
    }
}

fn children_match_branch(
    container: &Container,
    fields: &[StructField],
    untagged_enums: &[StructField],
    untagged_structs: &[StructField],
) -> proc_macro2::TokenStream {
    if fields.is_empty() && untagged_enums.is_empty() && untagged_structs.is_empty() {
        return quote! {};
    }
    let mut branches = vec![];
    fields.iter().for_each(|f| {
        let ident = f.original.ident.as_ref().unwrap();
        let t = &f.original.ty;

        let type_for_is_enum_check = match &f.generic {
            Generic::Opt(inner_ty) | Generic::Boxed(inner_ty) | Generic::Vec(inner_ty) => quote! { #inner_ty },
            Generic::None => quote! { #t },
        };

        let field_tag_name = container.get_field_name(f).unwrap_or_else(|| {
            let ident_str = f.original.ident.as_ref().map(|i| i.to_string()).unwrap_or_else(|| "<unnamed>".to_string());
            panic!("No name or mapped_names for field: {} in children_match_branch", ident_str)
        });

        let branch = match f.generic {
            Generic::Vec(ref vec_ty) => {
                quote! {
                    _tag if #type_for_is_enum_check::__is_enum() && _tag == #field_tag_name.as_ref() => {
                        let __ele = <#vec_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident.push(__ele);
                    }
                    _tag if !(#type_for_is_enum_check::__is_enum()) && _tag == #field_tag_name.as_ref() => {
                        let __ele = <#vec_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident.push(__ele);
                    }
                }
            },
            Generic::Opt(ref opt_ty) => {
                quote! {
                    _tag if #type_for_is_enum_check::__is_enum() && _tag == #field_tag_name.as_ref() => {
                        let __f = <#opt_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident = Some(__f);
                    }
                    _tag if !(#type_for_is_enum_check::__is_enum()) && _tag == #field_tag_name.as_ref() => {
                        let __f = <#opt_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident = Some(__f);
                    }
                }
            },
            Generic::Boxed(ref inner_ty) => {
                 quote! {
                    _tag if #type_for_is_enum_check::__is_enum() && _tag == #field_tag_name.as_ref() => {
                        let __f = <#inner_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident = Some(Box::new(__f));
                    }
                    _tag if !(#type_for_is_enum_check::__is_enum()) && _tag == #field_tag_name.as_ref() => {
                        let __f = <#inner_ty as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #ident = Some(Box::new(__f));
                    }
                }
            },
            Generic::None => {
                let assignment = if f.default.is_some() {
                    quote! { #ident = __f; }
                } else {
                    quote! { #ident = Some(__f); }
                };
                quote! {
                     _tag if #type_for_is_enum_check::__is_enum() && _tag == #field_tag_name.as_ref() => {
                        let __f = <#t as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #assignment
                    }
                    _tag if !(#type_for_is_enum_check::__is_enum()) && _tag == #field_tag_name.as_ref() => {
                        let __f = <#t as ::xmlserde::XmlDeserialize>::deserialize(_tag, reader, s.attributes(), is_empty);
                        #assignment
                    }
                }
            },
        };
        branches.push(branch);
    });
    let untagged_enums_branches = untag_enums_match_branch(untagged_enums);
    let untagged_structs_branches = untag_structs_match_branch(untagged_structs);
    let untag_text_enum = untag_text_enum_branches(untagged_enums);
    quote! {
        Ok(Event::Empty(s)) => {
            let is_empty = true;
            let current_tag = s.name().into_inner();
            match current_tag {
                #(#branches)*
                #untagged_enums_branches
                #untagged_structs_branches
                _ => {},
            }
        }
        Ok(Event::Start(s)) => {
            let is_empty = false;
            let current_tag = s.name().into_inner();
            match current_tag {
                #(#branches)* // branches are the if _tag == ... constructs
                #untagged_enums_branches
                #untagged_structs_branches
                _ => {},
            }
        }
        Ok(Event::Text(t)) => {
            use ::xmlserde::{XmlValue, XmlDeserialize};
            let _str = t.unescape().expect("failed to unescape string");
            if _str.trim() != "" {
                #untag_text_enum
            }
        }
    }
}

fn rename_all(container: &Container) -> proc_macro2::TokenStream {
    if let Some(rename_all) = &container.rename_all {
        let case = parse_case(rename_all).expect("Invalid case for rename_all");
        let case_str = case.to_rename_all_variant();
        let case_ident = format_ident!("{}", case_str);
        quote! {
            fn rename_all() -> ::xmlserde::xmlserde_shared::Case {
                ::xmlserde::xmlserde_shared::Case::#case_ident
            }
        }
    } else {
        quote! {
            fn rename_all() -> ::xmlserde::xmlserde_shared::Case {
                ::xmlserde::xmlserde_shared::Case::None
            }
        }
    }
}
