use coach::config::load_config;
use sqlx::postgres::PgPool;

#[tokio::main]
async fn main() {
    let config = load_config().expect("Failed to load config");
    println!("Database URL: {}", config.database.url);
    let pool = PgPool::connect(&config.database.url).await.expect("Failed to connect to database");

    sqlx::migrate!("storage/migrations")
        .run(&pool)
        .await.expect("Failed to migrate database");
}
