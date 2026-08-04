//! Starts the Rust inventory service.
//!
//! It reads the settings, connects to the database, and starts the HTTP server.

mod domain;
mod http;
mod service;

use std::{env, io, net::SocketAddr, num::ParseIntError, path::PathBuf, sync::Arc};

use service::InventoryService;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_postgres::NoTls;

/// Runs the inventory service.
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

/// Reads one required setting.
fn required(name: &'static str) -> Result<String, MainError> {
    env::var(name).map_err(|_| MainError::Missing(name))
}

/// The service could not start or keep running.
#[derive(Debug, Error)]
enum MainError {
    /// A required setting is missing.
    #[error("required environment variable {0} is not set")]
    Missing(&'static str),
    /// The port is invalid.
    #[error("PORT must be a valid TCP port")]
    InvalidPort(#[source] ParseIntError),
    /// The database connection failed.
    #[error("could not connect to PostgreSQL")]
    Database(#[from] tokio_postgres::Error),
    /// The HTTP server failed.
    #[error("inventory HTTP server failed")]
    Http(#[from] io::Error),
}
