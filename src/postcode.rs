use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct AddressRecord {
    pub lon: f32,
    pub lat: f32,
    pub number: String,
    pub street: String,
    pub city: String,
    pub region: String,
    pub postcode: String
}