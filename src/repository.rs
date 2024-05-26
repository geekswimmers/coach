use crate::model::{ImportHistory, Meet, Swimmer};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::Row;

pub async fn find_meet(conn: &PgPool, meet_id: &str) -> Meet {
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

pub async fn find_meets_with_results(conn: &PgPool, except: &str) -> Vec<Meet> {
    sqlx::query(
        "
        select m.id, m.name
        from meet m
	        left join import_history ih on m.id = ih.meet
        where m.id <> $1
            and ih.dataset = 'MEET_RESULTS'
    ",
    )
    .bind(except)
    .map(|row: PgRow| Meet {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
    })
    .fetch_all(conn)
    .await
    .expect("Failed to fetch meets with entries")
}

pub async fn search_swimmer_by_name(conn: &PgPool, name: String) -> Result<Swimmer, sqlx::Error> {
    let first_name = name.split(' ').next();
    let last_name = name.split(' ').nth(1);

    sqlx::query(
        "
        select id, first_name, last_name, gender, birth_date
        from swimmer
        where first_name = $1 and last_name = $2
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

pub async fn find_import_history(conn: &PgPool, meet_id: &str) -> Vec<ImportHistory> {
    sqlx::query(
        "
        select id, load_time, num_swimmers, num_entries, duration, swimmers, meet, dataset
        from import_history
        where meet = $1
        order by load_time desc
    ",
    )
    .bind(meet_id)
    .map(|row: PgRow| ImportHistory {
        id: row.get("id"),
        load_time: row.get("load_time"),
        num_swimmers: row.get("num_swimmers"),
        num_entries: row.get("num_entries"),
        duration: row.get("duration"),
        swimmers: row.get("swimmers"),
        meet: row.get("meet"),
        dataset: row.get("dataset"),
    })
    .fetch_all(conn)
    .await
    .expect("Error finding import history")
}
