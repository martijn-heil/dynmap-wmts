#[macro_use] extern crate rocket;
use rocket::http::{ContentType};
use serde_json::json;
use handlebars::Handlebars;
use dotenv::dotenv;

const CAPABILITIES_TEMPLATE: &str = include_str!("capabilities.xml");

#[get("/WMTSCapabilities.xml")]
fn get_capabilities() -> (ContentType, String) {
    let reg = Handlebars::new();
    let base_url = std::env::var("BASE_URL").expect("BASE_URL must be set.");
    let capabilities = reg.render_template(CAPABILITIES_TEMPLATE, &json!({"base_url": base_url, "worlds": [{"name": "testworld"}]}));

    (ContentType::XML, capabilities.unwrap())
}

#[get("/tiles/<tile_matrix>/<tile_col>/<tile_row>")]
fn get_tile(tile_matrix: i64, tile_col: i64, tile_row: i64) -> &'static str {
    "test"
}

#[launch]
fn rocket() -> _ {
    dotenv().ok();
    rocket::build().mount("/", routes![get_capabilities, get_tile])
}

