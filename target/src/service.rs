//! Database inventory operations and reservation event recording.

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
const RESERVE_INVENTORY: &str = "WITH updated AS (UPDATE inventory SET reserved = reserved + $2 WHERE sku = $1 AND available - reserved >= $2 RETURNING sku, available, reserved, available - reserved AS remaining) SELECT TRUE AS reserved_now, sku, available, reserved, remaining FROM updated UNION ALL SELECT FALSE AS reserved_now, sku, available, reserved, available - reserved FROM inventory WHERE sku = $1 AND NOT EXISTS (SELECT 1 FROM updated)";

#[derive(Clone)]
pub(crate) struct InventoryService {
    client: Arc<Client>,
    event_log: Arc<PathBuf>,
}

impl InventoryService {
    pub(crate) const fn new(client: Arc<Client>, event_log: Arc<PathBuf>) -> Self {
        Self { client, event_log }
    }

    pub(crate) async fn find(&self, sku: &Sku) -> Result<Option<Inventory>, ServiceError> {
        let sku = sku.as_str();
        let parameters: &[(&(dyn ToSql + Sync), Type)] = &[(&sku, Type::TEXT)];
        self.client
            .query_typed_opt(FIND_INVENTORY, parameters)
            .await?
            .map(|row| inventory_from_row(&row))
            .transpose()
    }

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

fn inventory_from_row(row: &Row) -> Result<Inventory, ServiceError> {
    Ok(Inventory::new(
        Sku::parse(row.try_get("sku")?).map_err(|_| ServiceError::InvalidSku)?,
        row.try_get("available")?,
        row.try_get("reserved")?,
        row.try_get("remaining")?,
    ))
}

#[derive(Debug, Error)]
pub(crate) enum ServiceError {
    #[error("PostgreSQL query failed")]
    Query(#[from] tokio_postgres::Error),
    #[error("could not append reservation event")]
    Event(#[from] io::Error),
    #[error("PostgreSQL returned an invalid SKU")]
    InvalidSku,
}
