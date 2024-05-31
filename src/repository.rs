use crate::model::{ImportHistory, Meet, Swimmer, SwimmerTime};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::Row;

pub async fn find_all_meets(conn: &PgPool) -> Vec<Meet> {
    sqlx::query(
        "
            select id, name, start_date, end_date, course
            from meet
            order by end_date desc
        ",
    )
    .map(|row: PgRow| Meet {
        id: row.get("id"),
        name: row.get("name"),
        start_date: row.get("start_date"),
        end_date: row.get("end_date"),
        course: row.get("course"),
    })
    .fetch_all(conn)
    .await
    .expect("Failed to fetch meets")
}

pub async fn find_meet(conn: &PgPool, meet_id: &str) -> Meet {
    sqlx::query(
        "
            select id, name, start_date, end_date, course
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
        course: row.get("course"),
    })
    .fetch_one(conn)
    .await
    .expect("Failed to fetch meet")
}

pub async fn find_meets_with_results(conn: &PgPool, except: &str) -> Vec<Meet> {
    sqlx::query(
        "
            select m.id, m.name, m.start_date, m.end_date, m.course
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
        course: row.get("course"),
    })
    .fetch_all(conn)
    .await
    .expect("Failed to fetch meets with entries")
}

pub async fn find_all_swimmers(conn: &PgPool) -> Vec<Swimmer> {
    sqlx::query(
        "
            select id, first_name, last_name, gender, birth_date
            from swimmer
            order by first_name, last_name
        ",
    )
    .map(|row: PgRow| Swimmer {
        id: row.get("id"),
        first_name: row.get("first_name"),
        last_name: row.get("last_name"),
        gender: row.get("gender"),
        birth_date: row.get("birth_date"),
    })
    .fetch_all(conn)
    .await
    .expect("Failed to fetch swimmers")
}

pub async fn find_meet_swimmers(conn: &PgPool, import_history: &ImportHistory) -> Vec<SwimmerTime> {
    let swimmers = import_history
        .swimmers
        .split(',')
        .fold("''".to_string(), |acc, s| format!("{},'{}'", acc, s.trim()));

    let sql = format!("
            select s.id, s.first_name, s.last_name,
                   st.style, st.distance, st.official_time, st.date_time
            from swimmer_time st
                join swimmer s on s.id = st.swimmer
            where st.meet = $1
                and st.dataset = $2
                and st.course = $3
                and st.swimmer in ({swimmers})
            order by s.first_name, s.last_name, st.style, st.distance, st.official_time
        ");

    sqlx::query(sql.as_str())
    .bind(&import_history.meet.id)
    .bind(&import_history.dataset)
    .bind(&import_history.meet.course)
    .map(|row: PgRow| SwimmerTime {
        swimmer: Swimmer::new(
            row.get("id"),
            row.get("first_name"),
            row.get("last_name")
        ),
        style: row.get("style"),
        distance: row.get("distance"),
        course: import_history.meet.course.clone(),
        time: row.get("official_time"),
        time_date: row.get("date_time"),
        meet: import_history.meet.clone(),
        dataset: import_history.dataset.clone(),
    })
    .fetch_all(conn)
    .await
    .expect("Failed to fetch meet entry swimmers")
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
            select ih.id, ih.load_time, ih.num_swimmers, ih.num_entries, ih.duration, ih.swimmers, ih.meet, ih.course, ih.dataset
            from import_history ih
                join meet m on m.id = ih.meet
            where ih.meet = $1
            order by ih.load_time desc
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
        meet: Meet::new(row.get("meet"), row.get("course")),
        dataset: row.get("dataset"),
    })
    .fetch_all(conn)
    .await
    .expect("Error finding import history")
}

pub async fn find_latest_imported_swimmers(conn: &PgPool, meet_id: &str) -> Vec<ImportHistory> {
    sqlx::query(
        "
            select ih.id, ih.load_time, ih.num_swimmers, ih.num_entries, ih.duration, ih.swimmers, ih.meet, m.course, ih.dataset
            from import_history ih
                join meet m on m.id = ih.meet
            where ih.meet = $1
        	    and ih.dataset = 'MEET_ENTRIES'
        	    and ih.load_time >= (select max(load_time) from import_history where meet = $1 and dataset = 'MEET_ENTRIES')
            union
            select ih.id, ih.load_time, ih.num_swimmers, ih.num_entries, ih.duration, ih.swimmers, ih.meet, m.course, ih.dataset
            from import_history ih
                join meet m on m.id = ih.meet
            where ih.meet = $1
        	    and ih.dataset = 'MEET_RESULTS'
        	    and ih.load_time >= (select max(load_time) from import_history where meet = $1 and dataset = 'MEET_RESULTS')
        ",
    )
    .bind(meet_id)
    .map(|row| ImportHistory {
        id: row.get("id"),
        load_time: row.get("load_time"),
        num_swimmers: row.get("num_swimmers"),
        num_entries: row.get("num_entries"),
        duration: row.get("duration"),
        swimmers: row.get("swimmers"),
        meet: Meet::new(row.get("meet"), row.get("course")),
        dataset: row.get("dataset"),
    })
    .fetch_all(conn)
    .await
    .expect("Error finding imported swimmers")
}
