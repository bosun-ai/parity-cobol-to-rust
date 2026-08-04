//! HTTP routes and public JSON response shapes.
//!
//! Handlers validate path values, call the inventory service, and map each
//! domain outcome to the response shape shared with the COBOL service.

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

/// Builds the inventory API with shared service state.
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

/// Reports process health without touching storage.
async fn health() -> Response {
    (StatusCode::OK, Json(HealthResponse { status: "ok" })).into_response()
}

/// Returns one inventory item or the shared not-found response.
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

/// Validates and attempts a reservation, then maps its domain outcome to HTTP.
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

/// Rejects percent-encoded paths so the Rust and COBOL parsers accept the same inputs.
fn reject_encoded_path(uri: &axum::http::Uri) -> Result<(), ApiError> {
    (!uri.path().contains('%'))
        .then_some(())
        .ok_or(ApiError::InvalidRequest)
}

/// Handles unknown paths and unsupported methods with the shared response.
async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found")
}

/// A request failed before the service could produce a response value.
#[derive(Debug, Error)]
enum ApiError {
    /// The path contains a value outside the public request rules.
    #[error("invalid request")]
    InvalidRequest,
    /// No inventory row exists for the requested SKU.
    #[error("inventory not found")]
    NotFound,
    /// Storage or event recording failed.
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

/// Builds the common JSON error shape.
fn error_response(status: StatusCode, error: &'static str) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}

/// JSON body returned by the health route.
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// JSON body returned when the requested stock is unavailable.
#[derive(Serialize)]
struct InsufficientStockResponse<'a> {
    error: &'static str,
    sku: &'a Sku,
    available: i32,
    reserved: i32,
    requested: i32,
}

impl<'a> InsufficientStockResponse<'a> {
    /// Builds the conflict response from the unchanged inventory snapshot.
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

/// JSON body shared by public error responses.
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
