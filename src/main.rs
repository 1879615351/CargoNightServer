mod config;
mod db;
mod error;
mod models;
mod handlers;
mod ws;
mod middleware;
mod game;

use axum::Router;
use tower_http::cors::{CorsLayer, Any};
use tracing_subscriber::EnvFilter;
use db::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let conf = config::Config::from_env();
    let app_state = db::create_app_state(&conf.database_url, conf.clone()).await;
    sqlx::migrate!("./migrations").run(&app_state.pool).await.expect("Migration failed");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(handlers::auth::routes())
        .merge(handlers::home::routes())
        .merge(handlers::games::routes())
        .merge(handlers::rooms::routes())
        .merge(handlers::chat::routes())
        .merge(handlers::avalon::routes())
        .merge(handlers::ai::routes())
        .merge(handlers::profile::routes())
        .merge(handlers::friends::routes())
        .merge(ws::handler::routes())
        .layer(cors)
        .with_state(app_state);

    tracing::info!("CargoNight Server running on {}:{}", conf.server_host, conf.server_port);

    let addr = format!("{}:{}", conf.server_host, conf.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
