use serde::{Deserialize};

#[derive(Deserialize)]
struct Map {
    name: String,
    title: String,
    prefix: String,
    perspective: String,
    scale: f64,
    mapzoomout: u64,
    image_format: String
}

#[derive(Deserialize)]
struct World {
    name: String,
    title: String,
    maps: Vec<Map>
}

#[derive(Deserialize)]
struct Configuration {
    worlds: Vec<World>
}

async fn get_dynmap_config(base_url: &str) {
    let resp = reqwest::get(fmt!("{}{}", base_url, "/up/configuration"))
        .await?
        .json::<DynmapConfiguration>()
        .await?;
}
