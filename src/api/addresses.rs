use actix_web::{Error, HttpResponse, web};
use log::error;
use serde::Deserialize;

use crate::data::repo::addresses::get_addresses;
use crate::db::Pool;

#[derive(Deserialize)]
pub struct AddressRequest {
    postcode: String,
    number: Option<String>
}

pub async fn addresses(
    request: web::Query<AddressRequest>,
    pool: web::Data<Pool>
) -> Result<HttpResponse, Error> {
    let result = web::block(move || {
        get_addresses(
            &pool,
            &request.postcode,
            request.number.as_ref().map(|n| n.as_str())
        )
    })
        .await;

    match result {
        Ok(addresses) => { Ok(HttpResponse::Ok().json(addresses)) },
        Err(err) => {
            error!("Error while retrieving addresses: {}", err);
            Ok(HttpResponse::InternalServerError().finish())
        },
    }
}
