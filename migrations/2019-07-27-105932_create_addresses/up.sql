-- Your SQL goes here
CREATE TABLE addresses (
    id UUID PRIMARY KEY,
    lat FLOAT8 NOT NULL,
    lon FLOAT8 NOT NULL,
    number TEXT NOT NULL,
    street TEXT NOT NULL,
    city TEXT NOT NULL,
    region TEXT NOT NULL,
    postcode TEXT NOT NULL,

    CONSTRAINT u_postcode_number UNIQUE (postcode, number)
);