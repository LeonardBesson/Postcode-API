#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate dotenv;

use std::io;

use actix::System;
use actix_web::{App, Error, HttpResponse, HttpServer, web};
use actix_web::middleware::Logger;
use env_logger;
use log::error;
use futures::Future;
use serde::Deserialize;

use crate::data::{get_addresses, refresh_state};
use crate::db::{init_connection_pool, Pool};
use crate::state_refresher::StateRefresher;

mod schema;
mod data;
mod db;
mod models;
mod postcode;
mod tests;
mod state_refresher;

#[derive(Deserialize)]
pub struct AddressRequest {
    postcode: String,
    number: Option<String>
}

fn addresses(
    request: web::Query<AddressRequest>,
    pool: web::Data<Pool>
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || {
        get_addresses(
            pool,
            &request.postcode,
            request.number.as_ref().map(|n| n.as_str())
        )
    })
    .then(|res| match res {
        Ok(addresses) => { Ok(HttpResponse::Ok().json(addresses)) },
        Err(err) => {
            error!("Error while retrieving addresses: {}", err);
            Ok(HttpResponse::InternalServerError().finish())
        },
    })
}

embed_migrations!("./migrations");

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let pool = init_connection_pool();
    embedded_migrations::run(&pool.get().unwrap())
        .expect("Error while running migrations");

    let system = System::new("postcode-service");
    refresh_state(&pool.get().unwrap());

    StateRefresher::start()
        .expect("Could not start background state refresh thread");

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .wrap(Logger::default())
            .route("/addresses", web::get().to_async(addresses))
    })
    .bind("127.0.0.1:3000")?
    .start();

    system.run()
}
