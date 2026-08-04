//! Process entry point for the Rust inventory service.
//!
//! Startup reads process configuration, opens the database, and serves the HTTP
//! adapter on a single-threaded runtime.

mod domain;
mod http;
mod service;

use std::{env, io, net::SocketAddr, num::ParseIntError, path::PathBuf, sync::Arc};

use service::InventoryService;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;

/// Starts the database connection task and inventory HTTP server.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), MainError> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let port = required("PORT")?
        .parse::<u16>()
        .map_err(MainError::InvalidPort)?;
    let database_url = required("DATABASE_URL")?;
    let event_log = PathBuf::from(required("EVENT_LOG")?);
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls).await?;

    tokio::spawn(async move {
        if let Err(error) = connection.await {
            tracing::error!(error = ?error, "PostgreSQL connection stopped");
        }
    });

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    let inventory = InventoryService::new(Arc::new(client), Arc::new(event_log));
    axum::serve(listener, http::router(inventory)).await?;
    Ok(())
}

/// Reads one required process setting without exposing environment errors.
fn required(name: &'static str) -> Result<String, MainError> {
    env::var(name).map_err(|_| MainError::Missing(name))
}

/// Startup or server execution failed.
#[derive(Debug, Error)]
enum MainError {
    /// A required process setting is absent.
    #[error("required environment variable {0} is not set")]
    Missing(&'static str),
    /// The configured port is not an unsigned 16-bit integer.
    #[error("PORT must be a valid TCP port")]
    InvalidPort(#[source] ParseIntError),
    /// The service could not open its database connection.
    #[error("could not connect to PostgreSQL")]
    Database(#[from] tokio_postgres::Error),
    /// The listener or HTTP server returned an I/O error.
    #[error("inventory HTTP server failed")]
    Http(#[from] io::Error),
}
