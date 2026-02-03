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
            // Trigger the automated timer AFTER a move is successfully made
            spawn_timer(
                state.clone(),
                id.clone(),
                game.phase.clone(),
                game.current_player_idx,
            );

            (StatusCode::OK, "Move accepted").into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

// --- HELPER: RECURSIVE TIMER ---
fn spawn_timer(state: AppState, game_id: String, phase: GamePhase, player_idx: usize) {
    let timeout_seconds = match phase {
        GamePhase::ExplosionPending { timer_seconds } => timer_seconds as u64,
        GamePhase::PlayerTurn => 45,
        _ => return, // No timer for other phases
    };

    let target_phase_variant = std::mem::discriminant(&phase);

    tokio::spawn(async move {
        // 1. Wait patiently
        sleep(Duration::from_secs(timeout_seconds)).await;

        let mut games = state.games.lock().unwrap();
        if let Some(bg_game) = games.get_mut(&game_id) {
            let current_phase_variant = std::mem::discriminant(&bg_game.phase);

            // 2. Validate nothing changed while we slept
            if current_phase_variant == target_phase_variant
                && bg_game.current_player_idx == player_idx
            {
                println!(
                    "⏰ Timeout! Auto-playing for player {} in game {}",
                    player_idx, game_id
                );

                // 3. EXECUTE TIMEOUT
                // Using "system" as actor bypasses basic ID checks
                if let Ok(_) = bg_game.transition(GameEvent::TimerExpired, "system") {
                    // 4. CHAIN REACTION
                    // Spawn a NEW timer for the resulting state (infinite auto-play)
                    spawn_timer(
                        state.clone(),
                        game_id.clone(),
                        bg_game.phase.clone(),
                        bg_game.current_player_idx,
                    );
                }
            }
        }
    });
}
