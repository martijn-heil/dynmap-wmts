#[macro_use] extern crate rocket;
use bytes::Bytes;
use dotenv::dotenv;
use handlebars::Handlebars;
use ordered_float::NotNan;
use rocket::State;
use rocket::fairing::AdHoc;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::response::stream::ReaderStream;
use serde::{Serialize};
use std::collections::HashSet;
use std::path::Path;
use tokio_util::io::StreamReader;

mod dynmap;

use dynmap::Dynmap;

const CAPABILITIES_TEMPLATE: &str = include_str!("capabilities.xml");
const PIXEL_SIZE: f64 = 0.00028;
const BASE_BLOCKS_PER_TILE: u64 = 32;
const WORLD_SIZE: u64 = i16::MAX as u64;
const TOP_LEFT: (i64, i64) = (0 - WORLD_SIZE as i64 / 2 , 0 - (0 - WORLD_SIZE as i64 / 2 - 32));

#[derive(Debug, Serialize)]
struct Capabilities {
    base_url: String,
    layers: Vec<Layer>,
    tile_matrix_sets: HashSet<TileMatrixSet>
}

#[derive(Debug, Serialize)]
struct Layer {
    identifier: String,
    title: String,
    tile_content_type: String,
    tile_url_template: String
}

#[derive(Debug, Serialize, Eq, PartialEq, Hash)]
struct TileMatrix {
    identifier: String,
    tile_size: u64,
    scale_denominator: NotNan<f64>,
    top_left_corner: (i64, i64),
    matrix_size: u64
}

#[derive(Debug, Serialize, Eq, PartialEq, Hash)]
struct TileMatrixSet {
    identifier: String,
    title: String,
    matrices: Vec<TileMatrix>
}

fn scale_denominator(tile_size: u64, zoomout: u64) -> f64 {
    let scale = (tile_size as f64) * PIXEL_SIZE / BASE_BLOCKS_PER_TILE as f64;
    return 1.0 / scale * 2_f64.powf(zoomout as f64);
}

fn matrix_size(zoomout: u64) -> u64 {
    return (WORLD_SIZE as f64 / BASE_BLOCKS_PER_TILE as f64 / 2f64.powf(zoomout as f64)).ceil() as u64;
}

impl dynmap::Configuration {
    pub fn get_tile_matrix_sets(self: &dynmap::Configuration) -> HashSet<TileMatrixSet> {
        let mut tile_matrix_sets: HashSet<TileMatrixSet> = HashSet::new();

        for world in self.worlds.iter() {
            for map in world.maps.iter() {
                let mut matrices: Vec<TileMatrix> = Vec::new();
                let tile_size = dynmap::tile_size(map.tilescale);
                let max_zoomout = map.mapzoomout;

                for zoomout in 0..=max_zoomout {
                    matrices.push(TileMatrix {
                        identifier: format!("{}", zoomout),
                        tile_size: tile_size,
                        scale_denominator: NotNan::new(scale_denominator(tile_size, zoomout)).unwrap(),
                        top_left_corner: TOP_LEFT,
                        matrix_size: matrix_size(zoomout)
                    });
                }

                tile_matrix_sets.insert(TileMatrixSet {
                    identifier: format!("{}_{}", &world.name, &map.name),
                    title: format!("{} {}", &world.title, &map.title),
                    matrices: matrices
                });
            }
        }

        tile_matrix_sets
    }
}

#[get("/WMTSCapabilities.xml")]
async fn get_capabilities(dynmap: &State<Dynmap>) -> Result<(ContentType, String), Status> {
    let base_url = std::env::var("BASE_URL").expect("BASE_URL must be set.");

    let config = dynmap.get_config().await.unwrap();

    let capabilities = Capabilities {
        base_url: base_url.clone(),
        tile_matrix_sets: config.get_tile_matrix_sets(),
        layers: config.worlds.into_iter().flat_map(|world| world.maps.into_iter().map(move |map| {
            let identifier = format!("{}_{}", &world.name, &map.name);
            Layer {
                identifier: identifier.clone(),
                title: format!("{} {}", &world.title, &map.title),
                tile_content_type: ContentType::from_extension(&map.image_format).unwrap().to_string(),
                tile_url_template: format!("{}/tiles/{}/{}/{{TileMatrix}}/{{TileCol}}/{{TileRow}}.{}", 
                    std::env::var("BASE_URL").unwrap(),
                    &world.name,
                    &map.name,
                    &map.image_format
                )
            }
        })).collect()
    };

    let reg = Handlebars::new();
    let response = reg.render_template(CAPABILITIES_TEMPLATE, &capabilities);
    Ok((ContentType::XML, response.unwrap()))
}

#[get("/tiles/<world>/<map>/<tile_matrix>/<tile_col>/<file>")]
async fn get_tile(
    dynmap: &State<Dynmap>,
    world: &str,
    map: &str,
    tile_matrix: u64,
    tile_col: i64,
    file: &str) -> Result<(ContentType, ReaderStream![StreamReader<impl rocket::futures::Stream<Item = Result<Bytes, std::io::Error>>, Bytes>]), Status> {
    let file_path = Path::new(file);
    let extension = match file_path.extension() {
        Some(x) => match x.to_str() {
            Some(s) => s,
            None => return Err(Status::UnprocessableEntity)
        },
        None => return Err(Status::UnprocessableEntity)
    };
    let stem = match file_path.file_stem() {
        Some(x) => match x.to_str() {
            Some(s) => s,
            None => return Err(Status::UnprocessableEntity)
        },
        None => return Err(Status::UnprocessableEntity)
    };
    let tile_row: i64 = match stem.parse() {
        Ok(tile_row) => tile_row,
        Err(_) => return Err(Status::UnprocessableEntity)
    };
    let content_type = match ContentType::from_extension(&extension) {
        Some(v) => v,
        None => return Err(Status::UnprocessableEntity)
    };

    let matrix_size = matrix_size(tile_matrix);
    let zoom_ratio = 2i64.pow(tile_matrix as u32);
    let x = (tile_col - (matrix_size as i64) / 2) * zoom_ratio;
    let y = (tile_row - (matrix_size as i64) / 2) * zoom_ratio;
    let z = -y;

    let stream = dynmap.get_tile(&world, &map, tile_matrix, x, z, &extension).await.unwrap();

    use rocket::futures::TryStreamExt; // for map_err() call below:
    let reader = StreamReader::new(stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    Ok((content_type, ReaderStream::one(reader)))
}

#[launch]
fn rocket() -> _ {
    dotenv().ok();

    let dynmap_url = std::env::var("DYNMAP_URL").expect("DYNMAP_URL must be set.");
    let dynmap = Dynmap::new(dynmap_url);

    rocket::build()
        .manage(dynmap)
        .attach(AdHoc::on_response("CORS", |_, resp| Box::pin(async move {
            resp.set_raw_header("Access-Control-Allow-Origin", "*");
            resp.set_raw_header("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, OPTIONS");
            resp.set_raw_header("Access-Control-Allow-Headers", "DNT,X-CustomHeader,Keep-Alive,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type");
        })))
        .mount("/", routes![get_capabilities, get_tile])
}

