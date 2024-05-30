use crate::model::AppState;
use crate::repository::{find_latest_imported_swimmers, find_meet, find_meets_with_results};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use tera::Context;

#[derive(Deserialize)]
pub struct MeetPath {
    pub id: String,
}

pub async fn home_view(state: web::Data<AppState>) -> impl Responder {
    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            state
                .get_ref()
                .template
                .render("index.html", &context)
                .unwrap(),
        )
}

pub async fn meet_view(path: web::Path<MeetPath>, state: web::Data<AppState>) -> impl Responder {
    let meet = find_meet(&state.get_ref().pool, &path.id).await;
    let meets_with_results = find_meets_with_results(&state.get_ref().pool, &meet.id).await;

    let import_history = find_latest_imported_swimmers(&state.get_ref().pool, &meet.id).await;
    let entries_loaded = import_history
        .iter()
        .filter(|i| i.dataset == "MEET_ENTRIES")
        .count()
        > 0;
    let results_loaded = import_history
        .iter()
        .filter(|i| i.dataset == "MEET_RESULTS")
        .count()
        > 0;

    let _entries_swimmers = import_history
        .iter()
        .find(|i| i.dataset == "MEET_ENTRIES")
        .expect("No entries swimmers");
    let _results_swimmers = import_history
        .iter()
        .find(|i| i.dataset == "MEET_RESULTS")
        .expect("No result swimmers");

    let mut context = Context::new();
    context.insert("meet", &meet);
    context.insert("meets_with_results", &meets_with_results);
    context.insert("entries_loaded", &entries_loaded);
    context.insert("results_loaded", &results_loaded);

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(state.get_ref().template.render("meet.html", &context).unwrap())
}

pub async fn meets_form_view(state: web::Data<AppState>) -> impl Responder {
    let context = Context::new();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            state
                .get_ref()
                .template
                .render("meet_form.html", &context)
                .unwrap(),
        )
}
