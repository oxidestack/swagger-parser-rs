//! Mirrors reference/v1/V1Bean.java: swagger-core 1.x `Json.mapper().convertValue(node, type)` then
//! `Json.mapper().writeValueAsString(result)` for each `{"type", "value"}` case of a JSON array file.
use jackson::json::{JsonWriter, WriteFeatures};
use swagger_parser::v2::{bean, json as v1json, property_builder};

fn main() {
    let file = std::env::args().nth(1).expect("cases file");
    let text = std::fs::read_to_string(file).unwrap();
    let cases = swagger_parser::v2::parser::read_json(&text).unwrap();
    for c in cases.as_array().unwrap() {
        let t = c.get("type").unwrap().as_text();
        let v = c.get("value").unwrap();
        let mut w = JsonWriter::new(WriteFeatures::default());
        let r: Result<bool, String> = (|| {
            let e = |e: swagger_parser::java::JavaException| e.class;
            let j = |e: jackson::JacksonError| e.class;
            match t.as_str() {
                "Model" => match bean::model(v).map_err(e)? {
                    None => Ok(false),
                    Some(m) => v1json::write_model_with(&mut w, &m.borrow(), true).map(|_| true).map_err(j),
                },
                "Parameter" => match bean::parameter(v).map_err(e)? {
                    None => Ok(false),
                    Some(p) => v1json::write_parameter_v1(&mut w, &p.borrow()).map(|_| true).map_err(j),
                },
                "Response" => match bean::response(v).map_err(e)? {
                    None => Ok(false),
                    Some(p) => v1json::write_response_v1(&mut w, &p.borrow()).map(|_| true).map_err(j),
                },
                "Path" => match bean::path(v).map_err(e)? {
                    None => Ok(false),
                    Some(p) => v1json::write_path_v1(&mut w, &p.borrow()).map(|_| true).map_err(j),
                },
                _ => match property_builder::property_from_node(v).map_err(e)? {
                    None => Ok(false),
                    Some(p) => v1json::write_property_with(&mut w, &p.borrow(), true).map(|_| true).map_err(j),
                },
            }
        })();
        let out = match r {
            Ok(true) => w.into_string(),
            Ok(false) => "null".to_string(),
            Err(class) => format!("THROW {class}"),
        };
        println!("{t} {out}");
    }
}
