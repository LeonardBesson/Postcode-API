table! {
    addresses (id) {
        id -> Uuid,
        lat -> Float8,
        lon -> Float8,
        number -> Text,
        street -> Text,
        city -> Text,
        region -> Text,
        postcode -> Text,
    }
}

table! {
    states (id) {
        id -> Uuid,
        hash -> Text,
        processed_at -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(
    addresses,
    states,
);
