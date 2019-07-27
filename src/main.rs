#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate dotenv;

use std::io;

use actix::System;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web::client::Client;
use actix_web::middleware::Logger;
use diesel::update;
use diesel_migrations::embed_migrations;
use env_logger;
use futures::{Future, lazy};
use log::{error, info};

use data::{get_state_info, update_state};

use crate::data::{RefreshError, StateInfo};
use crate::db::init_connection_pool;

mod schema;
mod data;
mod db;
mod models;
mod postcode;

embed_migrations!("./migrations");

fn addresses() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let pool = init_connection_pool();
    embedded_migrations::run(&pool.get().unwrap());

    let mut system = System::new("postcode-service");

    let status = system.block_on(lazy(|| { get_state_info(&pool) }));
    match status {
        Ok(state_info) => {
            match state_info.info {
                Some((count, url, state_hash)) => {
                    let up_to_date = state_info
                        .current_state
                        .filter(|s| s.hash == state_hash)
                        .is_some();

                    if up_to_date {
                        info!("Data already up to date (state: {})", state_hash);
                    } else {
                        info!("Updating data...");
                        match system.block_on(lazy(|| { update_state(&pool, count, url, state_hash) })) {
                            Ok(_) => { info!("Successfuly updated data"); },
                            Err(err) => { error!("Error while updating state: {}", err); },
                        };
                    }
                },
                None => {
                    if state_info.current_state.is_none() {
                        panic!("Couldn't fetch data and no fallback");
                    } else {
                        info!("Falling back");
                    };
                }
            }
        },
        Err(RefreshError::NoData) => { panic!("Couldn't fetch data and no fallback"); },
        Err(RefreshError::OldData) => { error!("Falling back"); }
    };

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .route("/addresses", web::get().to(addresses))
    })
    .bind("127.0.0.1:3000")?
    .start();

    system.run()
}
