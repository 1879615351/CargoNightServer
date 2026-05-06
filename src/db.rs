use sqlx::postgres::PgPool;
use sqlx::postgres::PgPoolOptions;
use crate::config::Config;
use crate::ws::manager::WsState;
use crate::game::avalon::engine::AvalonGame;
use crate::game::avalon::ai::AIController;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UserOnlineInfo {
    pub user_id: Uuid,
    pub username: String,
    pub avatar: String,
    pub status: String,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub game_name: Option<String>,
    pub game_mode: Option<String>,
    pub player_count: Option<i32>,
    pub max_players: Option<i32>,
    pub has_password: Option<bool>,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub ws_state: WsState,
    pub avalon_games: Arc<Mutex<HashMap<Uuid, AvalonGame>>>,
    pub ai_controllers: Arc<Mutex<HashMap<Uuid, AIController>>>,
    pub online_users: Arc<Mutex<HashMap<Uuid, UserOnlineInfo>>>,
}

pub async fn create_app_state(database_url: &str, config: Config) -> AppState {
    let pool = PgPoolOptions::new().max_connections(20).connect(database_url).await.expect("Failed to connect to PostgreSQL");
    let ws_state = WsState::new();
    let avalon_games = Arc::new(Mutex::new(HashMap::new()));
    let ai_controllers = Arc::new(Mutex::new(HashMap::new()));
    let online_users = Arc::new(Mutex::new(HashMap::new()));
    AppState { pool, config, ws_state, avalon_games, ai_controllers, online_users }
}
