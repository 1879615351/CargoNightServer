use axum::{Router, routing::get, extract::State, Json};

use crate::models::game::Game;
use crate::models::room::HomeStats;
use crate::db::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/home/stats", get(get_home_stats))
}

async fn get_home_stats(
    State(state): State<AppState>,
) -> Result<Json<HomeStats>, crate::error::AppError> {
    let games = sqlx::query_as::<_, Game>("SELECT * FROM games ORDER BY online_count DESC")
        .fetch_all(&state.pool)
        .await?;

    let active_rooms = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rooms WHERE status != 'Finished'"
    ).fetch_one(&state.pool).await?;

    let games_in_play = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rooms WHERE status = 'Playing'"
    ).fetch_one(&state.pool).await?;

    Ok(Json(HomeStats {
        online_players: 1523,
        active_rooms: active_rooms as i32,
        games_in_play: games_in_play as i32,
        hot_games: games.clone(),
        hot_rooms: vec![],
    }))
}
