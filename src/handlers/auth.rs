use axum::{Router, routing::{post, get}, extract::State, Json};

use crate::models::user::{User, LoginRequest, RegisterRequest, AuthResponse, UserPublic};
use crate::middleware::auth::{AuthenticatedUser, create_token};
use crate::error::AppError;
use crate::db::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(get_me))
}

use rand::Rng;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    if payload.username.trim().is_empty() || payload.email.trim().is_empty() || payload.password.len() < 6 {
        return Err(AppError::BadRequest("Invalid input".into()));
    }

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1 OR username = $2"
    )
    .bind(&payload.email)
    .bind(&payload.username)
    .fetch_one(&state.pool)
    .await?;

    if existing > 0 {
        return Err(AppError::Conflict("Email or username already exists".into()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Hash error: {}", e)))?
        .to_string();

    let account_id = loop {
        let candidate: String = (0..12).map(|_| rand::thread_rng().gen_range(0..10).to_string()).collect();
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE account_id = $1")
            .bind(&candidate).fetch_one(&state.pool).await.unwrap_or(0);
        if exists == 0 { break candidate; }
    };

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, account_id) VALUES ($1, $2, $3, $4) RETURNING *"
    )
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&account_id)
    .fetch_one(&state.pool)
    .await?;

    let token = create_token(user.id, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(format!("Token error: {}", e)))?;

    Ok(Json(AuthResponse { token, user: user.into() }))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::Unauthorized("Invalid credentials".into()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|_| AppError::Internal("Invalid password hash".into()))?;

    let valid = Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_ok();

    if !valid {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    let token = create_token(user.id, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(format!("Token error: {}", e)))?;

    Ok(Json(AuthResponse { token, user: user.into() }))
}

async fn get_me(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<UserPublic>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound("User not found".into()))?;

    Ok(Json(user.into()))
}
