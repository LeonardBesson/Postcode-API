use std::{thread, io};
use std::time::Duration;
use crate::db::establish_connection;
use crate::data::refresh_state;
use log::info;
use std::thread::JoinHandle;

pub struct StateRefresher {}

impl StateRefresher {
    const INTERVAL_SECS: u64 = 3600 * 4;

    pub fn start() -> io::Result<JoinHandle<()>> {
        thread::Builder::new()
            .name("state-refresher".into())
            .spawn(|| {
                info!("Starting state refresh every {} hours", Self::INTERVAL_SECS / 3600);
                loop {
                    thread::sleep(Duration::from_secs(Self::INTERVAL_SECS));
                    let conn = establish_connection();
                    refresh_state(&conn);
                }
            })
    }
}