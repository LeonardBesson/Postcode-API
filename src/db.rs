use std::env;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;

use dotenv::dotenv;

pub type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;

const DEFAULT_DB_POOL_SIZE: usize = 15;

pub fn init_connection_pool() -> Pool {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool_size = env::var("DATABASE_POOL_SIZE")
        .map(|size| size
            .parse::<u32>()
            .expect("DATABASE_POOL_SIZE must be an integer")
        )
        .unwrap_or(15);

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .max_size(pool_size)
        .build(manager)
        .expect("Failed to create db pool")
}