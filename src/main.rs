#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate dotenv;
#[macro_use]
extern crate lazy_static;

use std::{io, thread};

use actix::fut::ok;
use actix::{System, Arbiter, Actor, Running, AsyncContext, SystemRunner};
use actix_web::{App, Error, HttpResponse, HttpServer, Responder, web};
use actix_web::client::Client;
use actix_web::middleware::Logger;
use diesel::update;
use diesel_migrations::embed_migrations;
use env_logger;
use futures::{Future, lazy};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::data::{get_addresses, RefreshError, StateInfo, refresh_state};
use crate::db::{init_connection_pool, Pool, establish_connection};
use std::time::Duration;

mod schema;
mod data;
mod db;
mod models;
mod postcode;
mod tests;

embed_migrations!("./migrations");

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
        Err(err) => { Ok(HttpResponse::InternalServerError().finish()) },
    })
}

struct StateRefresher {}

impl StateRefresher {
    const INTERVAL_SECS: u64 = 3600 * 4;

    fn start() {
        thread::Builder::new()
            .name("state-refresher".into())
            .spawn(|| {
                info!("Starting state refresh every {} hours", Self::INTERVAL_SECS / 3600);
                loop {
                    thread::sleep(Duration::from_secs(Self::INTERVAL_SECS));
                    let conn = establish_connection();
                    refresh_state(&conn);
                }
            });
    }
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let pool = init_connection_pool();
    embedded_migrations::run(&pool.get().unwrap());

    let system = System::new("postcode-service");
    refresh_state(&pool.get().unwrap());

    StateRefresher::start();

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
