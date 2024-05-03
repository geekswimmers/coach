use std::env;

use config::ConfigError;

#[derive(serde::Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server_port: u16,
    pub results_url: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

pub fn load_config() -> Result<Config, config::ConfigError> {
    match config::Config::builder()
        .add_source(config::File::new("config.yaml", config::FileFormat::Yaml))
        .build()
    {
        Ok(config) => config
            .try_deserialize::<Config>()
            .or_else(load_config_from_env),
        Err(e) => load_config_from_env(e),
    }
}

fn load_config_from_env(e: ConfigError) -> Result<Config, config::ConfigError> {
    log::info!("{}. Loading from environment variables instead.", e);

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .expect("PORT must be a number");

    let results_url = env::var("RESULTS_URL").unwrap_or_else(|_| "".to_string());

    let config: Config = match env::var("DATABASE_URL") {
        Ok(url) => Config {
            server_port: port,
            database: DatabaseConfig { url },
            results_url,
        },
        Err(e) => {
            log::error!("DATABASE_URL: {}", e);
            Config {
                server_port: port,
                database: DatabaseConfig {
                    url: String::from(""),
                },
                results_url,
            }
        }
    };
    Ok(config)
}
