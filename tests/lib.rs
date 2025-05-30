#[cfg(test)]
mod tests {

    use xmlserde::{xml_deserialize_from_str, xml_serialize, Unparsed, XmlValue};
    use xmlserde_derives::{XmlDeserialize, XmlSerdeEnum, XmlSerialize};

    #[test]
    fn xml_serde_enum_test() {
        #[derive(XmlSerdeEnum)]
        enum T {
            #[xmlserde(rename = "a")]
            A,
            #[xmlserde(rename = "b")]
            B,
            #[xmlserde(rename = "c")]
            C,
            #[xmlserde(other)]
            Other(String),
        }

        assert!(matches!(T::deserialize("c"), Ok(T::C)));
        assert!(matches!(T::deserialize("b"), Ok(T::B)));
        assert!(matches!(T::deserialize("a"), Ok(T::A)));
        assert!(matches!(
            T::deserialize("d2"),
            Ok(T::Other(x)) if x == "d2"
        ));
        assert_eq!((T::A).serialize(), "a");
    }

    #[test]
    fn default_for_child() {
        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"property")]
        struct Property {
            #[xmlserde(name = b"name", ty = "attr")]
            name: String,
        }

        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"properties")]
        struct InnerProperties {
            #[xmlserde(name = b"property", ty = "child")]
            properties: Vec<Property>,
        }

        #[derive(Default)]
        struct Properties(Vec<Property>);

        impl xmlserde::XmlDeserialize for Properties {
            fn deserialize<B: std::io::prelude::BufRead>(
                tag: &[u8],
                reader: &mut xmlserde::quick_xml::Reader<B>,
                attrs: xmlserde::quick_xml::events::attributes::Attributes,
                is_empty: bool,
            ) -> Self {
                let inner = InnerProperties::deserialize(tag, reader, attrs, is_empty);
                Self(inner.properties)
            }
        }

        #[derive(XmlDeserialize)]
        #[xmlserde(root = b"namespace")]
        struct Namespace {
            #[xmlserde(name = b"properties", ty = "child", default = "Properties::default")]
            properties: Properties,
        }

        let xml = r#"<namespace>
        </namespace>"#;
        let result = xml_deserialize_from_str::<Namespace>(xml).unwrap();
        assert!(result.properties.0.is_empty(),);

        let xml = r#"<namespace>
            <properties>
                <property name="test" />
            </properties>
        </namespace>"#;
        let result = xml_deserialize_from_str::<Namespace>(xml).unwrap();
        assert_eq!(result.properties.0[0].name, "test",);
    }

    #[test]
    fn self_closed_boolean_child() {
        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"font")]
        struct Font {
            #[xmlserde(name = b"b", ty = "sfc")]
            bold: bool,
            #[xmlserde(name = b"i", ty = "sfc")]
            italic: bool,
            #[xmlserde(name = b"size", ty = "attr")]
            size: f64,
        }
        let xml = r#"<font size="12.2">
            <b/>
            <i/>
        </font>"#;
        let result = xml_deserialize_from_str::<Font>(xml);
        match result {
            | Ok(f) => {
                assert!(f.bold);
                assert!(f.italic);
                assert_eq!(f.size, 12.2);
            },
            | Err(_) => panic!(),
        }
    }

    #[test]
    fn derive_deserialize_vec_with_init_size_from_attr() {
        #[derive(XmlDeserialize, Default)]
        pub struct Child {
            #[xmlserde(name = b"age", ty = "attr")]
            pub age: u16,
            #[xmlserde(ty = "text")]
            pub name: String,
        }
        fn default_zero() -> u32 {
            0
        }
        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"root")]
        pub struct Aa {
            #[xmlserde(name = b"f", ty = "child", vec_size = "cnt")]
            pub f: Vec<Child>,
            #[xmlserde(name = b"cnt", ty = "attr", default = "default_zero")]
            pub cnt: u32,
        }
        let xml = r#"<root cnt="2">
            <f age="15"> Tom</f>
            <f age="9">Jerry</f>
        </root>"#;
        let result = xml_deserialize_from_str::<Aa>(xml);
        match result {
            | Ok(result) => {
                assert_eq!(result.f.len(), 2);
                assert_eq!(result.cnt, 2);
                let mut child_iter = result.f.into_iter();
                let first = child_iter.next().unwrap();
                assert_eq!(first.age, 15);
                assert_eq!(first.name, String::from(" Tom"));
                let second = child_iter.next().unwrap();
                assert_eq!(second.age, 9);
                assert_eq!(second.name, String::from("Jerry"));
            },
            | Err(_) => panic!(),
        }
    }

    #[test]
    fn derive_deserialize_vec_with_init_size() {
        #[derive(XmlDeserialize, Default)]
        pub struct Child {
            #[xmlserde(name = b"age", ty = "attr")]
            pub _age: u16,
            #[xmlserde(ty = "text")]
            pub _name: String,
        }
        fn default_zero() -> u32 {
            0
        }
        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"root")]
        pub struct Aa {
            #[xmlserde(name = b"f", ty = "child", vec_size = 10)]
            pub f: Vec<Child>,
            #[xmlserde(name = b"cnt", ty = "attr", default = "default_zero")]
            pub _cnt: u32,
        }
        let xml = r#"<root cnt="2">
            <f age="15">Tom</f>
            <f age="9">Jerry</f>
        </root>"#;
        let result = xml_deserialize_from_str::<Aa>(xml).unwrap();
        assert_eq!(result.f.capacity(), 10);
    }

    #[test]
    fn serialize_attr_and_text() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
            #[xmlserde(name = b"male", ty = "attr")]
            male: bool,
            #[xmlserde(name = b"name", ty = "text")]
            name: String,
        }
        let result = xml_serialize(Person {
            age: 12,
            male: true,
            name: String::from("Tom"),
        });
        assert_eq!(result, "<Person age=\"12\" male=\"1\">Tom</Person>");
    }

    #[test]
    fn serialize_attr_and_sfc() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
            #[xmlserde(name = b"male", ty = "sfc")]
            male: bool,
            #[xmlserde(name = b"lefty", ty = "sfc")]
            lefty: bool,
        }
        let p1 = Person {
            age: 16,
            male: false,
            lefty: true,
        };
        let result = xml_serialize(p1);
        assert_eq!(result, "<Person age=\"16\"><lefty/></Person>");
    }

    #[test]
    fn serialize_children() {
        #[derive(XmlSerialize)]
        struct Skills {
            #[xmlserde(name = b"eng", ty = "attr")]
            english: bool,
            #[xmlserde(name = b"jap", ty = "sfc")]
            japanese: bool,
        }
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
            #[xmlserde(name = b"skills", ty = "child")]
            skills: Skills,
        }

        let p = Person {
            age: 32,
            skills: Skills {
                english: false,
                japanese: true,
            },
        };
        let result = xml_serialize(p);
        assert_eq!(
            result,
            "<Person age=\"32\"><skills eng=\"0\"><jap/></skills></Person>"
        );
    }

    #[test]
    fn serialize_opt_attr() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: Option<u16>,
        }
        let p = Person { age: Some(2) };
        let result = xml_serialize(p);
        assert_eq!(result, "<Person age=\"2\"/>");
        let p = Person { age: None };
        let result = xml_serialize(p);
        assert_eq!(result, "<Person/>");
    }

    #[test]
    fn deserialize_opt_attr() {
        #[derive(XmlDeserialize, Default)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: Option<u16>,
        }
        let xml = r#"<Person age="2"></Person>"#;
        let result = xml_deserialize_from_str::<Person>(xml);
        match result {
            | Ok(p) => assert_eq!(p.age, Some(2)),
            | Err(_) => panic!(),
        }
    }

    #[test]
    fn deserialize_default() {
        fn default_age() -> u16 {
            12
        }
        #[derive(XmlDeserialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr", default = "default_age")]
            age: u16,
            #[xmlserde(name = b"name", ty = "text")]
            name: String,
        }
        let xml = r#"<Person>Tom</Person>"#;
        let result = xml_deserialize_from_str::<Person>(xml);
        match result {
            | Ok(p) => {
                assert_eq!(p.age, 12);
                assert_eq!(p.name, "Tom");
            },
            | Err(_) => panic!(),
        }
    }

    #[test]
    fn serialize_skip_default() {
        fn default_age() -> u16 {
            12
        }
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr", default = "default_age")]
            age: u16,
            #[xmlserde(name = b"name", ty = "text")]
            name: String,
        }

        let p = Person {
            age: 12,
            name: String::from("Tom"),
        };
        let result = xml_serialize(p);
        assert_eq!(result, "<Person>Tom</Person>")
    }

    #[test]
    fn serialize_with_ns() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"Person")]
        #[xmlserde(with_ns = b"namespace")]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
            #[xmlserde(name = b"name", ty = "text")]
            name: String,
        }
        let p = Person {
            age: 12,
            name: String::from("Tom"),
        };
        let result = xml_serialize(p);
        assert_eq!(
            result,
            "<Person xmlns=\"namespace\" age=\"12\">Tom</Person>"
        );
    }

    #[test]
    fn scf_and_child_test() {
        #[derive(XmlDeserialize, XmlSerialize)]
        struct Child {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
        }

        #[derive(XmlDeserialize, XmlSerialize)]
        #[xmlserde(root = b"Person")]
        struct Person {
            #[xmlserde(name = b"lefty", ty = "sfc")]
            lefty: bool,
            #[xmlserde(name = b"c", ty = "child")]
            c: Child,
        }

        let xml = r#"<Person><lefty/><c age="12"/></Person>"#;
        let p = xml_deserialize_from_str::<Person>(xml).unwrap();
        let result = xml_serialize(p);
        assert_eq!(xml, result);
    }

    #[test]
    fn custom_ns_test() {
        #[derive(XmlDeserialize, XmlSerialize)]
        #[xmlserde(root = b"Child")]
        #[xmlserde(with_custom_ns(b"a", b"c"))]
        struct Child {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
        }
        let c = Child { age: 12 };
        let p = xml_serialize(c);
        assert_eq!(p, "<Child xmlns:a=\"c\" age=\"12\"/>");
    }

    #[test]
    fn enum_serialize_test() {
        #[derive(XmlDeserialize, XmlSerialize)]
        struct TestA {
            #[xmlserde(name = b"age", ty = "attr")]
            pub age: u16,
        }

        #[derive(XmlDeserialize, XmlSerialize)]
        struct TestB {
            #[xmlserde(name = b"name", ty = "attr")]
            pub name: String,
        }

        #[derive(XmlSerialize, XmlDeserialize)]
        enum TestEnum {
            #[xmlserde(name = b"testA")]
            TestA(TestA),
            #[xmlserde(name = b"testB")]
            TestB(TestB),
        }

        #[derive(XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"Child")]
        struct Child {
            #[xmlserde(name = b"dummy", ty = "child")]
            pub c: TestEnum,
        }

        let obj = Child {
            c: TestEnum::TestA(TestA { age: 23 }),
        };
        let xml = xml_serialize(obj);
        let p = xml_deserialize_from_str::<Child>(&xml).unwrap();
        match p.c {
            | TestEnum::TestA(a) => assert_eq!(a.age, 23),
            | TestEnum::TestB(_) => panic!(),
        }
    }

    #[test]
    fn unparsed_serde_test() {
        #[derive(XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"TestA")]
        pub struct TestA {
            #[xmlserde(name = b"others", ty = "child")]
            pub others: Unparsed,
        }

        let xml = r#"<TestA><others age="16" name="Tom"><gf/><parent><f/><m name="Lisa">1999</m></parent></others></TestA>"#;
        let p = xml_deserialize_from_str::<TestA>(xml).unwrap();
        let ser = xml_serialize(p);
        assert_eq!(xml, ser);
    }

    #[test]
    fn untag_serde_test() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"Root")]
        pub struct Root {
            #[xmlserde(ty = "untag")]
            pub dummy: EnumA,
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub enum EnumA {
            #[xmlserde(name = b"a")]
            A1(Astruct),
            #[xmlserde(name = b"b")]
            B1(Bstruct),
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Astruct {
            #[xmlserde(name = b"aAttr", ty = "attr")]
            pub a_attr1: u32,
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Bstruct {
            #[xmlserde(name = b"bAttr", ty = "attr")]
            pub b_attr1: u32,
        }

        let xml = r#"<Root><a aAttr="3"/></Root>"#;
        let p = xml_deserialize_from_str::<Root>(xml).unwrap();
        match p.dummy {
            | EnumA::A1(ref a) => assert_eq!(a.a_attr1, 3),
            | EnumA::B1(_) => panic!(),
        }
        let ser = xml_serialize(p);
        assert_eq!(xml, &ser);
    }

    #[test]
    fn vec_untag_serde_test() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"Root")]
        pub struct Root {
            #[xmlserde(ty = "untag")]
            pub dummy: Vec<EnumA>,
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub enum EnumA {
            #[xmlserde(name = b"a")]
            A1(Astruct),
            #[xmlserde(name = b"b")]
            B1(Bstruct),
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Astruct {
            #[xmlserde(name = b"aAttr", ty = "attr")]
            pub a_attr1: u32,
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Bstruct {
            #[xmlserde(name = b"bAttr", ty = "attr")]
            pub b_attr1: u32,
        }

        let xml = r#"<Root><a aAttr="3"/><b bAttr="5"/><a aAttr="4"/></Root>"#;
        let p = xml_deserialize_from_str::<Root>(xml).unwrap();
        assert_eq!(p.dummy.len(), 3);
        let ser = xml_serialize(p);
        assert_eq!(xml, &ser);
    }

    #[test]
    fn option_untag_serde_test() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"Root")]
        pub struct Root {
            #[xmlserde(ty = "untag")]
            pub dummy: Option<EnumA>,
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub enum EnumA {
            #[xmlserde(name = b"a")]
            A1(Astruct),
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Astruct {
            #[xmlserde(name = b"aAttr", ty = "attr")]
            pub a_attr1: u32,
        }
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct Bstruct {
            #[xmlserde(name = b"bAttr", ty = "attr")]
            pub b_attr1: u32,
        }

        let xml = r#"<Root/>"#;
        let p = xml_deserialize_from_str::<Root>(xml).unwrap();
        assert!(p.dummy.is_none());
        let xml = r#"<Root><a aAttr="3"/></Root>"#;
        let p = xml_deserialize_from_str::<Root>(xml).unwrap();
        match p.dummy {
            | Some(EnumA::A1(ref a)) => assert_eq!(a.a_attr1, 3),
            | None => panic!(),
        }
        let ser = xml_serialize(p);
        assert_eq!(xml, &ser);
    }

    #[test]
    fn ser_opt_text() {
        #[derive(Debug, XmlSerialize)]
        #[xmlserde(root = b"ttt")]
        pub struct AStruct {
            #[xmlserde(ty = "text")]
            pub text: Option<String>,
        }

        let instance = AStruct {
            text: Some(String::from("hello world!")),
        };
        let expect = xml_serialize(instance);
        assert_eq!(expect, "<ttt>hello world!</ttt>");
    }

    #[test]
    fn test_generics() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"Root")]
        pub struct Root<T: xmlserde::XmlSerialize + xmlserde::XmlDeserialize> {
            #[xmlserde(ty = "untag")]
            pub dummy: Option<T>,
        }

        #[derive(XmlSerialize)]
        pub enum EnumB<T: xmlserde::XmlSerialize> {
            #[xmlserde(name = b"a")]
            #[allow(dead_code)]
            A1(T),
        }

        #[derive(Debug, XmlSerialize)]
        #[xmlserde(root = b"ttt")]
        pub struct AStruct {
            #[xmlserde(ty = "text")]
            pub text: Option<String>,
        }
    }

    #[test]
    fn test_untag_enum_no_type_child_and_text() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        struct Type {
            #[xmlserde(name = b"name", ty = "attr")]
            name: String,
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"parameter")]
        struct Parameter {
            #[xmlserde(ty = "untag")]
            ty: ParameterType,
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        enum ParameterType {
            #[xmlserde(name = b"varargs")]
            VarArgs,
            #[xmlserde(name = b"type")]
            Type(Type),
            #[xmlserde(ty = "text")]
            Text(String),
        }

        let xml = r#"<parameter><varargs /></parameter>"#;
        let p = xml_deserialize_from_str::<Parameter>(xml).unwrap();
        assert!(matches!(p.ty, ParameterType::VarArgs));

        let expect = xml_serialize(p);
        assert_eq!(expect, "<parameter><varargs/></parameter>");

        let xml = r#"<parameter><type name="n"/></parameter>"#;
        let p = xml_deserialize_from_str::<Parameter>(xml).unwrap();
        if let ParameterType::Type(t) = &p.ty {
            assert_eq!(t.name, "n")
        } else {
            panic!("")
        }
        let expect = xml_serialize(p);
        assert_eq!(expect, xml);

        let xml = r#"<parameter>ttttt</parameter>"#;
        let p = xml_deserialize_from_str::<Parameter>(xml).unwrap();
        assert!(matches!(p.ty, ParameterType::Text(_)));
        let expect = xml_serialize(p);
        assert_eq!(expect, xml);
    }

    #[test]
    fn test_untag_enum_vec_and_text() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"text:p")]
        pub struct TextP {
            #[xmlserde(ty = "untag")]
            pub text_p_content: Vec<TextPContent>,
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub enum TextPContent {
            #[xmlserde(ty = "text")]
            Text(String),
            #[xmlserde(name = b"text:span", ty = "child")]
            TextSpan(TextSpan),
        }

        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        pub struct TextSpan {
            #[xmlserde(ty = "text", name = b"p")]
            pub t: String,
        }

        let xml = r#"<text:p>
            <text:span> text1 </text:span>
            <text:span>text2</text:span>
        </text:p>"#;
        let text_p = xml_deserialize_from_str::<TextP>(xml).unwrap();
        let content = &text_p.text_p_content;
        assert_eq!(content.len(), 2);
        if let TextPContent::TextSpan(span) = content.first().unwrap() {
            assert_eq!(&span.t, " text1 ")
        } else {
            panic!("")
        }
        if let TextPContent::TextSpan(span) = content.get(1).unwrap() {
            assert_eq!(&span.t, "text2")
        } else {
            panic!("")
        }

        let expect = xml_serialize(text_p);
        assert_eq!(
            expect,
            "<text:p><text:span> text1 </text:span><text:span>text2</text:span></text:p>"
        );

        let xml = r#"<text:p>abcdefg</text:p>"#;
        let text_p = xml_deserialize_from_str::<TextP>(xml).unwrap();
        let content = &text_p.text_p_content;
        assert_eq!(content.len(), 1);
        if let TextPContent::Text(s) = content.first().unwrap() {
            assert_eq!(s, "abcdefg")
        } else {
            panic!("")
        };
        let expect = xml_serialize(text_p);
        assert_eq!(expect, xml);
    }

    #[test]
    #[should_panic]
    fn test_unknown_fields_in_struct_deny_unknown_attr() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"pet")]
        #[xmlserde(deny_unknown_fields)]
        pub struct Pet {
            #[xmlserde(ty = "attr", name = b"name")]
            pub name: String,
        }
        let xml = r#"<pet name="Chaplin" age="1"/>"#;
        let _ = xml_deserialize_from_str::<Pet>(xml).unwrap();
    }

    #[test]
    fn test_unknown_fields_in_struct_accept_unknown_attr() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"pet")]
        pub struct Pet {
            #[xmlserde(ty = "attr", name = b"name")]
            pub name: String,
        }
        let xml = r#"<pet name="Chaplin" age="1"/>"#;
        let _ = xml_deserialize_from_str::<Pet>(xml).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_unknown_fields_in_struct_deny_unknown_field() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"pet")]
        #[xmlserde(deny_unknown_fields)]
        pub struct Pet {
            #[xmlserde(ty = "attr", name = b"name")]
            pub name: String,
        }
        let xml = r#"<pet name="Chaplin"><weight/></pet>"#;
        let _ = xml_deserialize_from_str::<Pet>(xml).unwrap();
    }

    #[test]
    fn test_unknown_fields_in_struct_accept_unknown_field() {
        #[derive(Debug, XmlSerialize, XmlDeserialize)]
        #[xmlserde(root = b"pet")]
        pub struct Pet {
            #[xmlserde(ty = "attr", name = b"name")]
            pub name: String,
        }
        let xml = r#"<pet name="Chaplin"><weight/></pet>"#;
        let _ = xml_deserialize_from_str::<Pet>(xml).unwrap();
    }

    // https://github.com/ImJeremyHe/xmlserde/issues/52
    #[test]
    fn test_issue_52() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"root")]
        struct Wrapper<T: xmlserde::XmlSerialize> {
            #[xmlserde(name = b"header", ty = "attr")]
            header: String,
            #[xmlserde(ty = "untag")]
            body: T,
        }

        #[derive(XmlSerialize)]
        struct Foo {
            #[xmlserde(name = b"Bar", ty = "child")]
            bar: Bar,
        }

        #[derive(XmlSerialize)]
        struct Bar {}

        let wrapper = Wrapper {
            header: "".to_string(),
            body: Foo { bar: Bar {} },
        };

        let r = xml_serialize(wrapper);
        assert_eq!(r, r#"<root header=""><Bar/></root>"#);
    }

    #[test]
    fn test_issue_52_enum_body() {
        #[derive(XmlSerialize)]
        #[xmlserde(root = b"root")]
        struct Wrapper<T: xmlserde::XmlSerialize> {
            #[xmlserde(name = b"header", ty = "attr")]
            header: String,
            #[xmlserde(ty = "untag")]
            body: T,
        }

        #[derive(XmlSerialize)]
        struct VariantAValue {
            #[xmlserde(ty = "text")]
            value: String,
        }

        #[derive(XmlSerialize)]
        struct VariantBValue {
            #[xmlserde(ty = "text")]
            value: i32,
        }

        #[derive(XmlSerialize)]
        enum BodyEnum {
            #[xmlserde(name = b"VariantA")]
            VariantA(VariantAValue),
            #[xmlserde(name = b"VariantB")]
            VariantB(VariantBValue),
        }

        let wrapper_a = Wrapper {
            header: "header_a".to_string(),
            body: BodyEnum::VariantA(VariantAValue {
                value: "value_a".to_string(),
            }),
        };
        let r_a = xml_serialize(wrapper_a);
        assert_eq!(
            r_a,
            r#"<root header="header_a"><VariantA>value_a</VariantA></root>"#
        );

        let wrapper_b = Wrapper {
            header: "header_b".to_string(),
            body: BodyEnum::VariantB(VariantBValue { value: 42 }),
        };
        let r_b = xml_serialize(wrapper_b);
        assert_eq!(
            r_b,
            r#"<root header="header_b"><VariantB>42</VariantB></root>"#
        );
    }

    #[test]
    fn test_de_untagged_struct() {
        #[derive(XmlDeserialize)]
        #[xmlserde(root = b"foo")]
        struct Foo {
            #[xmlserde(ty = "untagged_struct")]
            bar: Bar,
        }

        #[derive(XmlDeserialize)]
        struct Bar {
            #[xmlserde(name = b"a", ty = "child")]
            a: A,
            #[xmlserde(name = b"c", ty = "child")]
            c: C,
        }

        #[derive(XmlDeserialize)]
        struct A {
            #[xmlserde(name = b"attr1", ty = "attr")]
            attr1: u16,
        }

        #[derive(XmlDeserialize)]
        struct C {
            #[xmlserde(name = b"attr2", ty = "attr")]
            attr2: u16,
        }

        let xml = r#"<foo><a attr1="12"/><c attr2="200"/></foo>"#;
        let foo = xml_deserialize_from_str::<Foo>(xml).unwrap();
        assert_eq!(foo.bar.a.attr1, 12);
        assert_eq!(foo.bar.c.attr2, 200);

        #[derive(XmlDeserialize)]
        #[xmlserde(root = b"foo")]
        struct FooOption {
            #[xmlserde(ty = "untagged_struct")]
            bar: Option<Bar>,
        }
        let xml = r#"<foo><a attr1="12"/><c attr2="200"/></foo>"#;
        let foo = xml_deserialize_from_str::<FooOption>(xml).unwrap();
        let bar = foo.bar.unwrap();
        assert_eq!(bar.a.attr1, 12);
        assert_eq!(bar.c.attr2, 200);

        let xml = r#"<foo>></foo>"#;
        let foo = xml_deserialize_from_str::<FooOption>(xml).unwrap();
        assert!(foo.bar.is_none());
    }

    #[test]
    fn test_issue_60() {
        #[derive(Clone, Debug, Default, XmlDeserialize)]
        pub struct Parameters {
            #[xmlserde(name = b"parameter", ty = "child")]
            _parameter: Vec<A>,
            #[xmlserde(name = b"instance-parameter", ty = "child")]
            _instance_parameter: Option<A>,
        }

        #[derive(Clone, Debug, Default, XmlDeserialize)]
        pub struct A {}
    }

    #[test]
    fn test_vec_deserialize() {
        #[derive(Debug, XmlDeserialize)]
        pub struct CtTextParagraph {
            #[xmlserde(name = b"pPr", ty = "child")]
            pub _p_pr: Option<CtTextParagraphProperties>,
            #[xmlserde(ty = "untagged_enum")]
            pub _text_runs: Vec<A>,
        }

        #[derive(Debug, XmlDeserialize, XmlSerialize)]
        pub struct A {}
        #[derive(Debug, XmlDeserialize, XmlSerialize)]
        pub struct CtTextParagraphProperties {}
    }

    #[test]
    fn test_enum_map_attribute() {
        #[derive(Debug, Clone, PartialEq, Eq, XmlSerdeEnum, Default)]
        pub enum Status {
            #[xmlserde(rename = "cat")]
            Cat,
            #[xmlserde(rename = "dog")]
            Dog,
            #[xmlserde(map = ["parrot", "pigeon"])]
            Bird,
            #[xmlserde(other)]
            Other(String),
            #[default]
            Unknown,
        }

        // Test serialization
        assert_eq!(Status::Cat.serialize(), "cat");
        assert_eq!(Status::Dog.serialize(), "dog");
        assert_eq!(Status::Bird.serialize(), "Bird");
        assert_eq!(Status::Other("unknown".to_string()).serialize(), "unknown");

        // Test deserialization
        assert_eq!(Status::deserialize("cat").unwrap(), Status::Cat);
        assert_eq!(Status::deserialize("dog").unwrap(), Status::Dog);
        assert_eq!(Status::deserialize("parrot").unwrap(), Status::Bird);
        assert_eq!(Status::deserialize("pigeon").unwrap(), Status::Bird);
        assert_eq!(
            Status::deserialize("unknown").unwrap(),
            Status::Other("unknown".to_string())
        );
    }

    #[test]
    fn test_struct_map_attribute() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"status")]
        struct Status {
            #[xmlserde(map = [b"parrot", b"pigeon"], ty = "attr")]
            bird: String,
        }

        // Test deserialization with parrot attribute
        let xml = r#"<status parrot="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        // Test deserialization with pigeon attribute
        let xml = r#"<status pigeon="flying"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "flying");

        // Test serialization - should only use the canonical name (first mapped name)
        let status = Status {
            bird: "talking".to_string(),
        };
        let xml = xml_serialize(status);
        assert!(xml.contains("parrot=\"talking\""));
        assert!(!xml.contains("pigeon=\"talking\""));
    }

    #[test]
    fn test_struct_map_attribute_optional() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"status")]
        struct Status {
            #[xmlserde(map = [b"parrot", b"pigeon"], ty = "attr")]
            bird: Option<String>,
        }

        // Test deserialization with parrot attribute
        let xml = r#"<status parrot="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, Some("talking".to_string()));

        // Test deserialization with pigeon attribute
        let xml = r#"<status pigeon="flying"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, Some("flying".to_string()));

        // Test deserialization with no attributes
        let xml = r#"<status></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, None);

        // Test serialization - should only use the canonical name (first mapped name)
        let status = Status {
            bird: Some("talking".to_string()),
        };
        let xml = xml_serialize(status);
        assert!(xml.contains("parrot=\"talking\""));
        assert!(!xml.contains("pigeon=\"talking\""));

        // Test serialization with None
        let status = Status { bird: None };
        let xml = xml_serialize(status);
        assert!(!xml.contains("parrot"));
        assert!(!xml.contains("pigeon"));
    }

    #[test]
    fn test_struct_map_attribute_with_default() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"status")]
        struct Status {
            #[xmlserde(map = [b"parrot", b"pigeon"], ty = "attr", default = "default_bird")]
            bird: String,
        }

        fn default_bird() -> String {
            "unknown".to_string()
        }

        // Test deserialization with parrot attribute
        let xml = r#"<status parrot="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        // Test deserialization with pigeon attribute
        let xml = r#"<status pigeon="flying"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "flying");

        // Test deserialization with no attributes (should use default)
        let xml = r#"<status></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "unknown");

        // Test serialization - should only use the canonical name (first mapped name)
        let status = Status {
            bird: "talking".to_string(),
        };
        let xml = xml_serialize(status);
        assert!(xml.contains("parrot=\"talking\""));
        assert!(!xml.contains("pigeon=\"talking\""));

        // Test serialization with default value
        let status = Status {
            bird: "unknown".to_string(),
        };
        let xml = xml_serialize(status);
        assert!(!xml.contains("parrot"));
        assert!(!xml.contains("pigeon"));

        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"status")]
        struct Status2 {
            #[xmlserde(map = [b"pigeon", b"parrot"], ty = "attr", default = "default_bird")]
            bird: String,
        }
        let status2 = Status2 {
            bird: "cooing".to_string(),
        };
        let xml = xml_serialize(status2);
        assert!(!xml.contains("parrot"));
        assert!(xml.contains("pigeon=\"cooing\""));
    }

    #[test]
    fn test_struct_rename_all() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"status")]
        #[xmlserde(rename_all = "lowercase")]
        struct Status {
            #[xmlserde(name = b"pigeon", ty = "attr")]
            bird: String,
        }

        let status = Status {
            bird: "talking".to_string(),
        };
        let xml = xml_serialize(status);
        assert!(xml.contains("pigeon=\"talking\""));

        let xml = r#"<status pigeon="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        let xml = r#"<status Pigeon="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        let xml = r#"<status PIGEON="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        let xml = r#"<status PiGeOn="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");

        let xml = r#"<status pigeoN="talking"></status>"#;
        let status = xml_deserialize_from_str::<Status>(xml).unwrap();
        assert_eq!(status.bird, "talking");
    }

    #[test]
    fn test_struct_rename_all_cases() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "lowercase")]
        struct Person {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "attr")]
            last_name: String,
            #[xmlserde(ty = "attr")]
            is_active: bool,
        }

        // Test lowercase conversion
        let person = Person {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            is_active: true,
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("first_name=\"John\""));
        assert!(xml.contains("last_name=\"Doe\""));
        assert!(xml.contains("is_active=\"1\""));

        let xml = r#"<person first_name="John" last_name="Doe" is_active="1"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");
        assert!(person.is_active);

        // Test case-insensitive deserialization
        let xml = r#"<person First_Name="John" LAST_NAME="Doe" IS_ACTIVE="1"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");
        assert!(person.is_active);

        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "UPPERCASE")]
        struct PersonUpper {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "attr")]
            last_name: String,
            #[xmlserde(ty = "attr")]
            is_active: bool,
        }

        // Test uppercase conversion
        let person = PersonUpper {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            is_active: true,
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("FIRST_NAME=\"John\""));
        assert!(xml.contains("LAST_NAME=\"Doe\""));
        assert!(xml.contains("IS_ACTIVE=\"1\""));

        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "camelCase")]
        struct PersonCamel {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "attr")]
            last_name: String,
            #[xmlserde(ty = "attr")]
            is_active: bool,
        }

        // Test camelCase conversion
        let person = PersonCamel {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            is_active: true,
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("firstName=\"John\""));
        assert!(xml.contains("lastName=\"Doe\""));
        assert!(xml.contains("isActive=\"1\""));

        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "PascalCase")]
        struct PersonPascal {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "attr")]
            last_name: String,
            #[xmlserde(ty = "attr")]
            is_active: bool,
        }

        // Test PascalCase conversion
        let person = PersonPascal {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            is_active: true,
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("FirstName=\"John\""));
        assert!(xml.contains("LastName=\"Doe\""));
        assert!(xml.contains("IsActive=\"1\""));
    }

    #[test]
    fn test_rename_all_with_mapped_names() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "snake_case")]
        struct Person {
            #[xmlserde(map = [b"first_name", b"firstName", b"FirstName"], ty = "attr")]
            first_name: String,
            #[xmlserde(map = [b"last_name", b"lastName", b"LastName"], ty = "attr")]
            last_name: String,
        }

        // Test serialization - should use the canonical name (first mapped name)
        let person = Person {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("first_name=\"John\""));
        assert!(xml.contains("last_name=\"Doe\""));

        // Test deserialization with different mapped names
        let xml = r#"<person first_name="John" last_name="Doe"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");

        let xml = r#"<person firstName="John" lastName="Doe"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");

        let xml = r#"<person FirstName="John" LastName="Doe"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");
    }

    #[test]
    fn test_nested_struct_rename_all() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person", rename_all = "snake_case")]
        struct Person {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "child")]
            address: Address,
        }

        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(rename_all = "camelCase")]
        struct Address {
            #[xmlserde(ty = "attr")]
            street_name: String,
            #[xmlserde(ty = "attr")]
            house_number: u32,
        }

        let person = Person {
            first_name: "John".to_string(),
            address: Address {
                street_name: "Main Street".to_string(),
                house_number: 123,
            },
        };

        let xml = xml_serialize(person);
        assert!(xml.contains("first_name=\"John\""));
        assert!(xml.contains("streetName=\"Main Street\""));
        assert!(xml.contains("houseNumber=\"123\""));

        let xml = r#"<person first_name="John"><address streetName="Main Street" houseNumber="123"/></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.address.street_name, "Main Street");
        assert_eq!(person.address.house_number, 123);

        // Test deserialization with case-sensitive root name
        let xml = r#"<Person first_name="John"><address streetName="Main Street2" houseNumber="123"/></Person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.address.street_name, "Main Street2");
        assert_eq!(person.address.house_number, 123);

        // Test case-insensitive deserialization
        let xml = r#"<person First_Name="John"><address StreetName="Main Street" HouseNumber="123"/></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.address.street_name, "Main Street");
        assert_eq!(person.address.house_number, 123);
    }

    #[test]
    fn test_rename_all_root_case_insensitive() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = b"person")]
        #[xmlserde(rename_all = "camelCase")]
        struct Person {
            #[xmlserde(ty = "attr")]
            first_name: String,
            #[xmlserde(ty = "attr")]
            last_name: String,
        }

        // Test deserialization with different case variations of the root element
        let xml = r#"<Person firstName="John" lastName="Doe"></Person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");

        let xml = r#"<PERSON firstName="John" lastName="Doe"></PERSON>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");

        let xml = r#"<person firstName="John" lastName="Doe"></person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.first_name, "John");
        assert_eq!(person.last_name, "Doe");

        // Test serialization - should use the canonical name (first mapped name)
        let person = Person {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
        };
        let xml = xml_serialize(person);
        assert!(xml.contains("person"));
        assert!(xml.contains("firstName=\"John\""));
        assert!(xml.contains("lastName=\"Doe\""));
    }

    #[test]
    fn test_multiple_roots() {
        #[derive(XmlDeserialize, XmlSerialize, Debug, PartialEq)]
        #[xmlserde(root = [b"person", b"employee"])]
        struct Person {
            #[xmlserde(name = b"age", ty = "attr")]
            age: u16,
            #[xmlserde(name = b"name", ty = "text")]
            name: String,
        }

        // Test deserialization with first root
        let xml = r#"<person age="25">John</person>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.age, 25);
        assert_eq!(person.name, "John");

        // Test deserialization with second root
        let xml = r#"<employee age="30">Jane</employee>"#;
        let person = xml_deserialize_from_str::<Person>(xml).unwrap();
        assert_eq!(person.age, 30);
        assert_eq!(person.name, "Jane");

        // Test serialization (should use first root)
        let person = Person {
            age: 25,
            name: "John".to_string(),
        };
        let xml = xml_serialize(person);
        assert_eq!(xml, "<person age=\"25\">John</person>");
    }

    #[derive(XmlDeserialize, Debug, Clone, XmlSerialize)]
    #[xmlserde(root = b"BirdObservation")]
    pub struct BirdObservation {
        #[xmlserde(name = b"Species", ty = "attr")]
        pub species: String,
        #[xmlserde(name = b"Mood", ty = "attr")]
        pub mood: String,
        #[xmlserde(name = b"Notes", ty = "attr")]
        pub notes: String,
        #[xmlserde(name = b"ObservationTime", ty = "attr")]
        pub observation_time: String,
        #[xmlserde(name = b"Count", ty = "attr")]
        pub count: i32,
        #[xmlserde(name = b"NestDetails", ty = "child")]
        pub nest_details: Option<NestDetails>,
    }

    #[derive(XmlDeserialize, Debug, Clone, XmlSerialize)]
    pub enum NestDetails {
        #[xmlserde(name = b"TreeNest")]
        TreeNest(TreeNest),
    }

    #[derive(XmlDeserialize, Debug, Clone, XmlSerialize)]
    pub struct TreeNest {
        #[xmlserde(name = b"Species", ty = "attr")]
        pub species: String,
        #[xmlserde(name = b"Location", ty = "child")]
        pub location: Location,
        #[xmlserde(name = b"Observer", ty = "child")]
        pub observer: Observer,
    }

    #[derive(XmlDeserialize, Debug, Clone, XmlSerialize)]
    pub struct Location {
        #[xmlserde(name = b"id", ty = "attr")]
        pub id: i32,
    }

    #[derive(XmlDeserialize, Debug, Clone, XmlSerialize)]
    pub struct Observer {
        #[xmlserde(name = b"id", ty = "attr")]
        pub id: i32,
    }

    #[test]
    fn test_bird_observation() {
        let xml = r#"<BirdObservation
            Species="Robin"
            Mood="Chirpy"
            Notes="Singing a lovely song."
            ObservationTime="2024-07-27T10:30:00"
            Count="2"
        >
            <NestDetails>
                <TreeNest Species="Robin">
                    <Location id="12345" />
                    <Observer id="98765" />
                </TreeNest>
            </NestDetails>
        </BirdObservation>"#;

        let result = xml_deserialize_from_str::<BirdObservation>(xml);
        match result {
            | Ok(observation) => {
                println!("Deserialized observation: {:#?}", observation);
                assert_eq!(observation.species, "Robin");
                assert_eq!(observation.mood, "Chirpy");
                assert_eq!(observation.notes, "Singing a lovely song.");
                assert_eq!(observation.observation_time, "2024-07-27T10:30:00");
                assert_eq!(observation.count, 2);
                // Debug output for nest_details
                println!("Nest details: {:?}", observation.nest_details);
                // Verify NestDetails
                let nest_details = observation.nest_details.unwrap();
                match nest_details {
                    | NestDetails::TreeNest(nest) => {
                        assert_eq!(nest.species, "Robin");
                        assert_eq!(nest.location.id, 12345);
                        assert_eq!(nest.observer.id, 98765);
                    },
                }
            },
            | Err(e) => {
                println!("Deserialization error: {}", e);
                panic!("Deserialization failed");
            },
        }
    }
}
