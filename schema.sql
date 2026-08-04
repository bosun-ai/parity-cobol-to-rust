drop table if exists inventory;

create table inventory (
  sku text primary key,
  available integer not null check (available >= 0),
  reserved integer not null check (reserved >= 0 and reserved <= available)
);

insert into inventory (sku, available, reserved) values ('book', 3, 0);
