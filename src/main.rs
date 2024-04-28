#[macro_use]
extern crate lazy_static;

use actix_files as fs;
use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::MultipartForm;
use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use coach::config::load_config;
use chrono::NaiveDate;
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

async fn import_meet_entries(conn: web::Data<PgPool>, MultipartForm(form): MultipartForm<UploadForm>) -> Result<impl Responder, Error> {
    for csv_file in form.files {
        let reader = io::BufReader::new(csv_file.file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        for (i, record) in csv_reader.records().enumerate() {
            match record {
                Ok(row) => import_row(conn.get_ref(), &row, i).await,
                Err(e) => println!("Error: {}", e)
            }
        }
        println!("Finished importing swimmers.")
    }

    Ok(HttpResponse::Ok())
}

async fn import_row(conn: &PgPool, row: &csv::StringRecord, row_num: usize) {
    let swimmer_id = row.get(0).unwrap();
    let full_name = row.get(4).unwrap();
    let last_name = full_name.split(" ").nth(0);
    let first_name = full_name.split(" ").last();
    let gender = row.get(5).unwrap().to_uppercase();
    let birth = row.get(7).unwrap();
    let birth_date = match NaiveDate::parse_from_str(birth, "%b-%d-%y") {
        Ok(dt) => dt,
        Err(e) => {
            println!("Error decoding date of birth at line {}: {}", row_num + 1, e);
            return
        }
    };
    
    sqlx::query!("
            insert into swimmer (id_external, name_first, name_last, gender, birth_date) 
            values ($1, $2, $3, $4, $5)
            on conflict do nothing
        ", swimmer_id, first_name, last_name, gender, birth_date)
        .execute(conn)
        .await.expect("Error inserting a swimmer");
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

    let conn = web::Data::new(pool);

    HttpServer::new(move || {
        App::new()
            .service(fs::Files::new("/static", "./static").show_files_listing())
            .route("/", web::get().to(home_view))
            .route("/meet/results", web::post().to(import_meet_entries))
            .app_data(conn.clone())})
        .bind(("0.0.0.0", config.server_port))?
        .run()
        .await
}
