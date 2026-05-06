use axum::{Router, routing::get, extract::State, Json};

use crate::models::game::Game;
use crate::db::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/games", get(get_game_list))
}

async fn get_game_list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Game>>, crate::error::AppError> {
    let games = sqlx::query_as::<_, Game>("SELECT * FROM games ORDER BY online_count DESC")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(games))
}
