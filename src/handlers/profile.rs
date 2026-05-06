use axum::{Router, routing::get, extract::{State, Path}, Json};
use uuid::Uuid;

use crate::db::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::models::game_record::{GameRecord, ProfileStats, GameRecordRow};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/profile/records", get(get_my_records))
        .route("/api/profile/records/{id}", get(get_record_detail))
        .route("/api/profile/stats", get(get_profile_stats))
}

async fn get_my_records(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<Vec<GameRecord>>, AppError> {
    let rows = sqlx::query_as::<_, GameRecordRow>(
        r#"SELECT gr.* FROM game_records gr
           JOIN game_record_players grp ON gr.id = grp.record_id
           WHERE grp.user_id = $1
           ORDER BY gr.created_at DESC LIMIT 20"#
    ).bind(auth.user_id).fetch_all(&state.pool).await?;

    let mut records = Vec::new();
    for row in rows {
        let role_row: Option<(String, bool)> = sqlx::query_as(
            "SELECT role, won FROM game_record_players WHERE record_id = $1 AND user_id = $2"
        ).bind(row.id).bind(auth.user_id).fetch_optional(&state.pool).await?;

        records.push(GameRecord {
            id: row.id,
            room_name: row.room_name,
            game_type: row.game_type,
            winner: row.winner,
            assassin_target: row.assassin_target,
            assassin_hit: row.assassin_hit,
            rounds_played: row.rounds_played,
            mission_results: row.mission_results,
            players: row.players,
            round_history: row.round_history,
            created_at: row.created_at.format("%Y-%m-%d %H:%M").to_string(),
            your_role: role_row.as_ref().map(|(r, _)| r.clone()),
            you_won: role_row.map(|(_, w)| w),
        });
    }
    Ok(Json(records))
}

async fn get_record_detail(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(record_id): Path<Uuid>,
) -> Result<Json<GameRecord>, AppError> {
    let row = sqlx::query_as::<_, GameRecordRow>(
        "SELECT * FROM game_records WHERE id = $1"
    ).bind(record_id).fetch_optional(&state.pool).await?
        .ok_or(AppError::NotFound("Record not found".into()))?;

    Ok(Json(GameRecord {
        id: row.id,
        room_name: row.room_name,
        game_type: row.game_type,
        winner: row.winner,
        assassin_target: row.assassin_target,
        assassin_hit: row.assassin_hit,
        rounds_played: row.rounds_played,
        mission_results: row.mission_results,
        players: row.players,
        round_history: row.round_history,
        created_at: row.created_at.format("%Y-%m-%d %H:%M").to_string(),
        your_role: None,
        you_won: None,
    }))
}

async fn get_profile_stats(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<ProfileStats>, AppError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_record_players WHERE user_id = $1"
    ).bind(auth.user_id).fetch_one(&state.pool).await?;

    let wins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_record_players WHERE user_id = $1 AND won = true"
    ).bind(auth.user_id).fetch_one(&state.pool).await?;

    let fav_role: Option<String> = sqlx::query_scalar(
        r#"SELECT role FROM game_record_players WHERE user_id = $1
           GROUP BY role ORDER BY COUNT(*) DESC LIMIT 1"#
    ).bind(auth.user_id).fetch_optional(&state.pool).await?;

    Ok(Json(ProfileStats {
        total_games: total,
        wins,
        win_rate: if total > 0 { wins as f64 / total as f64 } else { 0.0 },
        favorite_role: fav_role,
        recent_records: vec![],
    }))
}
