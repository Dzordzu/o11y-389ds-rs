// use crate::haproxy;
// use crate::AppState;
// use actix_web::{App, HttpServer, get, post, web};
//
// pub type Data = web::Data<AppState>;
//
// #[post("/mark/drain")]
// /// Set zero connections
// async fn drain() -> &'static str {
//     "Hello, world!"
// }
//
// #[post("/mark/stop")]
// /// Set zero connections
// async fn stop() -> &'static str {
//     "Hello, world!"
// }
//
// #[post("/mark/maintenance")]
// /// Set zero connections
// async fn maint() -> &'static str {
//     "Hello, world!"
// }
//
// #[post("/mark/ready")]
// /// Set server as up and ready. Removed /stop, /maintenance and /drain
// async fn ready() -> &'static str {
//     "Hello, world!"
// }
//
// #[get("/status/drained")]
// /// Check if server is already drained
// async fn is_drained() -> &'static str {
//     "Hello, world!"
// }
//
// #[get("/")]
// async fn index(data: web::Data<AppState>) -> String {
//     if data.lock().await.first_run {
//         haproxy::Response::new_down().to_haproxy_string()
//     } else {
//         data.lock().await.health.to_haproxy_string()
//     }
// }
//
// pub async fn webserver(addr: String, port: u16, app_state: AppState) {
//     HttpServer::new(move || {
//         let app_state = app_state.clone();
//         App::new()
//             .service(index)
//             .service(drain)
//             .service(is_drained)
//             .service(ready)
//             .service(stop)
//             .service(maint)
//             .app_data(web::Data::new(app_state))
//     })
//     .disable_signals()
//     .bind((addr, port))
//     .unwrap()
//     .run()
//     .await
//     .unwrap()
// }
