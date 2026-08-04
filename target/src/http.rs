//! HTTP routes and public JSON response shapes.

use axum::{
    Json, Router,
    extract::{OriginalUri, Path, State, rejection::PathRejection},
    http::StatusCode,
    response::{IntoResponse as _, Response},
    routing::{get, post},
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    domain::{Inventory, ReservationOutcome, ReservationQuantity, Sku},
    service::{InventoryService, ServiceError},
};

pub(crate) fn router(inventory: InventoryService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/inventory/{sku}", get(get_inventory))
        .route(
            "/inventory/{sku}/reservations/{quantity}",
            post(reserve_inventory),
        )
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(inventory)
}

async fn health() -> Response {
    (StatusCode::OK, Json(HealthResponse { status: "ok" })).into_response()
}

async fn get_inventory(
    State(inventory): State<InventoryService>,
    OriginalUri(uri): OriginalUri,
    path: Result<Path<String>, PathRejection>,
) -> Result<Response, ApiError> {
    reject_encoded_path(&uri)?;
    let Path(sku) = path.map_err(|_| ApiError::InvalidRequest)?;
    let sku = Sku::parse(sku).map_err(|_| ApiError::InvalidRequest)?;

    inventory.find(&sku).await?.map_or_else(
        || Err(ApiError::NotFound),
        |item| Ok((StatusCode::OK, Json(item)).into_response()),
    )
}

async fn reserve_inventory(
    State(inventory): State<InventoryService>,
    OriginalUri(uri): OriginalUri,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Response, ApiError> {
    reject_encoded_path(&uri)?;
    let Path((sku, quantity)) = path.map_err(|_| ApiError::InvalidRequest)?;
    let sku = Sku::parse(sku).map_err(|_| ApiError::InvalidRequest)?;
    let quantity = ReservationQuantity::parse(&quantity).map_err(|_| ApiError::InvalidRequest)?;

    match inventory.reserve(&sku, quantity).await? {
        Some(ReservationOutcome::Reserved(item)) => {
            Ok((StatusCode::CREATED, Json(item)).into_response())
        }
        Some(ReservationOutcome::Insufficient(item)) => Ok((
            StatusCode::CONFLICT,
            Json(InsufficientStockResponse::new(&item, quantity)),
        )
            .into_response()),
        None => Err(ApiError::NotFound),
    }
}

fn reject_encoded_path(uri: &axum::http::Uri) -> Result<(), ApiError> {
    (!uri.path().contains('%'))
        .then_some(())
        .ok_or(ApiError::InvalidRequest)
}

async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found")
}

#[derive(Debug, Error)]
enum ApiError {
    #[error("invalid request")]
    InvalidRequest,
    #[error("inventory not found")]
    NotFound,
    #[error(transparent)]
    Internal(#[from] ServiceError),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidRequest => error_response(StatusCode::BAD_REQUEST, "invalid_request"),
            Self::NotFound => error_response(StatusCode::NOT_FOUND, "not_found"),
            Self::Internal(error) => {
                tracing::error!(error = ?error, "inventory request failed");
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        }
    }
}

fn error_response(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct InsufficientStockResponse<'a> {
    error: &'static str,
    sku: &'a Sku,
    available: i32,
    reserved: i32,
    requested: i32,
}

impl<'a> InsufficientStockResponse<'a> {
    const fn new(item: &'a Inventory, quantity: ReservationQuantity) -> Self {
        Self {
            error: "insufficient_stock",
            sku: item.sku(),
            available: item.available(),
            reserved: item.reserved(),
            requested: quantity.get(),
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_error_response_does_not_expose_diagnostics() {
        assert_eq!(
            serde_json::to_string(&ErrorResponse { error: "internal" }).unwrap(),
            r#"{"error":"internal"}"#
        );
    }
}
