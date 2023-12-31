#![deny(clippy::unwrap_used)]

mod string;
use nom::{combinator::fail, error::ErrorKind, multi::separated_list1};
use std::{borrow::Cow, collections::HashMap};
use wasm_bindgen::prelude::*;

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while},
    character::complete::{char, digit1, one_of},
    combinator::{cut, map, opt, value},
    error::{context, ContextError, FromExternalError, ParseError},
    multi::separated_list0,
    number::complete::double,
    sequence::{delimited, preceded, separated_pair, terminated},
    AsChar, IResult, InputTakeAtPosition, Parser,
};

///
/// [`DataModel`] is used to perform ron object conversion it is the intermediate representation
/// for the parser.
///
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum DataModel<'a> {
    Null,                                 // ✅
    Boolean(bool),                        // ✅
    Float(f64),                           // ✅
    String(Cow<'a, str>),                 // ✅
    Map(HashMap<&'a str, DataModel<'a>>), // ✅
    Vec(Vec<DataModel<'a>>),              // ✅
}

impl<'a, T: 'a + Into<Cow<'a, str>>> From<T> for DataModel<'a> {
    fn from(value: T) -> Self {
        DataModel::String(value.into())
    }
}

impl<'a> std::hash::Hash for DataModel<'a>
where
    HashMap<&'a str, DataModel<'a>>: std::hash::Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            DataModel::Null => 0_u8.hash(state),
            DataModel::Boolean(data) => data.hash(state),
            DataModel::Float(_data) => {}
            DataModel::String(data) => data.hash(state),
            DataModel::Map(data) => data.hash(state),
            DataModel::Vec(data) => data.hash(state),
        }
    }
}

fn spacer<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";

    take_while(move |c| chars.contains(c))(i)
}

pub fn char_checker<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    <&'a str as nom::InputTakeAtPosition>::Item: nom::AsChar,
{
    input.split_at_position1_complete(
        |item| !(item.is_alphanum() || item == '_'),
        nom::error::ErrorKind::AlphaNumeric,
    )
}

pub fn num_checker<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    <&'a str as nom::InputTakeAtPosition>::Item: nom::AsChar,
{
    input.split_at_position1_complete(
        |item| !(item.is_ascii_digit() || item == '.'),
        nom::error::ErrorKind::AlphaNumeric,
    )
}

fn parse_str<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    escaped(char_checker, '\\', one_of("\"n\\"))(i)
}

fn parse_bool<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, bool, E> {
    let parse_true = value(true, tag("true"));
    let parse_false = value(false, tag("false"));

    alt((parse_true, parse_false)).parse(i)
}

fn parse_null<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, (), E> {
    value((), tag("None")).parse(input)
}

fn parse_string<'a, E: ParseError<&'a str> + ContextError<&'a str> + std::fmt::Debug>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    context(
        "string",
        preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
    )(input)
}

#[allow(dead_code)]
fn parse_integer<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, isize, E> {
    let (number, data) = opt(char('-'))(input)?;
    digit1(number).and_then(|(rest, doq)| match (doq.parse::<isize>(), data.is_some()) {
        (Ok(x), _) => Ok((rest, x)),
        (Result::Err(_), true) => Err(nom::Err::Failure(E::from_error_kind(
            input,
            nom::error::ErrorKind::Fail,
        ))),
        (Result::Err(_), false) => Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Fail,
        ))),
    })
}

fn parse_datetime<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    i: &'a str,
) -> IResult<&'a str, String, E> {
    context(
        "datetime",
        map(
            separated_pair(
                separated_list1(tag("-"), num_checker),
                tag(" "),
                separated_list1(tag(":"), num_checker),
            ),
            |x| {
                println!("datetime: {:#?}", x);
                let mut string = String::new();
                string.push_str(&x.0.join("-"));
                string.push(' ');
                string.push_str(&x.1.join(":"));
                string
            },
        ),
    )
    .parse(i)
}

fn parse_float<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, f64, E> {
    let data = double(input);
    // let data = map_opt(num_checker, |value| { // This is a optional rudimentary float parser
    //     eprintln!("parsing: {}", value);
    //     value.parse::<f64>().ok()
    // })
    // .parse(input);

    match data {
        Ok((rest, _)) if rest.starts_with('*') => fail(input),
        _ => data,
    }
}

fn parse_array<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, Vec<DataModel<'a>>, E> {
    context(
        "array",
        preceded(
            char('['),
            cut(terminated(
                separated_list0(preceded(spacer, char(',')), data_model),
                preceded(spacer, char(']')),
            )),
        ),
    )
    .parse(input)
}

fn parse_array_tuple<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, Vec<DataModel<'a>>, E> {
    context(
        "tuple",
        preceded(
            char('('),
            cut(terminated(
                separated_list0(preceded(spacer, char(',')), data_model),
                preceded(spacer, char(')')),
            )),
        ),
    )
    .parse(input)
}

fn parse_key_value_hash<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    i: &'a str,
) -> IResult<&'a str, (&'a str, DataModel<'a>), E> {
    separated_pair(
        preceded(spacer, parse_string),
        cut(preceded(spacer, char(':'))),
        preceded(spacer, data_model),
    )
    .parse(i)
}

fn parse_key_value_struct<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    i: &'a str,
) -> IResult<&'a str, (&'a str, DataModel<'a>), E> {
    separated_pair(
        preceded(spacer, parse_str.or(parse_string)),
        cut(preceded(spacer, char(':'))),
        preceded(spacer, data_model),
    )
    .parse(i)
}

fn parse_hash<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, HashMap<&'a str, DataModel<'a>>, E> {
    context(
        "map",
        preceded(
            char('{'),
            cut(terminated(
                map(
                    separated_list0(preceded(spacer, char(',')), parse_key_value_hash),
                    |tuple_vec| tuple_vec.into_iter().collect(),
                ),
                preceded(spacer, char('}')),
            )),
        ),
    )(input)
}

fn parse_hash_unticked<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, HashMap<&'a str, DataModel<'a>>, E> {
    context(
        "struct map",
        preceded(
            spacer,
            preceded(
                char('{'),
                cut(terminated(
                    map(
                        separated_list0(preceded(spacer, char(',')), parse_key_value_struct),
                        |tuple_vec| tuple_vec.into_iter().collect(),
                    ),
                    preceded(spacer, char('}')),
                )),
            ),
        ),
    )(input)
}

fn parse_struct<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, HashMap<&'a str, DataModel<'a>>, E> {
    let value = context(
        "struct",
        separated_pair(parse_str, spacer, parse_hash_unticked),
    )(input);

    let value = value?;

    Ok((value.0, value.1 .1))
}


fn parse_named_array<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, Vec<DataModel<'a>>, E> {
    let value = context(
        "struct",
        separated_pair(parse_str, spacer, parse_array),
    )(input);

    let value = value?;

    Ok((value.0, value.1 .1))
}

fn parse_tuple_var<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    input: &'a str,
) -> IResult<&'a str, DataModel<'a>, E> {
    context(
        "option",
        preceded(
            preceded(parse_str, char('(')),
            cut(terminated(data_model, char(')'))),
        ),
    )(input)
}

pub fn char_checker_wc<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E>
where
    <&'a str as nom::InputTakeAtPosition>::Item: nom::AsChar,
{
    input.split_at_position1_complete(
        |item| item == ',' || item == '}' || item == ')' || item == ']',
        nom::error::ErrorKind::AlphaNumeric,
    )
}

pub fn everything_none_space<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E>
where
    <&'a str as nom::InputTakeAtPosition>::Item: nom::AsChar,
{
    input.split_at_position1_complete(|item| item == ' ', nom::error::ErrorKind::AlphaNumeric)
}

fn parse_wildcard<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    // escaped(char_checker_wc, '\\', one_of("\"n\\"))(i)
    alt((
        map(masked_data, |_| "*** masked ***"),
        escaped(char_checker_wc, '\\', one_of("\"n\\")),
    ))(i)
}

fn masked_data<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    delimited(tag("*** "), everything_none_space, tag(" ***"))(i)
}

///
/// Parse string into [`DataModel`] using this function.
///
pub fn data_model<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    i: &'a str,
) -> IResult<&'a str, DataModel<'a>, E> {
    dbg!(i);
    println!("");
    preceded(
        spacer,
        alt((
            map(parse_null, |_| DataModel::Null),
            map(parse_bool, DataModel::Boolean),
            map(parse_datetime, Into::into),
            map(parse_float, DataModel::Float),
            map(string::parse_string, Into::into),
            map(parse_array_tuple, DataModel::Vec),
            map(parse_array, DataModel::Vec),
            map(parse_hash, DataModel::Map),
            map(parse_tuple_var, |x| x),
            map(parse_struct, DataModel::Map),
            map(parse_named_array, DataModel::Vec),
            map(parse_wildcard, Into::into),
        )),
    )
    .parse(i)
}

///
/// Function exposed as `wasm` function in js `parse`. Allowing use to extend the functionality and
/// usage for web
///
#[wasm_bindgen(js_name=parse)]
pub fn my_parse(val: String) -> String {
    serde_json::to_string(
        &root::<(&str, ErrorKind)>(&val)
            .expect("Failed to parse the ron object")
            .1,
    )
    .expect("Failed to serialize to json")
}

///
/// The entrypoint to the crate this is internally calling [`data_model`] with a relaxed
/// constraints of space padding on the start and the end
///
pub fn root<
    'a,
    E: ParseError<&'a str>
        + ContextError<&'a str>
        + FromExternalError<&'a str, std::num::ParseIntError>
        + std::fmt::Debug,
>(
    i: &'a str,
) -> IResult<&'a str, DataModel<'a>, E> {
    delimited(spacer, data_model, opt(spacer)).parse(i)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(dead_code)]

    use nom::error::ErrorKind;

    use crate::*;

    #[derive(Debug)]
    struct Everything {
        integer: i32,
        uint: u32,
        float: f64,
        string: String,
        vector_int: Vec<i32>,
        vector_str: Vec<String>,
        hashmap: HashMap<String, i32>,
        nested: Bob,
        custom_hidden: Hidden,
        enumer1: Boat,
        enumer2: Boat,
        enumer3: Option<Boat>,
        enumer4: Boat,
        tutu: (i32, f64),
        nothing: Option<()>,
        boolean: bool,
    }

    #[derive(Debug)]
    enum Boat {
        JustOne(i32),
        AnCouple((i32, String)),
        JustStruct { names: Vec<String>, age: i32 },
        Unit,
    }

    #[derive(Debug)]
    struct Bob {
        inner_int: f32,
        inner_string: String,
    }

    struct Hidden;

    impl std::fmt::Debug for Hidden {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("*** hidden ***")
        }
    }

    fn generate_data() -> Everything {
        Everything {
            uint: 321,
            integer: -123,
            float: 123.456,
            string: "Bob said, \"Hello!\"".to_owned(),
            vector_int: vec![12, 45, 56, -1, -3],
            vector_str: vec![
                "Alice".to_string(),
                "Venus".to_string(),
                "Karter".to_string(),
            ],
            hashmap: [
                ("Draco".to_string(), 123),
                ("Harry".to_string(), -123),
                ("Ron".to_string(), 0),
            ]
            .into_iter()
            .collect(),
            nested: Bob {
                inner_int: -50.0,
                inner_string: "Sharel".to_string(),
            },
            custom_hidden: Hidden,
            enumer1: Boat::JustOne(1024),
            enumer2: Boat::AnCouple((512, "Freak".to_string())),
            enumer3: Some(Boat::JustStruct {
                names: vec!["Tricky".to_string(), "Hacky".to_string()],
                age: -256,
            }),
            enumer4: Boat::Unit,
            tutu: (12, -12.5),
            nothing: None,
            boolean: false,
        }
    }

    #[test]
    #[ignore]
    fn test_generate() {
        panic!("{:?}", generate_data())
    }

    #[test]
    #[ignore]
    fn debug_test() {
        let data = "!13113431";
        let (x, y) = parse_integer::<()>(data).unwrap();
        panic!("{:#?}", (x, y))
    }

    #[test]
    fn test_null() {
        let data = "None";
        let _value = parse_null::<(&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_not_null() {
        let data = "123";
        let _value = parse_null::<(&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    fn test_boolean() {
        let data = "true";
        let value = parse_bool::<(&str, ErrorKind)>(data).unwrap();
        assert!(value.1, "residue: {}", value.0)
    }

    #[test]
    #[should_panic]
    fn test_not_bool() {
        let data = "123";
        let _value = parse_bool::<(&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    fn test_string() {
        let data = r#""true""#;
        let value = string::parse_string::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(value.1, "true", "residue: {}", value.0)
    }

    #[test]
    #[should_panic]
    fn test_not_string() {
        let data = "true";
        let _value = string::parse_string::<(&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    fn test_float() {
        let data = r#"123.35"#;
        let value = double::<_, (&str, ErrorKind)>(data).unwrap();
        assert_eq!(value.1, 123.35, "residue: {}", value.0)
    }

    #[test]
    #[should_panic]
    fn test_not_float() {
        let data = r#""213""#;
        let _value = double::<_, (&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    fn test_integer() {
        let data = r#"123"#;
        let value = parse_integer::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(value.1, 123, "residue: {}", value.0)
    }

    #[test]
    #[should_panic]
    fn test_not_integer() {
        let data = r#""213""#;
        let _value = parse_integer::<(&str, ErrorKind)>(data).unwrap();
    }

    #[test]
    fn test_array() {
        let data = "[ \"12\", 2.3]";

        let value = parse_array::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            vec![DataModel::String("12".into()), DataModel::Float(2.3)],
            "residue: {}",
            value.0
        )
    }

    #[test]
    #[should_panic]
    fn test_not_array() {
        let data = "[ \"12\"; 23]";
        let value = parse_array::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            vec![DataModel::String("12".into()), DataModel::Float(23.0)],
            "residue: {}",
            value.0
        )
    }

    #[test]
    fn test_array_tuple() {
        let data = "(\"12\",23)";
        let value = parse_array_tuple::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            vec![DataModel::String("12".into()), DataModel::Float(23.0)],
            "residue: {}",
            value.0
        )
    }

    #[test]
    #[should_panic]
    fn test_not_array_tuple() {
        let data = "( \"12\"; 23)";
        let value = parse_array_tuple::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            vec![DataModel::String("12".into()), DataModel::Float(23.0)],
            "residue: {}",
            value.0
        )
    }

    #[test]
    fn test_hash() {
        let data = r#"{ "inner": "data", "outer": 123 }"#;
        let value = parse_hash::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            [
                ("inner", DataModel::String("data".into())),
                ("outer", DataModel::Float(123.0))
            ]
            .into_iter()
            .collect(),
            "residue: {}",
            value.0
        )
    }

    #[test]
    #[should_panic]
    fn test_not_hash() {
        let data = r#"{ inner: "data", outer: 123, value: {} }"#;
        let value = parse_hash::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            [
                ("inner", DataModel::String("data".into())),
                ("outer", DataModel::Float(123.0))
            ]
            .into_iter()
            .collect(),
            "residue: {}",
            value.0
        )
    }

    #[test]
    fn test_struct() {
        let data = r#"Yager { inner: "data", outer: 123 }"#;
        let value = parse_struct::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            [
                ("inner", DataModel::String("data".into())),
                ("outer", DataModel::Float(123.0))
            ]
            .into_iter()
            .collect(),
            "residue: {}",
            value.0
        )
    }

    #[test]
    #[should_panic]
    fn test_not_struct() {
        let data = r#"Insider( inner: "data", outer: 123, value: {} )"#;
        let value = parse_struct::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            [
                ("inner", DataModel::String("data".into())),
                ("outer", DataModel::Float(123.0))
            ]
            .into_iter()
            .collect(),
            "residue: {}",
            value.0
        )
    }

    #[test]
    fn test_array_tuple_var() {
        let data = "Data((\"12\",23))";
        let value = parse_tuple_var::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            DataModel::Vec(vec![DataModel::String("12".into()), DataModel::Float(23.0)]),
            "residue: {}",
            value.0
        )
    }

    #[test]
    #[should_panic]
    fn test_not_array_tuple_var() {
        let data = "Data( \"12\", 23)";
        let value = parse_tuple_var::<(&str, ErrorKind)>(data).unwrap();
        assert_eq!(
            value.1,
            DataModel::Vec(vec![DataModel::String("12".into()), DataModel::Float(23.0)]),
            "residue: {}",
            value.0
        )
    }

    #[test]
    fn test_bob() {
        let bob = Bob {
            inner_int: 123.0,
            inner_string: "data".to_string(),
        };

        let val = format!("{:?}", bob);

        let a_val1 = "{\"inner_string\":\"data\",\"inner_int\":123.0}";
        let a_val2 = "{\"inner_int\":123.0,\"inner_string\":\"data\"}";
        let value = serde_json::to_string(&root::<(&str, ErrorKind)>(&val).unwrap().1).unwrap();

        assert!(value == a_val1 || value == a_val2);
    }

    #[test]
    #[ignore = "It's panicable"]
    fn test_try_all() {
        let data = generate_data();
        let data = format!("{:?}", data);

        let data_model = root::<(&str, ErrorKind)>(&data).unwrap().1;

        panic!("{:?}", data_model);
    }

    #[derive(Debug)]
    struct A {
        data: String,
        value: Ba,
    }
    #[derive(Debug)]
    struct Ba {
        item: i32,
    }

    #[test]
    fn test_xyz() {
        let data = A {
            data: "123".to_string(),
            value: Ba { item: 123 },
        };
        let data = format!("{:?}", data);
        let data_model = root::<(&str, ErrorKind)>(&data).unwrap().1;
        let value = serde_json::to_string(&data_model).unwrap();

        let a_val2 = "{\"value\":{\"item\":123.0},\"data\":\"123\"}";
        let a_val1 = "{\"data\":\"123\",\"value\":{\"item\":123.0}}";
        assert!(value == a_val1 || value == a_val2)
    }

    #[test]
    fn test_me_10000() {
        let data1 = r#"Dalton { name: ""#;
        let data2 = r#"" }"#;
        let heavy_data = String::from("A").repeat(1000);
        let composite_data = {
            let mut output = String::new();
            output.push_str(data1);
            output.push_str(&heavy_data);
            output.push_str(data2);
            output
        };
        let parsed = root::<(&str, ErrorKind)>(&composite_data).unwrap().1;
        let expected = DataModel::Map([("name", DataModel::String(heavy_data.into()))].into());
        println!("{:#?}", parsed);
        assert_eq!(parsed, expected)
    }

    #[test]
    #[ignore = "It's panicable"]
    fn test_payment_request() {
        let data = r#"PaymentsRequest { payment_id: None, merchant_id: None, amount: Some(Value(6500)), routing: None, connector: None, currency: Some(USD), capture_method: Some(Automatic), amount_to_capture: None, capture_on: None, confirm: Some(false), customer: None, customer_id: Some("hyperswitch111"), email: Some(Email(*********@gmail.com)), name: None, phone: None, phone_country_code: None, off_session: None, description: Some("Hello this is description"), return_url: None, setup_future_usage: None, authentication_type: Some(ThreeDs), payment_method_data: None, payment_method: None, payment_token: None, card_cvc: None, shipping: Some(Address { address: Some(AddressDetails { city: Some("Banglore"), country: Some(US), line1: Some(*** alloc::string::String ***), line2: Some(*** alloc::string::String ***), line3: Some(*** alloc::string::String ***), zip: Some(*** alloc::string::String ***), state: Some(*** alloc::string::String ***), first_name: Some(*** alloc::string::String ***), last_name: None }), phone: Some(PhoneDetails { number: Some(*** alloc::string::String ***), country_code: Some("+1") }) }), billing: Some(Address { address: Some(AddressDetails { city: Some("San Fransico"), country: Some(AT), line1: Some(*** alloc::string::String ***), line2: Some(*** alloc::string::String ***), line3: Some(*** alloc::string::String ***), zip: Some(*** alloc::string::String ***), state: Some(*** alloc::string::String ***), first_name: Some(*** alloc::string::String ***), last_name: Some(*** alloc::string::String ***) }), phone: Some(PhoneDetails { number: Some(*** alloc::string::String ***), country_code: Some("+91") }) }), statement_descriptor_name: None, statement_descriptor_suffix: None, metadata: Some(Metadata { order_details: Some(OrderDetails { product_name: "gillete razor", quantity: 1 }), order_category: None, redirect_response: None, allowed_payment_method_types: None }), order_details: None, client_secret: None, mandate_data: None, mandate_id: None, browser_info: None, payment_experience: None, payment_method_type: None, business_country: Some(US), business_label: Some("default"), merchant_connector_details: None, allowed_payment_method_types: None, business_sub_label: None, manual_retry: false, udf: None }"#;

        let data_model = root::<(&str, ErrorKind)>(data).unwrap().1;

        panic!("{:?}", data_model);
    }

    #[test]
    fn test_parse_datetime() {
        let datetime = "2023-06-06 12:30:30.351996";
        let parse = parse_datetime::<(&str, ErrorKind)>(datetime).unwrap();
        assert_eq!(parse.1, "2023-06-06 12:30:30.351996")
    }

    #[test]
    fn test_parse_date_response() {
        let data = "PaymentsResponse { created: Some(2023-06-06 12:30:30.351996)}";
        let parse = root::<(&str, ErrorKind)>(data).unwrap().1;
        assert_eq!(
            parse,
            DataModel::Map(
                [(
                    "created",
                    DataModel::String("2023-06-06 12:30:30.351996".into())
                )]
                .into()
            )
        )
    }

    #[test]
    #[ignore = "It's panicable"]
    fn regression_test_1() {
        let data = r#"PaymentsRequest { payment_id: Some(PaymentIntentId("pay_nLjAOteAucUEv29qLv01")), merchant_id: None, amount: None, routing: None, connector: None, currency: None, capture_method: None, amount_to_capture: None, capture_on: None, confirm: Some(true), customer: None, customer_id: None, email: None, name: None, phone: None, phone_country_code: None, off_session: None, description: None, return_url: Some(Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("app.hyperswitch.io")), port: None, path: "/home", query: None, fragment: None }), setup_future_usage: None, authentication_type: None, payment_method_data: Some(Card(Card { card_number: CardNumber(424242**********), card_exp_month: *** alloc::string::String ***, card_exp_year: *** alloc::string::String ***, card_holder_name: *** alloc::string::String ***, card_cvc: *** alloc::string::String ***, card_issuer: Some(""), card_network: Some(Visa) })), payment_method: Some(Card), payment_token: None, card_cvc: None, shipping: None, billing: None, statement_descriptor_name: None, statement_descriptor_suffix: None, metadata: None, order_details: None, client_secret: Some("pay_nLjAOteAucUEv29qLv01_secret_9M2BQVnMPskkdYGitWNJ"), mandate_data: None, mandate_id: None, browser_info: Some(Object {"color_depth": Number(30), "java_enabled": Bool(true), "java_script_enabled": Bool(true), "language": String("en-GB"), "screen_height": Number(1117), "screen_width": Number(1728), "time_zone": Number(-330), "ip_address": String("65.1.52.128"), "accept_header": String("text\\/html,application\\/xhtml+xml,application\\/xml;q=0.9,image\\/webp,image\\/apng,*\\/*;q=0.8"), "user_agent": String("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36")}), payment_experience: None, payment_method_type: None, business_country: None, business_label: None, merchant_connector_details: None, allowed_payment_method_types: None, business_sub_label: None, manual_retry: false, udf: None }"#;

        let parse = root::<(&str, ErrorKind)>(data).unwrap().1;
        panic!("{:#?}", parse);
    }

    #[test]
    fn test_empty_brackets() {
        let data = "PaymentsRequest { payment_methods: [] }";
        let parse = root::<(&str, ErrorKind)>(data).unwrap().1;
        assert_eq!(
            parse,
            DataModel::Map([("payment_methods", DataModel::Vec(vec![]))].into())
        )
    }

    #[test]
    fn test_edge_case() {
        let data = r#"PaymentsRequest { payment_id: Some(PaymentIntentId("pay_tf5WjPnA2ErXv1foocwA")), merchant_id: None, amount: None, routing: None, connector: Some([]), currency: None, capture_method: None, amount_to_capture: None, capture_on: None, confirm: Some(true), customer: None, customer_id: None, email: None, name: None, phone: None, phone_country_code: None, off_session: None, description: None, return_url: Some(Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("app.hyperswitch.io")), port: None, path: "/home", query: None, fragment: None }), setup_future_usage: None, authentication_type: None, payment_method_data: Some(BankTransfer(AchBankTransfer { billing_details: AchBillingDetails { email: Email(**************@gmail.com) } })), payment_method: Some(BankTransfer), payment_token: None, card_cvc: None, shipping: None, billing: None, statement_descriptor_name: None, statement_descriptor_suffix: None, order_details: None, client_secret: Some("pay_tf5WjPnA2ErXv1foocwA_secret_nmxdfPGZRIXvv7UKngMu"), mandate_data: None, mandate_id: None, browser_info: Some(Object {"color_depth": Number(30), "java_enabled": Bool(true), "java_script_enabled": Bool(true), "language": String("en-GB"), "screen_height": Number(900), "screen_width": Number(1440), "time_zone": Number(-330), "ip_address": String("103.159.11.202"), "accept_header": String("text\\/html,application\\/xhtml+xml,application\\/xml;q=0.9,image\\/webp,image\\/apng,*\\/*;q=0.8"), "user_agent": String("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36")}), payment_experience: None, payment_method_type: Some(Ach), business_country: None, business_label: None, merchant_connector_details: None, allowed_payment_method_types: None, business_sub_label: None, retry_action: None, metadata: None, connector_metadata: None, feature_metadata: None }"#;

        let parse = root::<(&str, ErrorKind)>(data).unwrap().1;
        panic!("{:#?}", parse);
    }

    #[test]
    fn test_edge_case2() {
        let data = r#"PaymentsResponse { payment_id: Some("VND9P0YMT7S91EZF7NK2"), merchant_id: Some("reloadhero113"), status: Succeeded, amount: 370, amount_capturable: Some(0), amount_received: Some(370), connector: Some("trustpay"), client_secret: Some(*** alloc::string::String ***), created: Some(2023-09-21 9:42:47.856847), currency: "EUR", customer_id: Some("e064f3fe-a027-458a-a373-09eb38122b67"), description: None, refunds: None, disputes: None, attempts: None, captures: None, mandate_id: None, mandate_data: None, setup_future_usage: None, off_session: None, capture_on: None, capture_method: None, payment_method: None, payment_method_data: None, payment_token: Some("token_K1vASOnmHBh292RJExlQ"), shipping: None, billing: Some(Address { address: Some(AddressDetails { city: Some("Bengaluru"), country: Some(DE), line1: Some(*** alloc::string::String ***), line2: None, line3: None, zip: Some(*** alloc::string::String ***), state: None, first_name: Some(*** alloc::string::String ***), last_name: Some(*** alloc::string::String ***) }), phone: Some(PhoneDetails { number: None, country_code: None }) }), order_details: None, email: Some(Encryptable { inner: ****@test.com, encrypted: *** Encrypted 41 of bytes *** }), name: Some(Encryptable { inner: *** alloc::string::String ***, encrypted: *** Encrypted 37 of bytes *** }), phone: None, return_url: Some("http://localhost:3000/en/checkout/result"), authentication_type: Some(ThreeDs), statement_descriptor_name: None, statement_descriptor_suffix: None, next_action: None, cancellation_reason: None, error_code: None, error_message: None, payment_experience: None, payment_method_type: None, connector_label: None, business_country: None, business_label: None, business_sub_label: None, allowed_payment_method_types: Some(Array [String("credit"), String("debit"), String("crypto_currency"), String("apple_pay"), String("google_pay"), String("giropay")]), ephemeral_key: None, manual_retry_allowed: Some(false), connector_transaction_id: Some("pGbTn8clC7RASLMxnCWmUA"), frm_message: None, metadata: None, connector_metadata: None, feature_metadata: None, reference_id: None, profile_id: Some("pro_BOWTexIKYSXp2hhehu4a"), attempt_count: 1, merchant_decision: None }"#;

        let parse = root::<(&str, ErrorKind)>(data).unwrap().1;
        panic!("{:#?}", parse);
    }
}
