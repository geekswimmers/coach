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
use actix_web::web::Redirect;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{NaiveDate, ParseError};
use coach::config::load_config;
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

#[derive(Serialize, Clone)]
struct Swimmer {
    id: String,
    first_name: String,
    last_name: String,
    gender: String,
    birth_date: NaiveDate,
}

impl Swimmer {
    pub fn new(id: String) -> Self {
        Self { 
            id,
            first_name: String::new(),
            last_name: String::new(),
            gender: String::new(),
            birth_date: NaiveDate::MIN,
        }
    }
}

#[derive(Serialize)]
struct SwimmerTime {
    swimmer: Swimmer,
    style: String,
    distance: i32,
    course: String,
    time: i32,
    time_date: NaiveDate,
    meet: Meet,
}

#[derive(Serialize, Deserialize, Clone)]
struct Meet {
    id: String,
    name: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
}

impl Meet {
    pub fn new(id: String) -> Self {
        Self { 
            id,
            name: String::new(),
            start_date: NaiveDate::MIN,
            end_date: NaiveDate::MAX,
        }
    }
}

#[derive(Deserialize)]
struct MeetPath {
    id: String,
}

async fn home_view() -> impl Responder {
    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("index.html", &context).unwrap())
}

async fn meets_view(state: web::Data<AppState>) -> impl Responder {
    let meets = sqlx::query(
        "
            select id, name, start_date, end_date 
            from meet
            order by end_date desc
        ",
    )
    .map(|row: PgRow| Meet {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
    })
    .fetch_all(&state.get_ref().pool)
    .await
    .expect("Failed to fetch meets");

    let mut context = Context::new();
    context.insert("meets", &meets);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("meets.html", &context).unwrap())
}

async fn meets_form_view() -> impl Responder {
    let context = Context::new();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("meet_form.html", &context).unwrap())
}

async fn meets_entries_form_view(path: web::Path<MeetPath>, state: web::Data<AppState>) -> impl Responder {
    let meet = find_meet(&state.get_ref().pool, &path.id).await;

    let mut context = Context::new();
    context.insert("meet", &meet);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("entries.html", &context).unwrap())
}

async fn meets_results_form_view(path: web::Path<MeetPath>, state: web::Data<AppState>) -> impl Responder {
    let meet = find_meet(&state.get_ref().pool, &path.id).await;

    let mut context = Context::new();
    context.insert("meet", &meet);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("results.html", &context).unwrap())
}

async fn meets_new(form: web::Form<Meet>, state: web::Data<AppState>) -> impl Responder {
    sqlx::query(
        "
            insert into meet (id, name, start_date, end_date) 
            values ($1, $2, $3, $4)
            on conflict do nothing
        ",
    )
    .bind(form.id.as_str())
    .bind(form.name.as_str())
    .bind(form.start_date)
    .bind(form.end_date)
    .execute(&state.get_ref().pool)
    .await
    .expect("Error inserting a meet.");

    Redirect::to(format!("/meets/{}/", form.id)).see_other()
}

async fn find_meet(conn: &PgPool, meet_id: &str) -> Meet {
    sqlx::query(
        "
            select id, name, start_date, end_date 
            from meet
            where id = $1
        ",
    )
    .bind(meet_id)
    .map(|row: PgRow| Meet {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
    })
    .fetch_one(conn)
    .await
    .expect("Failed to fetch meet")
}

async fn meet_view(path: web::Path<MeetPath>, state: web::Data<AppState>) -> impl Responder {
    let meet = find_meet(&state.get_ref().pool, &path.id).await;

    let mut context = Context::new();
    context.insert("meet", &meet);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("meet.html", &context).unwrap())
}

async fn swimmers_view(state: web::Data<AppState>) -> impl Responder {
    let swimmers = sqlx::query(
        "
            select id, name_first, name_last, gender, birth_date 
            from swimmer
            order by name_first, name_last
        ",
    )
    .map(|row: PgRow| Swimmer {
        id: row.get("id"),
        first_name: row.get("name_first"),
        last_name: row.get("name_last"),
        gender: row.get("gender"),
        birth_date: row.get("birth_date"),
    })
    .fetch_all(&state.get_ref().pool)
    .await
    .expect("Failed to fetch swimmers");

    let mut context = Context::new();
    context.insert("swimmers", &swimmers);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(TEMPLATES.render("swimmers.html", &context).unwrap())
}

async fn import_meet_entries(
    path: web::Path<MeetPath>,
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
                    import_times(&state.get_ref().pool, &row, i, &path.id).await;
                    num_entries += 1;
                }
                Err(e) => log::warn!("{}", e),
            }
        }
        let elapsed = now.elapsed();
        register_load(&state.get_ref().pool, swimmers, num_entries, elapsed, &path.id).await;
        log::info!("Finished importing meet entries.")
    }

    Redirect::to(format!("/meets/{}/", path.id)).see_other()
}

async fn import_swimmer(
    conn: &PgPool,
    row: &csv::StringRecord,
    row_num: usize,
) -> Result<String, ParseError> {
    let swimmer_id = row.get(0).unwrap().trim();
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

async fn import_times(conn: &PgPool, row: &csv::StringRecord, row_num: usize, meet_id: &str) {
    let swimmer_id = row.get(0).unwrap().trim();
    let event = row.get(9).unwrap();
    let distance: i32 = event.split(' ').next().unwrap().parse().unwrap();
    let style = convert_style(event.split(' ').last().unwrap());
    let swimmer = Swimmer::new(swimmer_id.to_string());
    let meet = Meet::new(meet_id.to_string());
    
    let mut swimmer_time: SwimmerTime = SwimmerTime {
        swimmer,
        style: style.to_string(),
        distance,
        course: String::new(),
        time: 0,
        time_date: NaiveDate::MAX,
        meet,
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

        swimmer_time.course = "SHORT".to_string();
        swimmer_time.time = time_to_miliseconds(best_time_short);
        swimmer_time.time_date = best_time_short_date;

        import_time(
            conn,
            &swimmer_time,
        )
        .await;
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

    swimmer_time.course = "LONG".to_string();
    swimmer_time.time = time_to_miliseconds(best_time_long);
    swimmer_time.time_date = best_time_long_date;

    import_time(
        conn,
        &swimmer_time,
    )
    .await;
}

async fn import_time(
    conn: &PgPool,
    swimmer_time: &SwimmerTime,
) {
    sqlx::query(
        "
        insert into swimmer_time (swimmer, style, distance, course, time_official, time_date, meet)
        values ($1, $2, $3, $4, $5, $6, $7)
        on conflict do nothing
    ",
    )
    .bind(&swimmer_time.swimmer.id)
    .bind(&swimmer_time.style)
    .bind(swimmer_time.distance)
    .bind(&swimmer_time.course)
    .bind(swimmer_time.time)
    .bind(swimmer_time.time_date)
    .bind(&swimmer_time.meet.id)
    .execute(conn)
    .await
    .expect("Error inserting swimmer's time");
}

async fn register_load(
    conn: &PgPool,
    swimmers: HashSet<String>,
    num_entries: i32,
    duration: Duration,
    meet_id: &str,
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
            insert into entries_load (num_swimmers, num_entries, duration, swimmers, meet)
            values ($1, $2, $3, $4, $5)
        ",
    )
    .bind(num_swimmers)
    .bind(num_entries)
    .bind(duration.as_millis() as i32)
    .bind(ss)
    .bind(meet_id)
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
    path: web::Path<MeetPath>,
    state: web::Data<AppState>,
    MultipartForm(form): MultipartForm<MeetResultsForm>,
) -> impl Responder {
    let meet = find_meet(&state.as_ref().pool, &path.id).await;

    let row_selector = Selector::parse(r#"table > tbody > tr"#).unwrap();
    let cell_selector = Selector::parse(r#"td"#).unwrap();
    let name_selector = Selector::parse(r#"b"#).unwrap();
    let re_time = Regex::new(r"^[0-5][0-9]:[0-5][0-9].[0-9]{2}\S$").unwrap();

    for mut results_file in form.files {
        println!("File: {}", results_file.file_name.clone().unwrap());
        let mut raw_results = Vec::new();
        results_file
            .file
            .read_to_end(&mut raw_results)
            .expect("Unable to read");
        let str_results = unsafe { from_utf8_unchecked(&raw_results) };
        let mut swimmer = Swimmer {
            id: String::new(),
            first_name: String::new(),
            last_name: String::new(),
            gender: String::new(),
            birth_date: NaiveDate::MIN,
        };
        let html = Html::parse_document(str_results);
        let mut valid_swimmer = true;

        // Iterate over the <tr> found.
        for row in html.select(&row_selector) {
            let mut cell_idx = 0;
            let mut name_row = false;
            let mut valid_row = true;
            let time_date = meet.end_date;

            let mut swimmer_time: SwimmerTime = SwimmerTime {
                swimmer: swimmer.clone(),
                style: String::new(),
                distance: 0,
                course: String::new(),
                time: 0,
                time_date,
                meet: meet.clone(),
            };

            // Iterate over the <td> found within the <tr>.
            for cell in row.select(&cell_selector) {
                // Iterate over the <b> found inside <td>.
                for name in cell.select(&name_selector) {
                    let name_cell = name.inner_html();
                    let full_name = name_cell.split(',').next();
                    match search_swimmer_by_name(
                        &state.as_ref().pool,
                        full_name.unwrap().to_string(),
                    )
                    .await
                    {
                        Ok(s) => {
                            swimmer = s;
                            valid_swimmer = true;
                            name_row = true;
                        }
                        Err(e) => {
                            log::warn!("Swimmer '{}' not found: {}", name_cell, e);
                            swimmer = Swimmer {
                                id: String::new(),
                                first_name: String::new(),
                                last_name: String::new(),
                                gender: String::new(),
                                birth_date: NaiveDate::MIN,
                            };
                            valid_swimmer = false;
                            break;
                        }
                    };
                    continue;
                }

                if !valid_swimmer || name_row || !valid_row {
                    break;
                }

                let value = cell.inner_html();

                match cell_idx {
                    0 => { // the first column
                        if re_time.is_match(&value) {
                            let result_time = &value[..8];
                            swimmer_time.time = time_to_miliseconds(result_time);

                            if value.ends_with('L') {
                                swimmer_time.course = "LONG".to_string();
                            }

                            if value.ends_with('S') {
                                swimmer_time.course = "SHORT".to_string();
                            }
                        } else {
                            valid_row = false;
                        }
                    }
                    2 => { // the third column
                        swimmer_time.swimmer.gender =
                            value.split(' ').next().unwrap().to_uppercase();
                        swimmer_time.distance = match value.split(' ').nth(1).unwrap().parse() {
                            Ok(d) => d,
                            Err(e) => {
                                log::error!(
                                    "Error parsing distance of {}: {}",
                                    swimmer_time.swimmer.first_name,
                                    e
                                );
                                valid_row = false;
                                0
                            }
                        };
                        swimmer_time.style =
                            convert_style(value.split(' ').last().unwrap()).to_string();
                    }
                    _ => (),
                }

                cell_idx += 1;
            }

            if valid_swimmer && !name_row && valid_row {
                import_time(&state.as_ref().pool, &swimmer_time).await;
            }
        }
    }

    Redirect::to(format!("/meets/{}/", meet.id)).see_other()
}

/// Converts text in the format mm:ss.ms to miliseconds.
fn time_to_miliseconds(time: &str) -> i32 {
    if time.is_empty() {
        return 0;
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

fn convert_style(style: &str) -> &str {
    match style {
        "Fr" => "FREESTYLE",
        "Free" => "FREESTYLE",
        "Bk" => "BACKSTROKE",
        "Back" => "BACKSTROKE",
        "Br" => "BREASTSTROKE",
        "Breast" => "BREASTSTROKE",
        "FL" => "BUTTERFLY",
        "Fly" => "BUTTERFLY",
        "IM" => "MEDLEY",
        "I.M" => "MEDLEY",
        &_ => "",
    }
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

    let app_state = AppState { pool };
    let data_app_state = web::Data::new(app_state);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(fs::Files::new("/static", "./static").show_files_listing())
            .route("/", web::get().to(home_view))
            .route("/meets", web::get().to(meets_view))
            .route("/meets/new", web::get().to(meets_form_view))
            .route("/meets/new", web::post().to(meets_new))
            .route("/meets/{id}/", web::get().to(meet_view))
            .route("/meets/{id}/entries", web::get().to(meets_entries_form_view))
            .route("/meets/{id}/entries/load", web::post().to(import_meet_entries))
            .route("/meets/{id}/results", web::get().to(meets_results_form_view))
            .route("/meets/{id}/results/load", web::post().to(import_meet_results))
            .route("/swimmers", web::get().to(swimmers_view))
            .app_data(data_app_state.clone())
    })
    .bind(("0.0.0.0", server_port))?
    .run()
    .await
}
