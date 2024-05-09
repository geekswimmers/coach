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
    let configuration = config::Config::builder()
        .add_source(config::File::new("config.yaml", config::FileFormat::Yaml))
        .add_source(config::Environment::with_prefix("COACH")
            .prefix_separator("_")
            .separator("__"))
        .build()?;

    configuration.try_deserialize::<Config>()
}
