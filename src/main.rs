#[macro_use]
extern crate lazy_static;

use actix_files as fs;
use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::MultipartForm;
use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use coach::config::load_config;
use sqlx::postgres::PgPool;
use std::io;
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
    for csv_file in form.files {
        let reader = io::BufReader::new(csv_file.file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        for record in csv_reader.records() {
            match record {
                Ok(row) => import_row(&row),
                Err(e) => println!("Error: {}", e)
            }
            println!()
        }
    }

    Ok(HttpResponse::Ok())
}

fn import_row(row: &csv::StringRecord) {
    print!("{} - ", row.get(0).unwrap());
    for column in row { 
        print!("{} ", column)
    }
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
