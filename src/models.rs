use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Queryable)]
pub struct State {
    pub id: Uuid,
    pub hash: String,
    pub processed_at: NaiveDateTime
}