-- JWT plugin requires jwks table for storing JSON Web Key Sets
create table if not exists "jwks" (
  "id" text not null primary key,
  "publicKey" text not null,
  "privateKey" text not null,
  "createdAt" timestamptz default CURRENT_TIMESTAMP not null
);
