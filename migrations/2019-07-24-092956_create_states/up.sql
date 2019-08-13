-- Your SQL goes here
CREATE TABLE states (
    id UUID PRIMARY KEY,
    hash TEXT NOT NULL,
    version TEXT NOT NULL,
    processed_at TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT (now() AT TIME ZONE 'utc')
);