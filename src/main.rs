use std::io;

use actix::System;
use actix_web::{App, Error, HttpResponse, HttpServer, Responder, web};
use actix_web::client::Client;
use actix_web::middleware::Logger;
use env_logger;
use futures::{Future, lazy};
use log::info;

use data::fetch_status;

mod data;

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let mut system = System::new("postcode-service");

    let status = system.block_on(lazy(|| { fetch_status() }));
    match status {
        Ok(body) => { info!("Done refreshing status: {}", body) },
        Err(err) => { panic!("Could not refresh status {}", err) }
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
