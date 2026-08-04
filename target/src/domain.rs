//! Rules for SKUs, quantities, and inventory responses.
//!
//! The HTTP code checks request values here before it queries the database.

use serde::Serialize;
use thiserror::Error;

/// A SKU accepted by the service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct Sku(String);

impl Sku {
    /// Accepts 1 to 32 lowercase ASCII letters, digits, or hyphens.
    pub(crate) fn parse(value: String) -> Result<Self, ValidationError> {
        let valid = !value.is_empty()
            && value.len() <= 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        valid
            .then_some(Self(value))
            .ok_or(ValidationError::InvalidSku)
    }

    /// Returns the SKU text.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// A request to reserve between 1 and 9999 units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReservationQuantity(i32);

impl ReservationQuantity {
    /// Parses 1 to 4 ASCII digits and rejects zero.
    pub(crate) fn parse(value: &str) -> Result<Self, ValidationError> {
        if value.is_empty() || value.len() > 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ValidationError::InvalidQuantity);
        }

        let quantity = value
            .parse::<i32>()
            .map_err(|_| ValidationError::InvalidQuantity)?;
        (1..=9999)
            .contains(&quantity)
            .then_some(Self(quantity))
            .ok_or(ValidationError::InvalidQuantity)
    }

    /// Returns the number of units to reserve.
    pub(crate) const fn get(self) -> i32 {
        self.0
    }
}

/// Inventory the service sends to the client.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Inventory {
    sku: Sku,
    available: i32,
    reserved: i32,
    remaining: i32,
}

impl Inventory {
    /// Creates inventory from one database result.
    pub(crate) const fn new(sku: Sku, available: i32, reserved: i32, remaining: i32) -> Self {
        Self {
            sku,
            available,
            reserved,
            remaining,
        }
    }

    /// Returns the SKU.
    pub(crate) const fn sku(&self) -> &Sku {
        &self.sku
    }

    /// Returns the number of units in stock.
    pub(crate) const fn available(&self) -> i32 {
        self.available
    }

    /// Returns the number of units reserved after the operation.
    pub(crate) const fn reserved(&self) -> i32 {
        self.reserved
    }

    /// Builds the event line written after a reservation.
    pub(crate) fn event_line(&self, quantity: ReservationQuantity) -> String {
        format!(
            "reserved|{}|{}|{}|{}\n",
            self.sku.as_str(),
            quantity.get(),
            self.reserved,
            self.remaining
        )
    }
}

/// The result of a reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReservationOutcome {
    /// The service reserved the stock.
    Reserved(Inventory),
    /// There was not enough stock.
    Insufficient(Inventory),
}

/// An invalid request value.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(crate) enum ValidationError {
    /// The SKU has the wrong length or contains an invalid character.
    #[error("invalid SKU")]
    InvalidSku,
    /// The quantity is not a number from 1 through 9999.
    #[error("invalid reservation quantity")]
    InvalidQuantity,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sku_accepts_only_supported_identifiers() {
        for valid in ["book", "book-2", "a", "a2345678901234567890123456789012"] {
            assert!(Sku::parse(valid.to_owned()).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "Book",
            "book_2",
            "book/2",
            "a23456789012345678901234567890123",
        ] {
            assert_eq!(
                Sku::parse(invalid.to_owned()),
                Err(ValidationError::InvalidSku),
                "{invalid}"
            );
        }
    }

    #[test]
    fn quantity_accepts_one_to_four_ascii_digits_above_zero() {
        for (source, expected) in [("1", 1), ("0002", 2), ("9999", 9999)] {
            assert_eq!(ReservationQuantity::parse(source).unwrap().get(), expected);
        }
        for invalid in ["", "0", "10000", "+2", "2.0", "2x", "２"] {
            assert_eq!(
                ReservationQuantity::parse(invalid),
                Err(ValidationError::InvalidQuantity),
                "{invalid}"
            );
        }
    }

    #[test]
    fn inventory_has_public_json_and_event_shapes() {
        let item = Inventory::new(Sku::parse("book".to_owned()).unwrap(), 3, 2, 1);

        assert_eq!(
            serde_json::to_string(&item).unwrap(),
            r#"{"sku":"book","available":3,"reserved":2,"remaining":1}"#
        );
        assert_eq!(
            item.event_line(ReservationQuantity::parse("2").unwrap()),
            "reserved|book|2|2|1\n"
        );
    }
}
