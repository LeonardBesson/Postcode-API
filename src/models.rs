use uuid::Uuid;
use chrono::NaiveDateTime;
use crate::schema::addresses;

#[derive(Queryable, Debug)]
pub struct State {
    pub id: Uuid,
    pub hash: String,
    pub processed_at: NaiveDateTime
}

#[derive(Queryable, Debug)]
pub struct Address {
    pub id: Uuid,
    pub lon: f64,
    pub lat: f64,
    pub number: String,
    pub street: String,
    pub city: String,
    pub region: String,
    pub postcode: String
}

#[derive(Insertable, Debug)]
#[table_name="addresses"]
pub struct NewAddress<'a> {
    pub id: Uuid,
    pub lon: f64,
    pub lat: f64,
    pub number: &'a str,
    pub street: &'a str,
    pub city: &'a str,
    pub region: &'a str,
    pub postcode: &'a str
}
