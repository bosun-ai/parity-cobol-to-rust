# COBOL to Rust, proven with Parity

This repository contains one inventory service implemented twice: first in GNU COBOL, then in
Rust. The Parity proof describes the behavior both programs must share.

The example is the source for [Introducing Parity](https://homeport.ai/blog/cobol-rust-intent).

## What it proves

- Reading inventory returns the same response without changing state.
- A valid reservation updates the same row and appends the same event.
- Invalid input, missing inventory, and insufficient stock have no side effects.
- Generated invalid inputs give the same result in both programs.

Read [`specs/inventory.md`](specs/inventory.md) for the scenarios and complete proof blocks.

## Build the services

The build needs Rust 1.97, GNU COBOL, PostgreSQL client headers, and `libpq`.

```console
$ ./build.sh
```

This creates `bin/legacy` and `bin/target`. The Rust package can also be checked on its own:

```console
$ cargo fmt --manifest-path target/Cargo.toml --all -- --check
$ cargo clippy --manifest-path target/Cargo.toml --all-targets --locked -- -D warnings
$ cargo test --manifest-path target/Cargo.toml --locked
```

## Run the proof

Parity is not part of this repository. If you have access to its preview, provide two PostgreSQL
databases and run:

```console
$ export LEGACY_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1/legacy
$ export TARGET_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1/target
$ ./build.sh
$ parity audit
$ parity verify
```

The proof compares HTTP responses, inventory rows, event contents, and observed side effects. It
requires equality except for each difference that the proof requires by path and value.

## Try the defects

Two patches recreate the mistakes used in the article:

- [`defects/split-reservation.patch`](defects/split-reservation.patch) changes one database action
  into a read followed by a write.
- [`defects/missing-event.patch`](defects/missing-event.patch) stops writing the reservation event.

The helper applies a patch to a temporary copy. It never changes this checkout:

```console
$ ./scripts/verify-defect.sh split-reservation
$ ./scripts/verify-defect.sh missing-event
```

Each command succeeds only when Parity rejects the changed target for the expected reason. Reviewed
output from the example is in [`expected/`](expected/).

## Layout

- `legacy/` contains the GNU COBOL service.
- `target/` contains the Rust service.
- `specs/` contains the behavior and Parity proof.
- `Parityfile` connects the two programs to the proof.
- `defects/` contains the two article patches.

## License

Apache-2.0. See [`LICENSE`](LICENSE).
