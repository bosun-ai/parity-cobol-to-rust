//! Reads inventory, reserves stock, and writes reservation events.
//!
//! The HTTP routes call this code after they check the request values.

use std::{
    fs::OpenOptions,
    io::{self, Write as _},
    path::PathBuf,
    sync::Arc,
};

use thiserror::Error;
use tokio_postgres::{
    Client, Row,
    types::{ToSql, Type},
};

use crate::domain::{Inventory, ReservationOutcome, ReservationQuantity, Sku};

const FIND_INVENTORY: &str = "SELECT sku, available, reserved, available - reserved AS remaining FROM inventory WHERE sku = $1";
// One database call updates the stock and returns the result.
const RESERVE_INVENTORY: &str = "WITH updated AS (UPDATE inventory SET reserved = reserved + $2 WHERE sku = $1 AND available - reserved >= $2 RETURNING sku, available, reserved, available - reserved AS remaining) SELECT TRUE AS reserved_now, sku, available, reserved, remaining FROM updated UNION ALL SELECT FALSE AS reserved_now, sku, available, reserved, available - reserved FROM inventory WHERE sku = $1 AND NOT EXISTS (SELECT 1 FROM updated)";

/// Reads inventory and records reservations.
#[derive(Clone)]
pub(crate) struct InventoryService {
    client: Arc<Client>,
    event_log: Arc<PathBuf>,
}

impl InventoryService {
    /// Creates the service with its database and event file.
    pub(crate) const fn new(client: Arc<Client>, event_log: Arc<PathBuf>) -> Self {
        Self { client, event_log }
    }

    /// Gets one inventory item without changing it.
    pub(crate) async fn find(&self, sku: &Sku) -> Result<Option<Inventory>, ServiceError> {
        let sku = sku.as_str();
        let parameters: &[(&(dyn ToSql + Sync), Type)] = &[(&sku, Type::TEXT)];
        self.client
            .query_typed_opt(FIND_INVENTORY, parameters)
            .await?
            .map(|row| inventory_from_row(&row))
            .transpose()
    }

    /// Reserves stock and writes an event when it succeeds.
    pub(crate) async fn reserve(
        &self,
        sku: &Sku,
        quantity: ReservationQuantity,
    ) -> Result<Option<ReservationOutcome>, ServiceError> {
        let sku = sku.as_str();
        let requested = quantity.get();
        let parameters: &[(&(dyn ToSql + Sync), Type)] =
            &[(&sku, Type::TEXT), (&requested, Type::INT4)];
        let outcome = self
            .client
            .query_typed_opt(RESERVE_INVENTORY, parameters)
            .await?
            .map(|row| {
                let inventory = inventory_from_row(&row)?;
                Ok::<_, ServiceError>(if row.try_get::<_, bool>("reserved_now")? {
                    ReservationOutcome::Reserved(inventory)
                } else {
                    ReservationOutcome::Insufficient(inventory)
                })
            })
            .transpose()?;

        if let Some(ReservationOutcome::Reserved(inventory)) = &outcome {
            self.append_event(&inventory.event_line(quantity))?;
        }
        Ok(outcome)
    }

    /// Adds one line to the event file.
    fn append_event(&self, event: &str) -> Result<(), ServiceError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.event_log.as_ref())?;
        file.write_all(event.as_bytes())?;
        file.flush()?;
        Ok(())
    }
}

/// Turns one database result into inventory.
fn inventory_from_row(row: &Row) -> Result<Inventory, ServiceError> {
    Ok(Inventory::new(
        Sku::parse(row.try_get("sku")?).map_err(|_| ServiceError::InvalidSku)?,
        row.try_get("available")?,
        row.try_get("reserved")?,
        row.try_get("remaining")?,
    ))
}

/// A database or event file error.
#[derive(Debug, Error)]
pub(crate) enum ServiceError {
    /// The database query failed.
    #[error("PostgreSQL query failed")]
    Query(#[from] tokio_postgres::Error),
    /// The service could not write the event.
    #[error("could not append reservation event")]
    Event(#[from] io::Error),
    /// The database returned an invalid SKU.
    #[error("PostgreSQL returned an invalid SKU")]
    InvalidSku,
}
