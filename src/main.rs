#[macro_use]
extern crate lazy_static;

use std::collections::HashSet;
use std::io::{self, Read};
use std::str::from_utf8_unchecked;
use std::time::{Duration, Instant};

use actix_files as fs;
use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::MultipartForm;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{NaiveDate, ParseError};
use coach::config::{load_config, Config};
use env_logger::Env;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::Row;
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                log::error!("Template parsing error(s): {}", e);
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

#[derive(MultipartForm)]
struct MeetResultsForm {
    #[multipart(rename = "meet-results-file")]
    files: Vec<TempFile>,
}

#[derive(Serialize, Deserialize)]
struct MeetForm {
    id: String,
}

#[derive(serde::Serialize)]
struct Swimmer {
    id: String,
    first_name: String,
    last_name: String,
    gender: String,
    birth_date: NaiveDate,
}

#[derive(serde::Serialize)]
struct SwimmerTime {
    swimmer: Swimmer,
    style: String,
    distance: i32,
    course: String,
    time: i32,
    time_date: NaiveDate,
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

        log::info!("Started importing meet entries.");
        let mut swimmers = HashSet::new();
        let mut num_entries = 0;
        for (i, record) in csv_reader.records().enumerate() {
            match record {
                Ok(row) => {
                    match import_swimmer(&state.get_ref().pool, &row, i).await {
                        Ok(swimmer_id) => {
                            let _b = swimmers.insert(swimmer_id);
                        }
                        Err(e) => log::warn!("Failed importing swimmer at line {}: {}", i + 1, e),
                    };
                    import_times(&state.get_ref().pool, &row, i).await;
                    num_entries += 1;
                }
                Err(e) => log::error!("Error: {}", e),
            }
        }
        let elapsed = now.elapsed();
        register_load(&state.get_ref().pool, swimmers, num_entries, elapsed).await;
        log::info!("Finished importing meet entries.")
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
            log::warn!(
                "Failed decoding date of birth at line {}: {}",
                row_num + 1,
                e
            );
            return Err(e);
        }
    };

    sqlx::query(
        "
            insert into swimmer (id, name_first, name_last, gender, birth_date) 
            values ($1, $2, $3, $4, $5)
            on conflict do nothing
        ",
    )
    .bind(swimmer_id)
    .bind(first_name)
    .bind(last_name)
    .bind(gender)
    .bind(birth_date)
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
        Some(time) =>
            if time.is_empty() {
                ""
            } else {
                &time[..8]
            },
        None => return,
    };

    if !best_time_short.is_empty() {
        let best_time_short_date = match NaiveDate::parse_from_str(row.get(13).unwrap(), "%b-%d-%y")
        {
            Ok(dt) => dt,
            Err(e) => {
                log::warn!(
                    "Failed decoding best time date at line {}: {}",
                    row_num + 1,
                    e
                );
                return;
            }
        };

        import_time(
            conn,
            swimmer_id,
            style,
            distance,
            "SHORT",
            best_time_short,
            best_time_short_date,
        )
        .await;
    }

    let best_time_long = match row.get(14) {
        Some(time) => 
            if time.is_empty() {
                return;
            } else {
                &time[..8]
            },
        None => return,
    };

    let best_time_long_date = match NaiveDate::parse_from_str(row.get(15).unwrap(), "%b-%d-%y") {
        Ok(dt) => dt,
        Err(e) => {
            log::warn!(
                "Failed decoding best time date at line {}: {}",
                row_num + 1,
                e
            );
            return;
        }
    };

    import_time(
        conn,
        swimmer_id,
        style,
        distance,
        "LONG",
        best_time_long,
        best_time_long_date,
    )
    .await;
}

async fn import_time(
    conn: &PgPool,
    swimmer_id: &str,
    style: &str,
    distance: i32,
    course: &str,
    best_time: &str,
    best_time_date: NaiveDate,
) {
    let best_time_msecs = time_to_miliseconds(best_time);

    sqlx::query(
        "
        insert into swimmer_time (swimmer, style, distance, course, time_official, time_date)
        values ($1, $2, $3, $4, $5, $6)
        on conflict do nothing
    ",
    )
    .bind(swimmer_id)
    .bind(style)
    .bind(distance)
    .bind(course)
    .bind(best_time_msecs)
    .bind(best_time_date)
    .execute(conn)
    .await
    .expect("Error inserting swimmer's time");
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

    sqlx::query(
        "
            insert into entries_load (num_swimmers, num_entries, duration, swimmers)
            values ($1, $2, $3, $4)
        ",
    )
    .bind(num_swimmers)
    .bind(num_entries)
    .bind(duration.as_millis() as i32)
    .bind(ss)
    .execute(conn)
    .await
    .expect("Error inserting a swimmer");
}

async fn search_swimmer_by_name(conn: &PgPool, name: String) -> Result<Swimmer, sqlx::Error> {
    let first_name = name.split(' ').next();
    let last_name = name.split(' ').nth(1);

    sqlx::query(
        "
        select id, name_first, name_last, gender, birth_date 
        from swimmer
        where name_first = $1 and name_last = $2
    ",
    )
    .bind(first_name)
    .bind(last_name)
    .map(|row: PgRow| Swimmer {
        id: row.get("id"),
        first_name: first_name.unwrap().trim().to_string(),
        last_name: last_name.unwrap().trim().to_string(),
        gender: row.get("gender"),
        birth_date: row.get("birth_date"),
    })
    .fetch_one(conn)
    .await
}

async fn import_meet_results(
    state: web::Data<AppState>,
    MultipartForm(form): MultipartForm<MeetResultsForm>,
) -> impl Responder {
    let cell_selector = Selector::parse(r#"table > tbody > tr > td"#).unwrap();
    let name_selector = Selector::parse(r#"b"#).unwrap();
    let re = Regex::new(r"^[0-5][0-9]:[0-5][0-9].[0-9]{2}\s$").unwrap();

    for mut results_file in form.files {
        let mut raw_results = Vec::new();
        results_file
            .file
            .read_to_end(&mut raw_results)
            .expect("Unable to read");
        let str_results = unsafe { from_utf8_unchecked(&raw_results) };

        let html = Html::parse_document(str_results);
        let mut column_idx = 0;
        let mut skip_swimmer = false;

        let mut swimmer_time: SwimmerTime = SwimmerTime {
            swimmer: Swimmer {
                id: String::new(),
                first_name: String::new(),
                last_name: String::new(),
                gender: String::new(),
                birth_date: NaiveDate::MIN,
            },
            style: String::new(),
            distance: 0,
            course: String::new(),
            time: 0,
            time_date: NaiveDate::MIN,
        };

        for content in html.select(&cell_selector) {
            let mut skip_name = false;

            for name in content.select(&name_selector) {
                let name_cell = name.inner_html();
                let full_name = name_cell.split(',').next();
                match search_swimmer_by_name(&state.as_ref().pool, full_name.unwrap().to_string()).await
                {
                    Ok(swimmer) => {
                        println!(
                            "Swimmer: {:?} : {} {} : {}",
                            swimmer.id, swimmer.first_name, swimmer.last_name, name_cell.split(' ').last().unwrap()
                        );
                        swimmer_time.swimmer = swimmer;
                        skip_name = true;
                        skip_swimmer = false;
                    }
                    Err(e) => { 
                        log::warn!("Swimmer '{}' not found: {}", name_cell, e);
                        skip_swimmer = true;
                    }
                };
            }

            if skip_name || skip_swimmer {
                continue
            }

            let cell = content.inner_html();

            if cell == "&nbsp;" {
                column_idx = 5; // so it enters the next condition.
            }

            if column_idx == 5 {
                column_idx = 0;
                continue;
            }

            match column_idx {
                0 => {
                    if re.is_match(&cell) {
                        let result_time = &cell[..8];
                        swimmer_time.time = time_to_miliseconds(result_time);
        
                        if cell.ends_with('L') {
                            swimmer_time.course = "LONG".to_string();
                        }
        
                        if cell.ends_with('S') {
                            swimmer_time.course = "SHORT".to_string();
                        }
                    }
                },
                2 => {
                    println!("{}", cell);
                    swimmer_time.swimmer.gender = cell.split(' ').next().unwrap().to_uppercase();
                }
                _ => (),
            }
            
            column_idx += 1;
        }
        log::info!("File name: {}", results_file.file_name.unwrap());
    }

    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("results.html", &context).unwrap())
}

/// Converts text in the format mm:ss.ms to miliseconds.
fn time_to_miliseconds(time: &str) -> i32 {
    if time.is_empty() {
        return 0
    }

    let time_minute = match time.split(':').next() {
        Some(s) => match s.parse::<i32>() {
            Ok(i) => i,
            Err(e) => {
                log::error!("Error: {} {}", e, s);
                0
            }
        },
        None => 0,
    };
    
    let time_second = time
        .split(':')
        .nth(1)
        .unwrap()
        .split('.')
        .next()
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let time_milisecond = time.split('.').last().unwrap().parse::<i32>().unwrap();
    time_minute * 60000 + time_second * 1000 + time_milisecond * 10
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
            .route("/meet/results", web::post().to(import_meet_results))
            .app_data(data_app_state.clone())
    })
    .bind(("0.0.0.0", server_port))?
    .run()
    .await
}
