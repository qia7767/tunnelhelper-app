use crate::{config::AuthMode, service::CommandService};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "../web"]
struct WebAssets;

#[derive(Clone)]
pub struct ApiState {
    pub service: CommandService,
    pub auth_mode: AuthMode,
    pub access_token: Option<Arc<str>>,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/runtime", get(runtime))
        .route("/api/invoke/{command}", post(invoke))
        .fallback(static_asset)
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "tunnel-helper-server" }))
}

async fn runtime(State(state): State<ApiState>) -> Json<Value> {
    Json(json!({
        "runtime": "browser",
        "platform": "linux",
        "authRequired": state.auth_mode == AuthMode::Token,
        "dataDir": state.service.data_dir().to_string_lossy(),
    }))
}

async fn invoke(
    State(state): State<ApiState>,
    Path(command): Path<String>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    authorize(&state, &headers)?;
    state
        .service
        .invoke(&command, args)
        .await
        .map(Json)
        .map_err(ApiError::bad_request)
}

fn authorize(state: &ApiState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Ok(());
    }
    let expected = state
        .access_token
        .as_deref()
        .ok_or_else(|| ApiError::internal("Access Token mode is not configured"))?;
    let supplied = headers
        .get("x-tunnel-token")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        });
    if supplied == Some(expected) {
        Ok(())
    } else {
        Err(ApiError::unauthorized())
    }
}

async fn static_asset(uri: axum::http::Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    let (asset_path, asset) = match WebAssets::get(requested) {
        Some(asset) => (requested, asset),
        None => match WebAssets::get("index.html") {
            Some(asset) => ("index.html", asset),
            None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
    };
    let mime = mime_guess::from_path(asset_path).first_or_octet_stream();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(asset.data.into_owned()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "Access Token is required or invalid".to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
}
