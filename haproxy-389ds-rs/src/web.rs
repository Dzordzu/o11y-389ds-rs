use crate::AppState;
use actix_web::{App, HttpServer, get, post, web};
use serde::{Deserialize, Serialize};

pub type Data = web::Data<AppState>;

#[post("/mark/drain")]
/// Set zero connections
async fn drain(data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = true;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = false;

    serde_json::to_string(&data.health).unwrap().to_string()
}

#[post("/mark/stop")]
/// Set zero connections
async fn stop(data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = true;

    serde_json::to_string(&data.health).unwrap().to_string()
}

#[derive(Deserialize, Serialize)]
pub struct MaintenanceParams {
    #[serde(default)]
    force: bool,
}

#[post("/mark/maintenance")]
/// Set zero connections
async fn maint(params: web::Query<MaintenanceParams>, data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = !params.force;
    data.health.disabled.mark_soft_maint = true;
    data.health.disabled.mark_stopped = false;

    serde_json::to_string(&data.health).unwrap().to_string()
}

#[post("/mark/ready")]
/// Set server as up and ready. Removed /stop, /maintenance and /drain
async fn ready(data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = false;

    serde_json::to_string(&data.health).unwrap().to_string()
}

#[get("/health-status")]
/// Check if server is already drained
async fn get_status(data: web::Data<AppState>) -> String {
    let data = data.lock().await;

    serde_json::to_string(&data.health).unwrap().to_string()
}

#[derive(Deserialize, Serialize)]
pub struct IndexParams {
    #[serde(default)]
    skip_evaluation: bool,
}

#[get("/")]
/// Evaluate new state and return haproxy reponse
async fn index(params: web::Query<IndexParams>, data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    if !params.skip_evaluation {
        data.evaluate();
    }
    data.current_reponse.to_haproxy_string()
}

pub async fn webserver(addr: String, port: u16, app_state: AppState) {
    HttpServer::new(move || {
        let app_state = app_state.clone();
        App::new()
            .service(get_status)
            .service(index)
            .service(drain)
            .service(ready)
            .service(stop)
            .service(maint)
            .app_data(web::Data::new(app_state))
    })
    .disable_signals()
    .bind((addr, port))
    .unwrap()
    .run()
    .await
    .unwrap()
}
