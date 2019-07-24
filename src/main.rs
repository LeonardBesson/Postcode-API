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
use log::info;

use data::{get_state_info, update_state};
use diesel::update;

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
        Ok(_) => { info!("Done refreshing status") },
        Err(err) => { panic!("Could not refresh status {}", err) }
    };
//    let status = system.block_on(lazy(|| { update_state("1234".to_string(), "http://data.openaddresses.io/runs/655683/nl/countrywide.zip".to_string()) }));

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .route("/", web::get().to(index))
    })
    .bind("127.0.0.1:3000")?
    .start();

    system.run()
}
