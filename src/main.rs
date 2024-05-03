#[macro_use]
extern crate lazy_static;

use std::collections::HashSet;
use std::io;
use std::str::from_utf8;
use std::time::{Duration, Instant};

use actix_files as fs;
use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::MultipartForm;
use actix_web::middleware::Logger;
use actix_web::web::{Bytes, Form};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use awc::Client;
use chrono::{NaiveDate, ParseError};
use coach::config::{load_config, Config};
use env_logger::Env;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
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
        tera
    };
}

struct AppState {
    config: Config,
    pool: PgPool,
}

#[derive(Debug, MultipartForm)]
struct MeetEntriesUploadForm {
    #[multipart(rename = "meet-entries-file")]
    files: Vec<TempFile>,
}

#[derive(Serialize, Deserialize)]
struct MeetForm {
    id: String,
}

async fn home_view() -> impl Responder {
    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("index.html", &context).unwrap())
}

async fn import_meet_entries(
    state: web::Data<AppState>,
    MultipartForm(form): MultipartForm<MeetEntriesUploadForm>,
) -> impl Responder {
    for csv_file in form.files {
        let now = Instant::now();
        let reader = io::BufReader::new(csv_file.file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        println!("Started importing meet entries.");
        let mut swimmers = HashSet::new();
        let mut num_entries = 0;
        for (i, record) in csv_reader.records().enumerate() {
            match record {
                Ok(row) => {
                    match import_swimmer(&state.get_ref().pool, &row, i).await {
                        Ok(swimmer_id) => {
                            let _b = swimmers.insert(swimmer_id);
                        }
                        Err(e) => println!("Error importing swimmer: {}", e),
                    };
                    import_times(&state.get_ref().pool, &row, i).await;
                    num_entries += 1;
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        let elapsed = now.elapsed();
        register_load(&state.get_ref().pool, swimmers, num_entries, elapsed).await;
        println!("Finished importing meet entries.")
    }

    let context = Context::new();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("meet.html", &context).unwrap())
}

async fn import_swimmer(
    conn: &PgPool,
    row: &csv::StringRecord,
    row_num: usize,
) -> Result<String, ParseError> {
    let swimmer_id = row.get(0).unwrap();
    let full_name = row.get(4).unwrap();
    let last_name = full_name.split(' ').next();
    let first_name = full_name.split(' ').last();
    let gender = row.get(5).unwrap().to_uppercase();
    let birth = row.get(7).unwrap();
    let birth_date = match NaiveDate::parse_from_str(birth, "%b-%d-%y") {
        Ok(dt) => dt,
        Err(e) => {
            println!(
                "Error decoding date of birth at line {}: {}",
                row_num + 1,
                e
            );
            return Err(e);
        }
    };

    sqlx::query!(
        "
            insert into swimmer (id, name_first, name_last, gender, birth_date) 
            values ($1, $2, $3, $4, $5)
            on conflict do nothing
       ",
        swimmer_id,
        first_name,
        last_name,
        gender,
        birth_date
    )
    .execute(conn)
    .await
    .expect("Error inserting a swimmer");

    Ok(swimmer_id.to_string())
}

async fn import_times(conn: &PgPool, row: &csv::StringRecord, row_num: usize) {
    let swimmer_id = row.get(0).unwrap();
    let event = row.get(9).unwrap();
    let distance: i32 = event.split(' ').next().unwrap().parse().unwrap();
    let style = match event.split(' ').last().unwrap() {
        "Fr" => "FREESTYLE",
        "Bk" => "BACKSTROKE",
        "Br" => "BREASTSTROKE",
        "FL" => "BUTTERFLY",
        "I.M" => "MEDLEY",
        &_ => "",
    };

    let best_time_short = match row.get(12) {
        Some(time) => {
            if time.is_empty() {
                ""
            } else {
                &time[..8]
            }
        }
        None => return,
    };

    if !best_time_short.is_empty() {
        let best_time_minute = best_time_short
            .split(':')
            .next()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        let best_time_second = best_time_short
            .split(':')
            .nth(1)
            .unwrap()
            .split('.')
            .next()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        let best_time_milisecond = best_time_short
            .split('.')
            .last()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        let best_time: i32 =
            best_time_minute * 60000 + best_time_second * 1000 + best_time_milisecond * 10;

        let best_time_short_date = match NaiveDate::parse_from_str(row.get(13).unwrap(), "%b-%d-%y")
        {
            Ok(dt) => dt,
            Err(e) => {
                println!(
                    "Error decoding best time date at line {}: {}",
                    row_num + 1,
                    e
                );
                return;
            }
        };

        sqlx::query!("
                insert into swimmer_time (swimmer, style, distance, course, time_official, time_date)
                values ($1, $2, $3, $4, $5, $6)
                on conflict do nothing
            ", swimmer_id, style, distance, "SHORT", best_time, best_time_short_date)
            .execute(conn)
            .await.expect("Error inserting a swimmer");
    }

    let best_time_long = match row.get(14) {
        Some(time) => {
            if time.is_empty() {
                return;
            } else {
                &time[..8]
            }
        }
        None => return,
    };
    let best_time_minute = best_time_long
        .split(':')
        .next()
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let best_time_second = best_time_long
        .split(':')
        .nth(1)
        .unwrap()
        .split('.')
        .next()
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let best_time_milisecond = best_time_long
        .split('.')
        .last()
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let best_time = best_time_minute * 60000 + best_time_second * 1000 + best_time_milisecond * 10;

    let best_time_long_date = match NaiveDate::parse_from_str(row.get(15).unwrap(), "%b-%d-%y") {
        Ok(dt) => dt,
        Err(e) => {
            println!(
                "Error decoding best time date at line {}: {}",
                row_num + 1,
                e
            );
            return;
        }
    };

    sqlx::query!(
        "
            insert into swimmer_time (swimmer, style, distance, course, time_official, time_date)
            values ($1, $2, $3, $4, $5, $6)
            on conflict do nothing
        ",
        swimmer_id,
        style,
        distance,
        "LONG",
        best_time,
        best_time_long_date
    )
    .execute(conn)
    .await
    .expect("Error inserting a swimmer");
}

async fn register_load(
    conn: &PgPool,
    swimmers: HashSet<String>,
    num_entries: i32,
    duration: Duration,
) {
    let num_swimmers = swimmers.len() as i32;
    let mut ss: String = String::new();
    let mut sep: String = "".to_string();
    for swimmer in swimmers {
        ss.push_str(format!("{}{}", sep, swimmer).as_str());
        sep = ", ".to_string();
    }

    sqlx::query!(
        "
            insert into entries_load (num_swimmers, num_entries, duration, swimmers)
            values ($1, $2, $3, $4)
        ",
        num_swimmers,
        num_entries,
        duration.as_millis() as i32,
        ss
    )
    .execute(conn)
    .await
    .expect("Error inserting a swimmer");
}

async fn compare_with_meet(state: web::Data<AppState>, form: Form<MeetForm>) -> impl Responder {
    let url = format!("{}{}", &state.get_ref().config.results_url, form.id);
    println!("Downloading results from {}", url);

    let client = Client::default();
    let mut res = client
        .get(url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .unwrap();
    let body: Bytes = res.body().limit(20_000_000).await.unwrap();
    let html = from_utf8(&body).unwrap();
    println!("{}", html);

    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("results.html", &context).unwrap())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let config = load_config().expect("Failed to load config");
    let server_port = config.server_port;
    let pool = PgPool::connect(&config.database.url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("storage/migrations")
        .run(&pool)
        .await
        .expect("Failed to migrate database");

    let app_state = AppState { config, pool };
    let data_app_state = web::Data::new(app_state);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(fs::Files::new("/static", "./static").show_files_listing())
            .route("/", web::get().to(home_view))
            .route("/meet/entries", web::post().to(import_meet_entries))
            .route("/meet/results", web::post().to(compare_with_meet))
            .app_data(data_app_state.clone())
    })
    .bind(("0.0.0.0", server_port))?
    .run()
    .await
}
