use std::env;

#[derive(serde::Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server_port: u16,
}

#[derive(serde::Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

pub fn load_config() -> Result<Config, config::ConfigError> {
    match config::Config::builder()
        .add_source(config::File::new("config.yaml", config::FileFormat::Yaml))
        .build() {
            Ok(config) => {
                match config.try_deserialize::<Config>() {
                    Ok(c) => Ok(c),
                    Err(e) => {
                        println!("Error deserializing config file: {}. Trying environment variables.", e);
                        match load_config_from_env() {
                            Ok(c) => Ok(c),
                            Err(e) => Err(e),
                        }
                    }
                }
            },
            Err(e) => {
                println!("{}. Loading from environment variables instead.", e);
                match load_config_from_env() {
                    Ok(c) => Ok(c),
                    Err(e) => Err(e),
                }
            },
        }
}

pub fn load_config_from_env() -> Result<Config, config::ConfigError> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string()).parse().expect("PORT must be a number");
    let config: Config;

    if database_url.is_empty() {
        config = Config {
            server_port: port,
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").unwrap_or_else(|_| "".to_string()),
            }
        };
    } else {
        config = Config {
            server_port: port,
            database: DatabaseConfig {
                url: database_url,
            },
        };
    }

    Ok(config)
}