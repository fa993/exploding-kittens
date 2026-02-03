use std::time::Duration;

use crate::AppState;
use crate::game::engine::{GameContext, GameEvent, GamePhase};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use uuid::Uuid;

// ============================================================================
// DTOs (Data Transfer Objects)
// ============================================================================

#[derive(Deserialize)]
pub struct JoinRequest {
    pub player_name: String,
}

#[derive(Deserialize)]
pub struct MoveRequest {
    pub player_id: String, // <--- ADD THIS
    pub action: GameEvent,
}

#[derive(Deserialize)]
pub struct PollQuery {
    pub player_id: String,
}

#[derive(Serialize)]
pub struct CreateResponse {
    pub game_id: String,
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub player_id: String,
    pub message: String,
}

// ============================================================================
// ROUTER
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/games", post(create_game))
        .route("/games/:id/join", post(join_game))
        .route("/games/:id/start", post(start_game))
        .route("/games/:id", get(get_game_state)) // Polling Endpoint
        .route("/games/:id/move", post(play_move))
}

// ============================================================================
// HANDLERS
// ============================================================================

/// POST /games
/// Creates a new game lobby
async fn create_game(State(state): State<AppState>) -> impl IntoResponse {
    let mut games = state.games.lock().unwrap(); // Lock the map

    let game_id = Uuid::new_v4().to_string();
    let game = GameContext::new();

    games.insert(game_id.clone(), game);

    println!("Created game: {}", game_id);
    (StatusCode::CREATED, Json(CreateResponse { game_id }))
}

/// POST /games/:id/join
/// Request: { "player_name": "Alice" }
async fn join_game(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<JoinRequest>,
) -> impl IntoResponse {
    let mut games = state.games.lock().unwrap();

    if let Some(game) = games.get_mut(&id) {
        let player_id = Uuid::new_v4().to_string();

        match game.add_player(player_id.clone(), payload.player_name) {
            Ok(_) => (
                StatusCode::OK,
                Json(JoinResponse {
                    player_id,
                    message: "Joined successfully".into(),
                }),
            )
                .into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Game ID not found").into_response()
    }
}

/// POST /games/:id/start
/// Only works if 2+ players have joined
async fn start_game(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let mut games = state.games.lock().unwrap();

    if let Some(game) = games.get_mut(&id) {
        match game.transition(GameEvent::StartGame, format!("system").as_str()) {
            Ok(_) => (StatusCode::OK, "Game Started").into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Game ID not found").into_response()
    }
}

/// GET /games/:id?player_id=...
/// Returns the sanitized state for the specific player (hides opponent hands)
async fn get_game_state(
    Path(id): Path<String>,
    Query(params): Query<PollQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let games = state.games.lock().unwrap();

    if let Some(game) = games.get(&id) {
        let view = game.get_view_for_player(&params.player_id);
        // Axum serializes the GameView struct automatically
        (StatusCode::OK, Json(view)).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Game ID not found").into_response()
    }
}

/// POST /games/:id/move
async fn play_move(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<MoveRequest>,
) -> impl IntoResponse {
    let mut games = state.games.lock().unwrap();

    let game = match games.get_mut(&id) {
        Some(g) => g,
        None => return (StatusCode::NOT_FOUND, "Game not found").into_response(),
    };

    match game.transition(payload.action, &payload.player_id) {
        Ok(_) => {
            // =================================================================
            // ⏰ THE "GOD MODE" TIMER HOOK
            // =================================================================
            // We spawn a background task that watches the current player.
            // If they are still the active player after X seconds, we force an event.
            // =================================================================

            let game_id = id.clone();
            let state_clone = state.clone();

            // Snapshot the state *after* the move finished
            let target_phase_variant = std::mem::discriminant(&game.phase);
            let target_player_idx = game.current_player_idx;

            // Decide how long to wait based on phase
            let timeout_seconds = match game.phase {
                GamePhase::ExplosionPending { timer_seconds } => timer_seconds as u64,
                GamePhase::PlayerTurn => 45, // Give them 45s for a normal turn
                _ => 0,
            };

            if timeout_seconds > 0 {
                tokio::spawn(async move {
                    // 1. Wait patiently
                    sleep(Duration::from_secs(timeout_seconds)).await;

                    // 2. Wake up and check the game state
                    let mut games = state_clone.games.lock().unwrap();
                    if let Some(bg_game) = games.get_mut(&game_id) {
                        // 3. VALIDATION: Is it *still* the same situation?
                        // We check if the phase type is the same AND the active player is the same.
                        let current_phase_variant = std::mem::discriminant(&bg_game.phase);

                        if current_phase_variant == target_phase_variant
                            && bg_game.current_player_idx == target_player_idx
                        {
                            println!(
                                "⏰ Timeout! Auto-playing for player {} in game {}",
                                target_player_idx, game_id
                            );

                            // 4. Force the move (Using "system" as actor_id allows it to bypass checks if needed,
                            //    though our engine validates logic, not just ID for TimerExpired)
                            let _ = bg_game.transition(GameEvent::TimerExpired, "system");
                        }
                    }
                });
            }
            // =================================================================

            (StatusCode::OK, "Move accepted").into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}
