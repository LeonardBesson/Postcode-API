use std::{thread, io};
use std::time::Duration;
use crate::db::{establish_connection, Pool};
use crate::data::refresh_state;
use log::info;
use std::thread::JoinHandle;

pub struct StateRefresher {
    pub interval: Duration,
    // If the first tick should be immediate
    pub immediate: bool
}

impl StateRefresher {
    pub fn new(interval: Duration, immediate: bool) -> Self {
        Self { interval, immediate }
    }

    pub async fn start(self, pool: &Pool) {
        let mut interval = actix_rt::time::interval(self.interval);
        if !self.immediate {
            interval.tick().await;
        }
        loop {
            interval.tick().await;
            info!("StateRefresher: refreshing data...");
            refresh_state(&pool).await;
        }
    }
}
