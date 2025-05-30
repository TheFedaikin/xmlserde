//! `xmlserde` is another tool for serializing and deserializing XML. It is designed
//! for easy and clear use.
//!
//! Please add these dependencies in your `Cargo.toml`.
//! ```toml
//! xmlserde = "0.10"
//! xmlserde_derives = "0.10"
//! ```
//!
//! # Deserialize
//! Suppose that XML struct is to be deserialized as below:
//! ```xml
//! <person age="8">
//!     <name>Jeremy</name>
//!     <pet t="cat">Tom</pet>
//!     <pet t="dog">Spike</pet>
//! </person>
//! ```
//! You can create a struct and derive the `XmlDeserialize` from `xmlserde_derives`, like:
//! ```ignore
//! use xmlserde_derives::XmlDeserialize;
//! #[derive(XmlDeserialize)]
//! pub struct Person {
//!     #[xmlserde(name = b"age", ty = "attr")]
//!     pub age: u16,
//!     #[xmlserde(name = b"pet", ty = "child")]
//!     pub pets: Vec<Pet>,
//! }
//!
//! #[derive(XmlDeserialize)]
//! pub struct Pet {
//!     #[xmlserde(name = b"t", ty = "attr")]
//!     pub t: String,
//!     #[xmlserde(ty = "text")]
//!     pub name: String,
//! }
//! ```
//! In `xmlserde`, you need to declare clearly that which tag and which type you are going to
//! `serde`. Notice that it is a binary string for the `name`.
//!
//! # Serialize
//! As for serializing, you need to derive the `XmlSerialize`.
//!
//! # Enum
//! ## For attribute value
//! Please check in `xml_serde_enum` section.
//!
//! ## For children element
//! You can define an enum like this.
//! ```ignore
//! #[derive(XmlSerialize, Deserialize)]
//! pub enum Pet{
//!     #[xmlserde(name = b"dog")]
//!     Dog(Dog),
//!     #[xmlserde(name = b"cat")]
//!     Cat(Cat),
//! }
//! pub struct Dog{}
//! pub struct Cat{}
//! ```
//! In a field whose type is an `enum`, we can use `ty = untag`:
//! ```ignore
//! #[derive(XmlSerialize, Deserialize)]
//! pub struct Person {
//!     #[xmlserde(ty="untag")]
//!     pub pet: Pet,
//! }
//! ```
//! In this case, `Person` can be serialized as
//! ```xml
//! <person>
//!     <dog>
//!     ...
//!     </dog>
//! </person>
//! ```
//! or
//! ```xml
//! <person>
//!     <cat>
//!     ...
//!     </cat>
//! </person>
//! ```
//!
//! # Attributes
//! - name: the tag of the XML element.
//! - vec_size: creating a vector with the given capacity before deserilizing a element lists.
//!   `vec_size=4` or if your initial capacity is defined in an attr, you can use like this
//!   `vec_size="cnt"`.
//! - default: assigning a parameter-free function to create a default value for a certain field.
//!   Notice that it requires the type of this value impls `Eq` and it will skip serializing when
//!   the value equals to the default one.
//! - untag: see the `Enum` above.
//!
//! # Examples
//! Please see [LogiSheets](https://github.com/proclml/LogiSheets/tree/master/crates/workbook) for examples.

use std::{
    fmt::Debug,
    io::{BufRead, Write},
};

// We republic the `quick_xml` here is for helping the `derives` crate import
// it easily. In this way users don't need to import the `quick-xml` on
// their own.
pub use quick_xml;
use quick_xml::events::Event;
pub use xmlserde_shared;
use xmlserde_shared::Case;

pub trait XmlSerialize {
    fn serialize<W: Write>(&self, tag: &[u8], writer: &mut quick_xml::Writer<W>);
    fn ser_roots() -> Vec<&'static [u8]> {
        vec![]
    }
}

impl<T: XmlSerialize> XmlSerialize for Option<T> {
    fn serialize<W: Write>(&self, tag: &[u8], writer: &mut quick_xml::Writer<W>) {
        if let Some(t) = self {
            t.serialize(tag, writer)
        }
    }
}

impl<T: XmlSerialize> XmlSerialize for Vec<T> {
    fn serialize<W: Write>(&self, tag: &[u8], writer: &mut quick_xml::Writer<W>) {
        self.iter().for_each(|c| {
            c.serialize(tag, writer);
        });
    }
}

pub trait XmlDeserialize: Sized {
    fn deserialize<B: BufRead>(
        tag: &[u8],
        reader: &mut quick_xml::Reader<B>,
        attrs: quick_xml::events::attributes::Attributes,
        is_empty: bool,
    ) -> Self;

    fn de_roots() -> Vec<&'static [u8]> {
        vec![]
    }

    fn rename_all() -> Case {
        Case::None
    }

    /// A helper function used when ty = `untag`. It could help
    /// us to find out the children tags when deserializing
    fn __get_children_tags() -> Vec<&'static [u8]> {
        vec![]
    }

    /// A helper function used when handling the untag types.
    ///
    /// For a outside struct, it doesn't
    /// know how to deal with an untag type. The current solution is to treat them as `Unparsed`
    /// types first, and then pass them into this function to deserialize. Since the type is
    /// untagged, it doesn't require the attributes.
    fn __deserialize_from_unparsed_array(_array: Vec<(&'static [u8], Unparsed)>) -> Self {
        unreachable!("untagged types require having `child` types only")
    }

    /// A helper function for handling the untagged types.
    ///
    /// For efficiency, deserializing enums has no need to handle the untagged types by
    /// `__deserialize_from_unparsed_array` method. But we have no idea of whether this field is
    /// not enum or not, we make a helper function to discern it in the runtime.
    fn __is_enum() -> bool {
        false
    }

    fn __deserialize_from_text(_: &str) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }
}

/// `Unparsed` keeps the XML struct and will be serialized to XML with nothing change.
/// It is helpful when you are debugging on deserializeing certain element.
///
/// ```ignore
/// use xmlserde::Unparsed;
/// use xmlserde_derive::{XmlSerialize, XmlDeserialize};
///
/// #[derive(XmlSerialize, Deserialize)]
/// pub struct Person {
///     #[xmlserde(name=b"gender", ty = "attr")]
///     pub gender: Gender,
///     #[xmlserde(name=b"hobbies", ty = "child")]
///     pub hobbies: Unparsed
/// }
/// ```
/// In the example above, `<hobbies>` element keeps unchange after serializing and deserializing.
/// You can easily make a diff the former and latter version to check if other elments work well.
#[derive(Debug, Clone)]
pub struct Unparsed {
    data: Vec<Event<'static>>,
    attrs: Vec<(String, String)>,
}

impl XmlSerialize for Unparsed {
    fn serialize<W: Write>(&self, tag: &[u8], writer: &mut quick_xml::Writer<W>) {
        use quick_xml::events::*;
        let mut start = BytesStart::new(String::from_utf8_lossy(tag));
        self.attrs.iter().for_each(|(k, v)| {
            let k = k as &str;
            let v = v as &str;
            start.push_attribute((k, v));
        });
        if !self.data.is_empty() {
            let _ = writer.write_event(Event::Start(start));
            self.data.iter().for_each(|e| {
                let _ = writer.write_event(e.clone());
            });
            let _ = writer.write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(tag))));
        } else {
            let _ = writer.write_event(Event::Empty(start));
        }
    }
}

impl XmlDeserialize for Unparsed {
    fn deserialize<B: BufRead>(
        tag: &[u8],
        reader: &mut quick_xml::Reader<B>,
        attrs: quick_xml::events::attributes::Attributes,
        is_empty: bool,
    ) -> Self {
        use quick_xml::events::*;
        let mut attrs_vec = Vec::<(String, String)>::new();
        let mut data = Vec::<Event<'static>>::new();
        let mut buf = Vec::<u8>::new();
        attrs.into_iter().for_each(|a| {
            if let Ok(attr) = a {
                let key =
                    String::from_utf8(attr.key.into_inner().to_vec()).unwrap_or(String::from(""));
                let value = String::from_utf8(attr.value.to_vec()).unwrap_or(String::from(""));
                attrs_vec.push((key, value))
            }
        });
        if is_empty {
            return Unparsed {
                data,
                attrs: attrs_vec,
            };
        }
        loop {
            match reader.read_event_into(&mut buf) {
                | Ok(Event::End(e)) if e.name().into_inner() == tag => break,
                | Ok(Event::Eof) => break,
                | Err(_) => break,
                | Ok(e) => data.push(e.into_owned()),
            }
        }
        Unparsed {
            data,
            attrs: attrs_vec,
        }
    }

    fn __deserialize_from_unparsed_array(_array: Vec<(&'static [u8], Unparsed)>) -> Self {
        unreachable!(
            r#"seems you are using a struct having `attrs` or `text` as an UntaggedStruct"#
        )
    }
}

impl Unparsed {
    pub fn deserialize_to<T>(self) -> Result<T, String>
    where
        T: XmlDeserialize + Sized,
    {
        // TODO: Find a more efficient way
        let mut writer = quick_xml::Writer::new(Vec::new());
        let t = b"tmptag";
        self.serialize(t, &mut writer);
        let result = writer.into_inner();

        xml_deserialize_from_reader_with_root::<T, _>(result.as_slice(), t)
    }
}

/// The entry for serializing. `T` should have declared the `root` by `#[xmlserde(root=b"")]`
/// to tell the serializer the tag name of the root. This function will add the header needed for
/// a XML file.
pub fn xml_serialize_with_decl<T>(obj: T) -> String
where
    T: XmlSerialize,
{
    use quick_xml::events::BytesDecl;
    let mut writer = quick_xml::Writer::new(Vec::new());
    let decl = BytesDecl::new("1.0", Some("UTF-8"), Some("yes"));
    let _ = writer.write_event(Event::Decl(decl));
    let roots = T::ser_roots();
    if roots.is_empty() {
        panic!(r#"Expect a root element to serialize: #[xmlserde(root=b"tag")]"#);
    }
    obj.serialize(roots[0], &mut writer);
    String::from_utf8(writer.into_inner()).unwrap()
}

/// The entry for serializing. `T` should have declared the `root` by `#[xmlserde(root=b"")]`
/// to tell the serializer the tag name of the root.
pub fn xml_serialize<T>(obj: T) -> String
where
    T: XmlSerialize,
{
    let mut writer = quick_xml::Writer::new(Vec::new());
    let roots = T::ser_roots();
    if roots.is_empty() {
        panic!("Expect at least one root element");
    }
    obj.serialize(roots[0], &mut writer);
    String::from_utf8(writer.into_inner()).expect("decode error")
}

/// The entry for deserializing. `T` should have declared the `root` by `#[xmlserde(root=b"")]`
/// to tell the deserializer which tag is the start for deserializing.
/// ```ignore
/// use xmlserde_derives::XmlDeserialize;
/// #[derive(XmlDeserialize)]
/// #[xmlserde(root=b"person")]
/// pub struct Person {
///     #[xmlserde(name = b"age", ty = "attr")]
///     pub age: u16,
///     #[xmlserde(name = b"pet", ty = "child")]
///     pub pets: Vec<Pet>,
/// }
/// ```
pub fn xml_deserialize_from_reader<T, R>(mut reader: R) -> Result<T, String>
where
    T: XmlDeserialize,
    R: BufRead,
{
    let roots = T::de_roots();
    if roots.is_empty() {
        return Err(r#"#[xmlserde(root = b"tag")]"#.to_string());
    }
    // Read the entire input into a buffer
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let mut last_err = None;
    for root in &roots {
        let mut cursor = std::io::Cursor::new(&buf);
        match xml_deserialize_from_reader_with_root(&mut cursor, root) {
            | Ok(val) => return Ok(val),
            | Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "No matching root found".to_string()))
}

pub(crate) fn xml_deserialize_from_reader_with_root<T, R>(
    reader: R,
    root: &[u8],
) -> Result<T, String>
where
    T: XmlDeserialize,
    R: BufRead,
{
    let mut reader = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::<u8>::new();
    let rename_all = T::rename_all();
    let transformed_root = rename_all.transform(root);

    loop {
        match reader.read_event_into(&mut buf) {
            | Ok(Event::Start(start)) => {
                let name = start.name().into_inner();
                let transformed_name = rename_all.transform(name);
                if transformed_name == transformed_root {
                    let result = T::deserialize(root, &mut reader, start.attributes(), false);
                    return Ok(result);
                }
            },
            | Ok(Event::Empty(start)) => {
                let name = start.name().into_inner();
                let transformed_name = rename_all.transform(name);
                if transformed_name == transformed_root {
                    let result = T::deserialize(root, &mut reader, start.attributes(), true);
                    return Ok(result);
                }
            },
            | Ok(Event::Eof) => {
                return Err(format!(
                    "Cannot find the element: {}",
                    String::from_utf8_lossy(root)
                ))
            },
            | Err(e) => return Err(e.to_string()),
            | _ => {},
        }
    }
}

/// The entry for deserializing. `T` should have declared the `root` by `#[xmlserde(root=b"")]`
/// to tell the deserializer which tag is the start for deserializing.
/// ```ignore
/// use xmlserde_derives::XmlDeserialize;
/// #[derive(XmlDeserialize)]
/// #[xmlserde(root=b"person")]
/// pub struct Person {
///     #[xmlserde(name = b"age", ty = "attr")]
///     pub age: u16,
///     #[xmlserde(name = b"pet", ty = "child")]
///     pub pets: Vec<Pet>,
/// }
/// ```
pub fn xml_deserialize_from_str<T>(xml_str: &str) -> Result<T, String>
where
    T: XmlDeserialize,
{
    xml_deserialize_from_reader(xml_str.as_bytes())
}

pub trait XmlValue: Sized {
    fn serialize(&self) -> String;
    fn deserialize(s: &str) -> Result<Self, String>;
}

impl XmlValue for bool {
    fn serialize(&self) -> String {
        if *self {
            String::from("1")
        } else {
            String::from("0")
        }
    }

    fn deserialize(s: &str) -> Result<Self, String> {
        let s = s.to_ascii_lowercase();
        if s == "1" || s == "true" {
            Ok(true)
        } else if s == "0" || s == "false" {
            Ok(false)
        } else {
            Err(format!("Cannot parse {} into a boolean", s))
        }
    }
}

impl XmlValue for String {
    fn serialize(&self) -> String {
        self.to_owned()
    }

    fn deserialize(s: &str) -> Result<Self, String> {
        Ok(s.to_owned())
    }
}

macro_rules! impl_xml_value_for_num {
    ($num:ty) => {
        impl XmlValue for $num {
            fn serialize(&self) -> String {
                self.to_string()
            }

            fn deserialize(s: &str) -> Result<Self, String> {
                let r = s.parse::<$num>();
                match r {
                    | Ok(f) => Ok(f),
                    | Err(e) => Err(e.to_string()),
                }
            }
        }
    };
}

impl_xml_value_for_num!(i8);
impl_xml_value_for_num!(u8);
impl_xml_value_for_num!(i16);
impl_xml_value_for_num!(u16);
impl_xml_value_for_num!(i32);
impl_xml_value_for_num!(u32);
impl_xml_value_for_num!(i64);
impl_xml_value_for_num!(u64);
impl_xml_value_for_num!(i128);
impl_xml_value_for_num!(u128);
impl_xml_value_for_num!(isize);
impl_xml_value_for_num!(usize);
impl_xml_value_for_num!(f32);
impl_xml_value_for_num!(f64);
impl_xml_value_for_num!(std::num::NonZeroI8);
impl_xml_value_for_num!(std::num::NonZeroU8);
impl_xml_value_for_num!(std::num::NonZeroI16);
impl_xml_value_for_num!(std::num::NonZeroU16);
impl_xml_value_for_num!(std::num::NonZeroI32);
impl_xml_value_for_num!(std::num::NonZeroU32);
impl_xml_value_for_num!(std::num::NonZeroI64);
impl_xml_value_for_num!(std::num::NonZeroU64);
impl_xml_value_for_num!(std::num::NonZeroI128);
impl_xml_value_for_num!(std::num::NonZeroU128);
impl_xml_value_for_num!(std::num::NonZeroIsize);
impl_xml_value_for_num!(std::num::NonZeroUsize);
