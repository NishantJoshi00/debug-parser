#![allow(dead_code)]

use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nom::error::ErrorKind;
use ron_parser::root;

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
    tutu: (i32, f64),
    nothing: Option<()>,
    boolean: bool,
}

#[derive(Debug)]
enum Boat {
    JustOne(i32),
    AnCouple((i32, String)),
    JustStruct { names: Vec<String>, age: i32 },
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
        tutu: (12, -12.5),
        nothing: None,
        boolean: false,
    }
}

fn bench_everything(c: &mut Criterion) {
    let data = generate_data();
    c.bench_function("bench_parser", |b| {
        b.iter(|| {
            let value = format!("{:?}", black_box(&data));
            let _ = black_box(root::<(&str, ErrorKind)>(&value)).unwrap().1;
        })
    });
}

fn bench_everything_dbg_pnt(c: &mut Criterion) {
    let data = generate_data();
    c.bench_function("bench_debug_print", |b| {
        b.iter(|| {
            let _ = format!("{:?}", black_box(&data));
        })
    });
}

fn bench_parse_and_serialize(c: &mut Criterion) {
    let data = generate_data();
    c.bench_function("bench_parse_and_serialize", |b| {
        b.iter(|| {
            let value = format!("{:?}", black_box(&data));
            let _parsed = black_box(root::<(&str, ErrorKind)>(&value).unwrap().1);
        })
    });
}

fn bench_payment_request(c: &mut Criterion) {
    let data = r#"PaymentsRequest { payment_id: None, merchant_id: None, amount: Some(Value(6500)), routing: None, connector: None, currency: Some(USD), capture_method: Some(Automatic), amount_to_capture: None, capture_on: None, confirm: Some(false), customer: None, customer_id: Some("hyperswitch111"), email: Some(Email(*********@gmail.com)), name: None, phone: None, phone_country_code: None, off_session: None, description: Some("Hello this is description"), return_url: None, setup_future_usage: None, authentication_type: Some(ThreeDs), payment_method_data: None, payment_method: None, payment_token: None, card_cvc: None, shipping: Some(Address { address: Some(AddressDetails { city: Some("Banglore"), country: Some(US), line1: Some(*** alloc::string::String ***), line2: Some(*** alloc::string::String ***), line3: Some(*** alloc::string::String ***), zip: Some(*** alloc::string::String ***), state: Some(*** alloc::string::String ***), first_name: Some(*** alloc::string::String ***), last_name: None }), phone: Some(PhoneDetails { number: Some(*** alloc::string::String ***), country_code: Some("+1") }) }), billing: Some(Address { address: Some(AddressDetails { city: Some("San Fransico"), country: Some(AT), line1: Some(*** alloc::string::String ***), line2: Some(*** alloc::string::String ***), line3: Some(*** alloc::string::String ***), zip: Some(*** alloc::string::String ***), state: Some(*** alloc::string::String ***), first_name: Some(*** alloc::string::String ***), last_name: Some(*** alloc::string::String ***) }), phone: Some(PhoneDetails { number: Some(*** alloc::string::String ***), country_code: Some("+91") }) }), statement_descriptor_name: None, statement_descriptor_suffix: None, metadata: Some(Metadata { order_details: Some(OrderDetails { product_name: "gillete razor", quantity: 1 }), order_category: None, redirect_response: None, allowed_payment_method_types: None }), order_details: None, client_secret: None, mandate_data: None, mandate_id: None, browser_info: None, payment_experience: None, payment_method_type: None, business_country: Some(US), business_label: Some("default"), merchant_connector_details: None, allowed_payment_method_types: None, business_sub_label: None, manual_retry: false, udf: None }"#;

    c.bench_function("bench_payment_request", |b| {
        b.iter(|| {
            let _parsed = black_box(root::<(&str, ErrorKind)>(data).unwrap().1);
        })
    });
}

criterion_group!(
    benches,
    bench_everything,
    bench_everything_dbg_pnt,
    bench_parse_and_serialize,
    bench_payment_request
);
criterion_main!(benches);
