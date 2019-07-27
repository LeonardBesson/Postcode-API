#[macro_use]
extern crate diesel;
extern crate dotenv;

use std::io;

use actix::System;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web::client::Client;
use actix_web::middleware::Logger;
use env_logger;
use futures::{Future, lazy};
use log::{info, error};

use data::{get_state_info, update_state};
use diesel::update;
use crate::data::{StateInfo, RefreshError};

mod schema;
mod data;
mod db;
mod models;

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let mut system = System::new("postcode-service");

    let status = system.block_on(lazy(|| { get_state_info() }));
    match status {
        Ok(state_info) => {
            if state_info.info.is_none() {
                if state_info.current_state.is_none() {
                    panic!("Couldn't fetch data and no fallback");
                } else {
                    info!("Falling back");
                }
            } else {
                info!("Updating state...");
                let (url, hash) = state_info.info.unwrap();
                match system.block_on(lazy(|| { update_state(url, hash) })) {
                    Ok(_) => { info!("Successfuly updated state"); },
                    Err(err) => { error!("Error while updating state: {}", err); },
                }
            }
        },
        Err(RefreshError::NoData) => { panic!("Couldn't fetch data and no fallback"); },
        Err(RefreshError::OldData) => { error!("Falling back"); }
    };

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .route("/", web::get().to(index))
    })
    .bind("127.0.0.1:3000")?
    .start();

    system.run()
}
