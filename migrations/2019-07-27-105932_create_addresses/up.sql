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

    -- UNIQUE creates a B-tree index which can be used in a
    -- LIKE/ILIKE query if the pattern is left-anchored ('foo%')
    -- so there is no need to add additional indexes. Moreover,
    -- composite indexes can be taken advantage of in queries
    -- on the first column only, so we don't need another
    -- index for the cases where we search by postcode only.
    CONSTRAINT u_postcode_number UNIQUE (postcode, number)
);