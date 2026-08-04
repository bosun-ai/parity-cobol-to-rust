# Inventory behavior

PostgreSQL traffic remains part of the proof. The clients use different labels for request 2, so
each database scenario requires `libpq` to report `portal` and `tokio-postgres` to report
`statement`. Parity requires every other captured value to match.

### Requirement: Available inventory can be reserved durably

#### Scenario: Reserve stock that is available

- **GIVEN** SKU `book` has 3 available units and none reserved
- **WHEN** 2 units are reserved
- **THEN** the request succeeds, 1 unit remains, and one reservation event is appended

```parity
proof inventory.reserve {
  example available { sku = "book", quantity = "2" }
  when reserve(sku, quantity)
  then {
    assert (result.status == 201 &&
      result.body.sku == examples.sku &&
      result.body.available == 3 &&
      result.body.reserved == 2 &&
      result.body.remaining == 1)
    assert views.inventory == {
      "added": [{"sku": "book", "available": 3, "reserved": 2, "remaining": 1}],
      "removed": [{"sku": "book", "available": 3, "reserved": 0, "remaining": 3}]
    }
    assert (views.events.added == [] &&
      views.events.removed == [] &&
      views.events.modified.size() == 1 &&
      views.events.modified[0].path == "reservations.tsv" &&
      views.events.modified[0].before.content == "" &&
      views.events.modified[0].after.content == "reserved|book|2|2|1\n")
    assert ((diff.path == ["views", "filesystem", "added", 0, "resource"] ||
      diff.path == ["views", "filesystem_state", "added", 0, "resource"]) &&
      diff.legacy.endsWith("/legacy/reservations.tsv") &&
      diff.target.endsWith("/target/reservations.tsv"))
    assert (diff.path == ["views", "postgres", "added", 0, "request", 2, "target"] &&
      diff.legacy == "portal" && diff.target == "statement")
  }
}
```

### Requirement: Invalid reservation input is rejected

#### Scenario: Reject invalid reservation paths

- **GIVEN** a reservation path contains an invalid SKU or quantity
- **WHEN** the reservation is requested
- **THEN** the request is rejected without changing inventory or appending an event

```parity
proof inventory.invalid-reservation {
  example uppercase-sku { sku = "BOOK", quantity = "1" }
  example zero-quantity { sku = "book", quantity = "0" }
  example non-numeric-quantity { sku = "book", quantity = "two" }
  example excessive-quantity { sku = "book", quantity = "10000" }
  generate 4 cases {
    sku = text matching "[a-z]{1,12}"
    quantity = text matching "[1-9][0-9]{4}"
  }
  when reserve(sku, quantity)
  then {
    assert (result.status == 400 && result.body.error == "invalid_request")
    assert views.inventory == {"added": [], "removed": []}
    assert views.events == {"added": [], "removed": [], "modified": []}
  }
}
```

### Requirement: Missing inventory is reported without side effects

#### Scenario: Read an unknown SKU

- **GIVEN** SKU `missing` does not exist
- **WHEN** its inventory is requested
- **THEN** the request reports it missing without changing inventory or appending an event

```parity
proof inventory.missing {
  example unknown { sku = "missing" }
  when get_inventory(sku)
  then {
    assert (result.status == 404 && result.body.error == "not_found")
    assert views.inventory == {"added": [], "removed": []}
    assert views.events == {"added": [], "removed": [], "modified": []}
    assert (diff.path == ["views", "postgres", "added", 0, "request", 2, "target"] &&
      diff.legacy == "portal" && diff.target == "statement")
  }
}
```

### Requirement: Inventory cannot be over-reserved

#### Scenario: Reject a reservation larger than the available stock

- **GIVEN** SKU `book` has 3 available units and none reserved
- **WHEN** 4 units are reserved
- **THEN** the request reports insufficient stock without changing inventory or appending an event

```parity
proof inventory.reject {
  example excessive { sku = "book", quantity = "4" }
  when reserve(sku, quantity)
  then {
    assert (result.status == 409 &&
      result.body.error == "insufficient_stock" &&
      result.body.sku == examples.sku &&
      result.body.available == 3 &&
      result.body.reserved == 0 &&
      result.body.requested == 4)
    assert views.inventory == {"added": [], "removed": []}
    assert views.events == {"added": [], "removed": [], "modified": []}
    assert (diff.path == ["views", "postgres", "added", 0, "request", 2, "target"] &&
      diff.legacy == "portal" && diff.target == "statement")
  }
}
```

### Requirement: Reading inventory has no business side effects

#### Scenario: Read current inventory

- **GIVEN** SKU `book` has 3 available units and none reserved
- **WHEN** its inventory is requested
- **THEN** the current inventory is returned without changing inventory or appending an event

```parity
proof inventory.show {
  example existing { sku = "book" }
  when get_inventory(sku)
  then {
    assert (result.status == 200 &&
      result.body.sku == examples.sku &&
      result.body.available == 3 &&
      result.body.reserved == 0 &&
      result.body.remaining == 3)
    assert views.inventory == {"added": [], "removed": []}
    assert views.events == {"added": [], "removed": [], "modified": []}
    assert (diff.path == ["views", "postgres", "added", 0, "request", 2, "target"] &&
      diff.legacy == "portal" && diff.target == "statement")
  }
}
```
