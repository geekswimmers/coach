#[macro_use]
extern crate lazy_static;

use actix_files as fs;
use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::MultipartForm;
use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use coach::config::load_config;
use sqlx::postgres::PgPool;
use std::env;
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Template parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        let _ = tera.full_reload();
        return tera;
    };
}

#[derive(Debug, MultipartForm)]
struct UploadForm {
    #[multipart(rename = "meet-entries-file")]
    files: Vec<TempFile>,
}

async fn save_files(MultipartForm(form): MultipartForm<UploadForm>) -> Result<impl Responder, Error> {
    let temp_dir = env::temp_dir();
    for f in form.files {
        let path = format!("{}/{}", temp_dir.display(), f.file_name.unwrap());
        println!("Saving to {}", path);
        f.file.persist(path).unwrap();
    }

    Ok(HttpResponse::Ok())
}

async fn home_view() -> impl Responder {
    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("index.html", &context).unwrap())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = load_config().expect("Failed to load config");
    println!("Database URL: {}", config.database.url);
    let pool = PgPool::connect(&config.database.url).await.expect("Failed to connect to database");

    sqlx::migrate!("storage/migrations")
        .run(&pool)
        .await.expect("Failed to migrate database");

        HttpServer::new(move || {
            App::new()
                .service(fs::Files::new("/static", "./static").show_files_listing())
                .route("/", web::get().to(home_view))
                .route("/meet/results", web::post().to(save_files))
        })
        .bind(("0.0.0.0", config.server_port))?
        .run()
        .await
}
