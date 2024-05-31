use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tera::Tera;

pub struct AppState {
    pub pool: PgPool,
    pub template: Tera,
}

#[derive(Serialize, Clone)]
pub struct Swimmer {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub gender: String,
    pub birth_date: NaiveDate,
}

impl Swimmer {
    pub fn new(id: String, first_name: String, last_name: String) -> Self {
        Self {
            id,
            first_name,
            last_name,
            gender: String::new(),
            birth_date: NaiveDate::MIN,
        }
    }
}

#[derive(Serialize)]
pub struct SwimmerTime {
    pub swimmer: Swimmer,
    pub style: String,
    pub distance: i32,
    pub course: String,
    pub time: i32,
    pub time_date: NaiveDate,
    pub meet: Meet,
    pub dataset: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Meet {
    pub id: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub course: String,
}

impl Meet {
    pub fn new(id: String, course: String) -> Self {
        Self {
            id,
            name: String::new(),
            start_date: NaiveDate::MIN,
            end_date: NaiveDate::MAX,
            course,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ImportHistory {
    pub id: i32,
    pub load_time: NaiveDateTime,
    pub num_swimmers: i32,
    pub num_entries: i32,
    pub duration: i32,
    pub swimmers: String,
    pub meet: Meet,
    pub dataset: String,
}
