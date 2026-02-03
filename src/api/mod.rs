use crate::AppState;
use crate::game::engine::{GameContext, GameEvent};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
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
        match game.transition(GameEvent::StartGame, format!("-1").as_str()) {
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
        // Use the engine method we wrote earlier to sanitize data
        let view = game.get_view_for_player(&params.player_id);
        (StatusCode::OK, Json(view)).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Game ID not found").into_response()
    }
}

/// POST /games/:id/move
async fn play_move(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<MoveRequest>, // Payload now includes player_id
) -> impl IntoResponse {
    let mut games = state.games.lock().unwrap();

    if let Some(game) = games.get_mut(&id) {
        // PASS THE ID HERE
        match game.transition(payload.action, &payload.player_id) {
            Ok(_) => (StatusCode::OK, "Move accepted").into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Game ID not found").into_response()
    }
}
