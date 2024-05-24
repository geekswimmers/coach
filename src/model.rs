use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub struct Swimmer {
    pub id: String,
    pub first_name: String,
    pub last_name: String,
    pub gender: String,
    pub birth_date: NaiveDate,
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
pub struct SwimmerTime {
    pub swimmer: Swimmer,
    pub style: String,
    pub distance: i32,
    pub course: String,
    pub time: i32,
    pub time_date: NaiveDate,
    pub meet: Meet,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Meet {
    pub id: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
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

#[derive(Serialize, Deserialize)]
pub struct ImportHistory {
    pub id: i32,
    pub load_time: NaiveDateTime,
    pub num_swimmers: i32,
    pub num_entries: i32,
    pub duration: i32,
    pub swimmers: String,
    pub meet: String,
    pub dataset: String,
}
