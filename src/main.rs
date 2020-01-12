#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate dotenv;

use std::io;

use actix_web::{App, Error, HttpResponse, HttpServer, web};
use actix_web::middleware::Logger;
use env_logger;
use log::{error, info};
use serde::Deserialize;

use crate::data::{get_addresses, refresh_state};
use crate::db::{init_connection_pool, Pool};
use crate::state_refresher::StateRefresher;
use std::time::Duration;

mod schema;
mod data;
mod db;
mod models;
mod tests;
mod state_refresher;
mod utils;

const DATA_REFRESH_INTERVAL_SECS: u64 = 3600 * 24;

#[derive(Deserialize)]
pub struct AddressRequest {
    postcode: String,
    number: Option<String>
}

async fn addresses(
    request: web::Query<AddressRequest>,
    pool: web::Data<Pool>
) -> Result<HttpResponse, Error> {
    let result = web::block(move || {
        get_addresses(
            pool,
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

embed_migrations!("./migrations");

#[actix_rt::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let pool = init_connection_pool();
    let conn = pool.get().unwrap();

    web::block(move || { embedded_migrations::run(&conn) })
        .await
        .expect("Error while running migrations");

    if let Err(err) = refresh_state(&pool).await {
        error!("Error while refreshing state: {}", err);
    };

    // Start background periodic state refresh
    let refresher_pool = pool.clone();
    actix_rt::spawn(async move {
        let state_refresher = StateRefresher::new(
            Duration::from_secs(DATA_REFRESH_INTERVAL_SECS),
            false
        );
        state_refresher.start(&refresher_pool).await;
    });

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .wrap(Logger::default())
            .route("/addresses", web::get().to(addresses))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
