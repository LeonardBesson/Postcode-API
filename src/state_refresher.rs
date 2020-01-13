use std::time::Duration;
use crate::db::Pool;
use crate::data::refresh_state;
use log::{error, info};

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
            if let Err(err) = refresh_state(&pool).await {
                error!("Error while refreshing state: {}", err);
            }
        }
    }
}
