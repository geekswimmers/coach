use actix_web::{HttpResponse, Responder, web};
use tera::{Context};
use crate::model::AppState;

pub async fn home_view(state: web::Data<AppState>) -> impl Responder {
    let context = Context::new();

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(state.get_ref().template.render("index.html", &context).unwrap())
}

pub async fn meets_form_view(state: web::Data<AppState>) -> impl Responder {
    let context = Context::new();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(state.get_ref().template.render("meet_form.html", &context).unwrap())
}