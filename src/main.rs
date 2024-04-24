use coach::config::load_config;


fn main() {
    let config = load_config().expect("Failed to load config");
    println!("Database URL: {}", config.database.url);
}
