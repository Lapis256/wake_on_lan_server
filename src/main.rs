use std::{collections::HashMap, fs::read_to_string, str::FromStr};

use actix_web::{App, Error, HttpServer, Responder, error, middleware, post, web};
use serde::{Deserialize, Deserializer};
use wol::{MacAddr, send_wol};

#[derive(Debug, Clone, Deserialize)]
struct Config {
    port: u16,

    #[serde(deserialize_with = "deserialize_devices")]
    devices: HashMap<String, MacAddr>,
}

impl Config {
    fn load() -> Result<Self, String> {
        let config_str = read_to_string("config.toml").map_err(|e| format!("Failed to read config.toml: {e}"))?;
        toml::from_str(&config_str).map_err(|e| format!("Failed to parse config.toml: {e}"))
    }
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
        .ok_or_else(|| error::ErrorNotFound(format!("No such device: {device_name}\n")))?;

    send_wol(*mac_addr, None, None)
        .map_err(|e| error::ErrorInternalServerError(format!("Failed to send WOL packet: {e}\n")))?;

    Ok(format!("Waking up device: {device_name} (Mac Address: {mac_addr})\n"))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = Config::load().unwrap_or_else(|e| panic!("Failed to parse config.toml: {}", e));
    let port = config.port;

    log::info!("starting HTTP server at http://localhost:{port}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config.clone()))
            .wrap(middleware::Logger::default())
            .service(wake_on_lan)
            .default_service(web::to(|| async { "Not Found" }))
    })
    .workers(2)
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
