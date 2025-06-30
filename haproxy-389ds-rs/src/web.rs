use crate::AppState;
use actix_web::{App, HttpServer, get, post, web};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;

pub type Data = web::Data<AppState>;

#[utoipa::path(
    responses(
        (
            status = 200,
            description = "HAProxy set to drain mode (weight = 0%)",
            body = crate::Health
        )
    )
)]
#[post("/mark/drain")]
/// Set zero connections
async fn drain(data: web::Data<AppState>) -> web::Json<crate::Health> {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = true;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = false;

    web::Json(data.health.clone())
}

#[utoipa::path(
    responses(
        (
            status = 200,
            description = "Node marked as stopped",
            body = crate::Health
        )
    )
)]
#[post("/mark/stop")]
/// Set zero connections
async fn stop(data: web::Data<AppState>) -> web::Json<crate::Health> {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = true;

    web::Json(data.health.clone())
}

#[derive(Deserialize, Serialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct MaintenanceParams {
    #[serde(default)]
    force: bool,
}

#[utoipa::path(
    responses(
        (
            status = 200,
            description = "Node marked as under maintenance", 
            body = crate::Health
        )
    ),
    params(
        MaintenanceParams
    )
)]
#[post("/mark/maintenance")]
/// Put node to maintenance mode
async fn maint(
    params: web::Query<MaintenanceParams>,
    data: web::Data<AppState>,
) -> web::Json<crate::Health> {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = !params.force;
    data.health.disabled.mark_soft_maint = true;
    data.health.disabled.mark_stopped = false;

    web::Json(data.health.clone())
}

#[utoipa::path(
    responses(
        (
            status = 200,
            description = "Unset any marks. Set to ready if was in a maintenance state. Set to up if was ready",
            body = crate::Health

        )
    )
)]
#[post("/mark/ready")]
/// Set server as up and ready. Removed /stop, /maintenance and /drain
async fn ready(data: web::Data<AppState>) -> web::Json<crate::Health> {
    let mut data = data.lock().await;
    data.health.disabled.mark_drain = false;
    data.health.disabled.mark_hard_maint = false;
    data.health.disabled.mark_soft_maint = false;
    data.health.disabled.mark_stopped = false;

    web::Json(data.health.clone())
}

#[utoipa::path(
    responses(
        (
            status = 200,
            description = "Get health details of the HAProxy 389ds agent",
            body = crate::Health
        )
    )
)]
#[get("/health-status")]
/// Check if server is already drained
async fn get_status(data: web::Data<AppState>) -> web::Json<crate::Health> {
    let data = data.lock().await;
    web::Json(data.health.clone())
}

#[derive(Deserialize, Serialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct IndexParams {
    /// Do not evaluate new state, just return the current one
    #[serde(default)]
    skip_evaluation: bool,
}

#[utoipa::path(
    responses(
        (status = 200, description = "Evaluate new state and return haproxy reponse")
    ),
    params(
        IndexParams
    )
)]
#[get("/")]
/// Evaluate new state and return haproxy reponse
async fn index(query: web::Query<IndexParams>, data: web::Data<AppState>) -> String {
    let mut data = data.lock().await;
    if !query.skip_evaluation {
        data.evaluate();
    }
    data.current_reponse.to_haproxy_string()
}

pub async fn webserver(addr: String, port: u16, app_state: AppState) {
    HttpServer::new(move || {
        let app_state = app_state.clone();
        App::new()
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .openapi_service(|api| {
                utoipa_swagger_ui::SwaggerUi::new("/swagger/{_:.*}")
                    .url("/api-docs/openapi.json", api)
            })
            .service(get_status)
            .service(index)
            .service(drain)
            .service(ready)
            .service(stop)
            .service(maint)
            .app_data(web::Data::new(app_state))
            .into_app()
            .service(web::redirect("/swagger", "/swagger/"))
    })
    .disable_signals()
    .bind((addr, port))
    .unwrap()
    .run()
    .await
    .unwrap()
}

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(index, get_status, drain, ready, stop, maint),
    components(schemas(MaintenanceParams, IndexParams, crate::Health))
)]
struct ApiDoc;
