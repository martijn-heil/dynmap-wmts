use bytes::Bytes;
use futures_core::stream::Stream;
use reqwest::Client;
use reqwest::header::ACCEPT;
use reqwest;
use rocket::http::ContentType;
use serde::{Deserialize};

pub struct Dynmap {
    client: Client,
    base_url: String
}

impl Dynmap {
    pub fn new(base_url: String) -> Self {
        Dynmap {
            client: Client::new(),
            base_url: base_url
        }
    }

    pub async fn get_config(&self) -> Result<Configuration, Box<dyn std::error::Error>> {
        let resp = reqwest::get(format!("{}{}", self.base_url, "/up/configuration"))
            .await?
            .json::<Configuration>()
            .await?;

        Ok(resp)
    }

    pub async fn get_tile(&self, world: &str, map: &str, zoom: u64, x: i64, z: i64, image_format: &str) 
    -> Result<impl Stream<Item = reqwest::Result<Bytes>>, Box<dyn std::error::Error>> {
        let content_type = ContentType::from_extension(&image_format).unwrap().to_string();
        let url = format!("{}/tiles/{}/{}/0_0/{}{}_{}.{}", self.base_url, world, map, zoom_prefix(zoom), x, z, image_format);
        let resp = self.client.get(url)
            .header(ACCEPT, content_type)
            .send()
            .await?
            .bytes_stream();

        Ok(resp)
    }
}

#[derive(Debug, Deserialize)]
pub struct Map {
    pub name: String,
    pub title: String,
    pub prefix: String,
    pub perspective: String,
    pub scale: u64,
    pub tilescale: u64, // see tile_size()
    pub mapzoomout: u64,
    #[serde(rename = "image-format")] 
    pub image_format: String // This is always a file extension, not a content type
}

#[derive(Debug, Deserialize)]
pub struct World {
    pub name: String,
    pub title: String,
    pub maps: Vec<Map>
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    pub worlds: Vec<World>
}

pub fn tile_size(tilescale: u64) -> u64 { 128 << tilescale }

fn zoom_prefix(zoom: u64) -> String {
    if zoom == 0 { return String::new(); }

		// zoom == 0 -> ''
		// zoom == 1 -> 'z_'
		// zoom == 2 -> 'zz_'
		let mut tmp = String::from(&"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"[..(zoom as usize)]);
    tmp.push_str("_");
    return tmp
}
