use std::{collections::HashMap, fs::read_to_string, str::FromStr};

use actix_web::{App, Error, HttpServer, Responder, middleware, post, web};
use serde::{Deserialize, Deserializer};
use wol::{MacAddr, send_wol};

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(deserialize_with = "deserialize_devices")]
    devices: HashMap<String, MacAddr>,
}

fn validate_device_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Device name cannot be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| (c.is_lowercase() && c.is_ascii_alphanumeric()) || c == '_')
    {
        return Err("Device name must be lowercase and alphanumeric or underscore".to_string());
    }
    Ok(())
}

fn deserialize_devices<'de, D>(deserializer: D) -> Result<HashMap<String, MacAddr>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, String> = HashMap::deserialize(deserializer)?;

    let map = map
        .into_iter()
        .map(|(name, mac_str)| {
            validate_device_name(&name).map_err(serde::de::Error::custom)?;

            Ok((name, MacAddr::from_str(&mac_str).map_err(serde::de::Error::custom)?))
        })
        .collect::<Result<HashMap<String, MacAddr>, _>>()?;

    Ok(map)
}

#[post("/wol/{device_name}")]
async fn wake_on_lan(device_name: web::Path<String>, config: web::Data<Config>) -> Result<impl Responder, Error> {
    let mac_addr = config
        .devices
        .get(device_name.as_str())
        .ok_or_else(|| actix_web::error::ErrorNotFound(format!("No such device: {device_name}")))?;

    send_wol(*mac_addr, None, None)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to send WOL packet: {}", e)))?;

    Ok(format!("Waking up device: {device_name} (Mac Address: {mac_addr})"))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(|| {
        let config = read_to_string("config.toml").expect("Failed to read config.toml");
        let config = match toml::from_str::<Config>(&config) {
            Ok(config) => config,
            Err(e) => {
                panic!("Failed to parse config.toml: {}", e);
            }
        };

        App::new()
            .app_data(web::Data::new(config))
            .wrap(middleware::Logger::default())
            .service(wake_on_lan)
            .default_service(web::to(|| async { "Not Found" }))
    })
    .workers(2)
    .bind(("0.0.0.0", 8888))?
    .run()
    .await
}
