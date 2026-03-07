mod api;
mod game;

use crate::game::engine::GameContext;
use axum::Router;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile}; // Import these

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<Mutex<HashMap<String, GameContext>>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Catch the same variable name used by Vite
    let base_path = option_env!("VITE_APP_BASE_PATH").unwrap_or("/");

    println!("Mounted at {base_path}");

    let state = AppState {
        games: Arc::new(Mutex::new(HashMap::new())),
    };

    // 1. Define the Static File Service
    // Serve files from "dist" directory.
    // If a file is not found (e.g. /game/uuid), serve index.html (SPA Fallback)
    let serve_dir = ServeDir::new("dist").not_found_service(ServeFile::new("dist/index.html"));

    let router = if base_path != "/" {
        Router::new().nest(
            base_path,
            // API Routes (prefixed with /api/games inside api.rs if you prefer, or mapped here)
            // Note: In api.rs, our routes are /games...
            // We will mount api::router() directly.
            Router::new()
                .merge(api::router())
                .fallback_service(serve_dir),
        )
    } else {
        Router::new()
            // API Routes (prefixed with /api/games inside api.rs if you prefer, or mapped here)
            // Note: In api.rs, our routes are /games...
            // We will mount api::router() directly.
            .merge(api::router())
            .fallback_service(serve_dir) // All other requests go to static files
    };

    // 2. Build Router
    // Note: Axum matches specific routes (like /games) first, then falls back to serve_dir
    let app = router.layer(CorsLayer::permissive()).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("🚀 Server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
