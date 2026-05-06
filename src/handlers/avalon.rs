use axum::{Router, routing::{get, post}, extract::{State, Path}, Json};
use serde::Deserialize;
use uuid::Uuid;
use std::collections::HashMap;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::db::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::game::avalon::engine::{AvalonGame, PlayerGameView, TeamVote, MissionVote, GamePhase};
use crate::game::avalon::ai::{AIController, GameEventRecord};
use crate::game::avalon::roles::Role;
use crate::ws::manager::RoomEvent;

#[derive(Deserialize, Default)]
struct StartBody {
    #[serde(default)]
    roles: Option<Vec<String>>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/rooms/{id}/avalon/start", post(start_avalon))
        .route("/api/rooms/{id}/avalon/state", get(get_avalon_state))
        .route("/api/rooms/{id}/avalon/select-team", post(select_team))
        .route("/api/rooms/{id}/avalon/team-vote", post(submit_team_vote))
        .route("/api/rooms/{id}/avalon/mission-vote", post(submit_mission_vote))
        .route("/api/rooms/{id}/avalon/end-speaking", post(end_speaking))
        .route("/api/rooms/{id}/avalon/assassinate", post(submit_assassinate))
        .route("/api/rooms/{id}/avalon/confirm-settlement", post(confirm_settlement))
        .route("/api/rooms/{id}/avalon/disconnect", post(mark_disconnected))
}

async fn start_avalon(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    body: Option<Json<StartBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let is_host: bool = sqlx::query_scalar("SELECT is_host FROM room_players WHERE room_id = $1 AND user_id = $2")
        .bind(room_id).bind(auth.user_id).fetch_optional(&state.pool).await?.unwrap_or(false);
    if !is_host { return Err(AppError::BadRequest("Only host can start".into())); }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_players WHERE room_id = $1").bind(room_id).fetch_one(&state.pool).await?;
    if total < 5 || total > 10 { return Err(AppError::BadRequest("Avalon needs 5-10 players".into())); }

    let player_rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT u.id, u.username, COALESCE(u.avatar, '🎮') FROM room_players rp JOIN users u ON rp.user_id = u.id WHERE rp.room_id = $1"
    ).bind(room_id).fetch_all(&state.pool).await?;

    let player_infos: Vec<(Uuid, String, String)> = player_rows.iter().map(|(id, name, avatar)| (*id, name.clone(), avatar.clone())).collect();

    let start_body = body.map(|b| b.0).unwrap_or_default();
    let custom_roles = start_body.roles.and_then(|arr| {
        Some(arr.iter().filter_map(|s| match s.as_str() {
            "Merlin" => Some(Role::Merlin), "Percival" => Some(Role::Percival),
            "LoyalServant" => Some(Role::LoyalServant), "Assassin" => Some(Role::Assassin),
            "Minion" => Some(Role::Minion), "Morgana" => Some(Role::Morgana),
            "Mordred" => Some(Role::Mordred), "Oberon" => Some(Role::Oberon),
            _ => None,
        }).collect::<Vec<Role>>())
    });

    let ai_ids: Vec<Uuid> = player_rows.iter()
        .filter(|(_, _, avatar)| avatar == "AI")
        .map(|(id, _, _)| *id)
        .collect();

    let mut game = AvalonGame::new_with_roles(player_infos.clone(), custom_roles);
    for id in &ai_ids { game.ai_player_ids.insert(*id); }
    game.init_speaking_for_phase();

    let mut ai_ctrl = AIController::new();
    if !ai_ids.is_empty() {
        let pids: Vec<Uuid> = player_rows.iter().map(|(id, _, _)| *id).collect();
        let names: Vec<String> = player_rows.iter().map(|(_, n, _)| n.clone()).collect();
        let roles = game.roles.clone();
        let diffs: Vec<String> = ai_ids.iter().map(|_| "normal".to_string()).collect();
        ai_ctrl.init_players(&pids, &names, &roles, &game, &diffs);
    }

    let mut games = state.avalon_games.lock().await;
    games.insert(room_id, game);
    drop(games);

    let mut ai_controllers = state.ai_controllers.lock().await;
    ai_controllers.insert(room_id, ai_ctrl);
    drop(ai_controllers);

    sqlx::query("UPDATE rooms SET status = 'Playing' WHERE id = $1").bind(room_id).execute(&state.pool).await?;
    sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, '阿瓦隆游戏开始！', true)")
        .bind(room_id).bind(auth.user_id).execute(&state.pool).await?;

    let room = crate::handlers::rooms::get_room_by_id(&state.pool, room_id).await?;
    state.ws_state.broadcast(room_id, RoomEvent::GameStarted { room: room.clone() }).await;

    broadcast_game_state(&state, room_id).await;
    drive_ai_chain(&state, room_id).await;
    start_watchdog(state.clone(), room_id);

    Ok(Json(serde_json::json!({"ok": true, "message": "Game started"})))
}

fn start_watchdog(state: AppState, room_id: Uuid) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let done = {
                let mut games = state.avalon_games.lock().await;
                match games.get_mut(&room_id) {
                    Some(game) => {
                        if game.phase == GamePhase::End { true }
                        else { game.check_timeout(); false }
                    }
                    None => true,
                }
            };
            if done { break; }
            broadcast_game_state(&state, room_id).await;
        }
    });
}

async fn get_avalon_state(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<PlayerGameView>, AppError> {
    let games = state.avalon_games.lock().await;
    let game = games.get(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    Ok(Json(game.get_player_view(auth.user_id)))
}

async fn select_team(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let team_ids: Vec<Uuid> = body["team"].as_array().ok_or(AppError::BadRequest("team required".into()))?
        .iter().filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok())).collect();

    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    game.select_team(auth.user_id, team_ids.clone())?;
    game.advance_team_selection();
    drop(games);

    broadcast_game_state(&state, room_id).await;
    drive_ai_chain(&state, room_id).await;

    Ok(Json(serde_json::json!({"ok": true, "team": team_ids})))
}

async fn submit_team_vote(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vote_str = body["vote"].as_str().unwrap_or("reject");
    let vote = if vote_str == "approve" { TeamVote::Approve } else { TeamVote::Reject };

    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    let (_, state_changed) = game.submit_team_vote(auth.user_id, vote)?;
    drop(games);

    if state_changed { broadcast_game_state(&state, room_id).await; }
    drive_ai_chain(&state, room_id).await;

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn submit_mission_vote(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let vote_str = body["vote"].as_str().unwrap_or("success");
    let vote = if vote_str == "fail" { MissionVote::Fail } else { MissionVote::Success };

    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    let (_, state_changed) = game.submit_mission_vote(auth.user_id, vote)?;
    let mission_team = game.mission_team.clone();
    drop(games);

    if state_changed {
        if let Some(last_result) = get_mission_result(&state, room_id).await {
            record_ai_event(&state, room_id, GameEventRecord { round: 0, event_type: "mission_end".into(), mission_success: last_result, team_members: mission_team, votes: HashMap::new() }).await;
        }
        broadcast_game_state(&state, room_id).await;
    }
    drive_ai_chain(&state, room_id).await;

    Ok(Json(serde_json::json!({"ok": true})))
}

async fn end_speaking(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    let phase_before = game.phase;
    let still_active = game.end_speaking(auth.user_id)?;
    // Proposal phase: don't auto-advance; leader selects team via select-team API
    if !still_active && phase_before != GamePhase::Proposal {
        game.advance_phase();
    }
    drop(games);

    let (speaking_phase, speaker) = {
        let games = state.avalon_games.lock().await;
        let g = match games.get(&room_id) { Some(g) => g, None => return Ok(Json(serde_json::json!({"ok": true}))) };
        (g.speaking.active, g.speaking.queue.get(g.speaking.current_idx).copied())
    };

    if let Some(sid) = speaker {
        state.ws_state.broadcast(room_id, RoomEvent::SpeakStart { player_id: sid, timeout: 90 }).await;
    }
    broadcast_game_state(&state, room_id).await;
    drive_ai_chain(&state, room_id).await;

    Ok(Json(serde_json::json!({"ok": true, "speaking_continues": speaking_phase})))
}

async fn submit_assassinate(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let target_str = body["target"].as_str().ok_or(AppError::BadRequest("target required".into()))?;
    let target_id = Uuid::parse_str(target_str).map_err(|_| AppError::BadRequest("invalid target".into()))?;

    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    let winner = game.assassinate(auth.user_id, target_id)?;
    drop(games);

    broadcast_game_state(&state, room_id).await;

    if let Some(w) = winner {
        let w_str = if w == crate::game::avalon::roles::Alignment::Good { "好人" } else { "坏人" };
        sqlx::query("INSERT INTO chat_messages (room_id, sender_id, content, is_system) VALUES ($1, $2, $3, true)")
            .bind(room_id).bind(auth.user_id).bind(format!("游戏结束！{}阵营获胜", w_str))
            .execute(&state.pool).await.ok();
    }

    Ok(Json(serde_json::json!({"ok": true, "winner": winner.map(|w| if w == crate::game::avalon::roles::Alignment::Good { "good" } else { "evil" })})))
}

async fn drive_ai_chain(state: &AppState, room_id: Uuid) {
    let mut rng = StdRng::from_entropy();
    loop {
        let (phase, next_player, game_snap) = {
            let games = state.avalon_games.lock().await;
            let game = match games.get(&room_id) { Some(g) => g, None => break };
            // Check timeout
            drop(games);
            let mut games = state.avalon_games.lock().await;
            if let Some(game) = games.get_mut(&room_id) { game.check_timeout(); }
            let game = match games.get(&room_id) { Some(g) => g, None => break };
            (game.phase, game.current_player_id(), game.clone())
        };

        let player_id = match next_player {
            Some(id) if !game_snap.is_human(id) => id,
            _ => break,
        };

        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        match phase {
            GamePhase::Proposal => {
                if game_snap.speaking.active && game_snap.speaking.queue.get(0).copied() == Some(player_id) {
                    let mut games = state.avalon_games.lock().await;
                    if let Some(game) = games.get_mut(&room_id) { let _ = game.end_speaking(player_id); }
                } else {
                    let team_size = game_snap.mission_sizes.get(game_snap.round - 1).copied().unwrap_or(2);
                    let all_pids: Vec<Uuid> = game_snap.players.iter().map(|p| p.user_id).collect();
                    let mut games = state.avalon_games.lock().await;
                    if let Some(game) = games.get_mut(&room_id) {
                        let _ = game.select_team(player_id, all_pids.iter().take(team_size).copied().collect());
                        game.advance_team_selection();
                    }
                }
            }
            GamePhase::Discussion => {
                if game_snap.speaking.active && game_snap.speaking.queue.get(game_snap.speaking.current_idx).copied() == Some(player_id) {
                    let mut games = state.avalon_games.lock().await;
                    if let Some(game) = games.get_mut(&room_id) {
                        let _ = game.end_speaking(player_id);
                        if !game.speaking.active { game.advance_phase(); }
                    }
                }
            }
            GamePhase::Vote => {
                let controllers = state.ai_controllers.lock().await;
                let vote = controllers.get(&room_id)
                    .and_then(|ctrl| ctrl.get_player(player_id))
                    .map_or(TeamVote::Approve, |ai| ai.vote_on_team(&game_snap.mission_team, &game_snap, &mut rng));
                drop(controllers);
                let mut games = state.avalon_games.lock().await;
                if let Some(game) = games.get_mut(&room_id) { let _ = game.submit_team_vote(player_id, vote); }
            }
            GamePhase::Mission => {
                let controllers = state.ai_controllers.lock().await;
                let vote = controllers.get(&room_id)
                    .and_then(|ctrl| ctrl.get_player(player_id))
                    .map_or(MissionVote::Success, |ai| ai.choose_mission_action(&mut rng));
                drop(controllers);
                let mut games = state.avalon_games.lock().await;
                if let Some(game) = games.get_mut(&room_id) { let _ = game.submit_mission_vote(player_id, vote); }
            }
            GamePhase::Assassination => {
                let pids: Vec<Uuid> = game_snap.players.iter().map(|p| p.user_id).collect();
                let target = pids.first().copied().unwrap_or_default();
                let mut games = state.avalon_games.lock().await;
                if let Some(game) = games.get_mut(&room_id) { let _ = game.assassinate(player_id, target); }
            }
            _ => break,
        }
        broadcast_game_state(state, room_id).await;
    }
}

async fn confirm_settlement(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let all_confirmed: bool;
    let game_phase: GamePhase;
    {
        let mut games = state.avalon_games.lock().await;
        let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
        all_confirmed = game.confirm_settlement(auth.user_id);
        game_phase = game.phase;
    }
    if all_confirmed {
        let game_snapshot = {
            let games = state.avalon_games.lock().await;
            games.get(&room_id).cloned()
        };
        if let Some(game) = game_snapshot {
            let room_name: String = sqlx::query_scalar("SELECT name FROM rooms WHERE id = $1")
                .bind(room_id).fetch_one(&state.pool).await.unwrap_or_default();
            let winner_str = match game.winner { Some(w) => if w == crate::game::avalon::roles::Alignment::Good { "Good" } else { "Evil" }, None => "Unknown" };
            let players_json = serde_json::to_value(game.players.iter().map(|p| serde_json::json!({
                "user_id": p.user_id, "username": p.username, "role": format!("{:?}", game.roles.get(&p.user_id).unwrap_or(&crate::game::avalon::roles::Role::LoyalServant)),
            })).collect::<Vec<_>>()).unwrap_or_default();
            let history_json = serde_json::to_value(&game.round_history).unwrap_or_default();
            let assassin_hit = game.assassin_target.and_then(|t| game.roles.get(&t)).map_or(false, |r| *r == crate::game::avalon::roles::Role::Merlin);
            let record_id = Uuid::new_v4();
            sqlx::query("INSERT INTO game_records (id, room_id, room_name, game_type, winner, assassin_target, assassin_hit, rounds_played, mission_results, players, round_history) VALUES ($1, $2, $3, 'avalon', $4, $5, $6, $7, $8, $9, $10)")
                .bind(record_id).bind(room_id).bind(&room_name).bind(winner_str).bind(game.assassin_target).bind(assassin_hit)
                .bind(game.round_history.len() as i32).bind(serde_json::to_value(&game.mission_results).unwrap_or_default())
                .bind(&players_json).bind(&history_json).execute(&state.pool).await.ok();
            for p in &game.players {
                let role = game.roles.get(&p.user_id).copied().unwrap_or(crate::game::avalon::roles::Role::LoyalServant);
                let alignment = role.alignment();
                let player_won = match (alignment, game.winner) {
                    (crate::game::avalon::roles::Alignment::Good, Some(crate::game::avalon::roles::Alignment::Good)) => true,
                    (crate::game::avalon::roles::Alignment::Evil, Some(crate::game::avalon::roles::Alignment::Evil)) => true,
                    _ => false,
                };
                sqlx::query("INSERT INTO game_record_players (record_id, user_id, role, alignment, won) VALUES ($1, $2, $3, $4, $5)")
                    .bind(record_id).bind(p.user_id).bind(format!("{:?}", role)).bind(format!("{:?}", alignment)).bind(player_won).execute(&state.pool).await.ok();
                sqlx::query("UPDATE users SET total_games = total_games + 1, win_rate = CASE WHEN total_games + 1 = 1 THEN CASE WHEN $2 THEN 1.0 ELSE 0.0 END ELSE (win_rate * total_games + CASE WHEN $2 THEN 1.0 ELSE 0.0 END) / (total_games + 1.0) END WHERE id = $1")
                    .bind(p.user_id).bind(player_won).execute(&state.pool).await.ok();
            }
        }
        sqlx::query("UPDATE rooms SET status = 'Waiting' WHERE id = $1").bind(room_id).execute(&state.pool).await.ok();
        state.avalon_games.lock().await.remove(&room_id);
        state.ai_controllers.lock().await.remove(&room_id);
    } else {
        broadcast_game_state(&state, room_id).await;
    }
    Ok(Json(serde_json::json!({"ok": true, "all_confirmed": all_confirmed, "phase": format!("{:?}", game_phase)})))
}

async fn mark_disconnected(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut games = state.avalon_games.lock().await;
    let game = games.get_mut(&room_id).ok_or(AppError::NotFound("No active game".into()))?;
    game.mark_disconnected(auth.user_id);
    broadcast_game_state(&state, room_id).await;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn get_mission_result(state: &AppState, room_id: Uuid) -> Option<Option<bool>> {
    let games = state.avalon_games.lock().await;
    let game = games.get(&room_id)?;
    game.mission_results.iter().find(|r| r.is_some()).map(|r| *r)
}

async fn record_ai_event(state: &AppState, room_id: Uuid, event: GameEventRecord) {
    let mut controllers = state.ai_controllers.lock().await;
    if let Some(ctrl) = controllers.get_mut(&room_id) { ctrl.record_event(event); }
}

async fn broadcast_game_state(state: &AppState, room_id: Uuid) {
    let games = state.avalon_games.lock().await;
    let game = match games.get(&room_id) { Some(g) => g, None => return };
    let mut views: HashMap<Uuid, PlayerGameView> = HashMap::new();
    for player in &game.players {
        views.insert(player.user_id, game.get_player_view(player.user_id));
    }
    drop(games);
    state.ws_state.broadcast_avalon(room_id, views).await;
}
