use actix_web::web;
use diesel::PgConnection;
use indicatif::ProgressBar;
use log::{error, info};
use regex::Regex;
use zip::ZipArchive;

use crate::data::models::AddressRecord;
use crate::data::models::State;
use crate::data::repo::addresses::create_or_update_addresses;
use crate::data::repo::states::{create_new_state, current_state};
use crate::data::state::error::RefreshError;
use crate::db::Pool;
use crate::utils::ExistsExtension;

pub mod error;
pub mod state_refresher;

#[derive(Debug)]
pub struct StateInfo {
    pub url: String,
    pub hash: String,
    pub version: String,
    pub address_count: usize
}

#[derive(Debug)]
pub struct DataStatus {
    pub state_info: Option<StateInfo>,
    pub current_state: Option<State>
}

const BATCH_SIZE: usize = 2500;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/79.0.3945.117 Safari/537.36";
const STATE_INFO_URL: &str = "http://results.openaddresses.io/state.txt";

pub async fn refresh_state(pool: &Pool) -> Result<(), RefreshError> {
    let status = get_data_status(&pool).await?;
    match status.state_info {
        Some(state_info) => {
            let up_to_date = status
                .current_state
                .exists(|s| s.version == state_info.version);

            if up_to_date {
                info!("Data already up to date (state: {})", state_info.version);
            } else {
                info!("Updating data...");
                match update_state(&pool, state_info).await {
                    Ok(_) => { info!("Successfully updated data"); },
                    Err(err) => {
                        if status.current_state.is_none() {
                            panic!(
                                "Couldn't update data, and no fallback is available:\n{}",
                                err
                            );
                        }
                        return Err(err);
                    },
                }
            }
        },
        None => {
            if status.current_state.is_none() {
                panic!("Couldn't fetch data info, and no fallback is available");
            } else {
                info!("Falling back to current state");
            }
        }
    }
    Ok(())
}

fn get_state_info<R: std::io::Read>(reader: R) -> Result<Option<StateInfo>, RefreshError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(reader);

    for record in reader.records() {
        let r = record?;
        if r.as_slice().starts_with("nl/countrywide.json") {
            let address_count = r[4]
                .parse::<usize>()
                .map_err(|err| RefreshError::InvalidData(Box::new(err)))?;

            return Ok(Some(StateInfo {
                address_count,
                url: r[8].to_owned(),
                hash: r[10].to_owned(),
                version: r[15].to_owned()
            }))
        }
    }
    return Ok(None);
}

pub async fn get_data_status(pool: &Pool) -> Result<DataStatus, RefreshError> {
    info!("Fetching state info at {}", STATE_INFO_URL);
    let response = reqwest::get(STATE_INFO_URL).await;
    let conn = pool.get().unwrap();
    let current_state = web::block(move || current_state(&conn)).await?;

    match response {
        Ok(resp) => {
            match resp.bytes().await {
                Ok(bytes) => {
                    let state_info = web::block(move || {
                        let cursor = std::io::Cursor::new(&bytes);
                        get_state_info(cursor)
                    })
                    .await?;
                    Ok(DataStatus { state_info, current_state })
                }
                Err(err) => {
                    error!("Error getting bytes from response: {}", err);
                    Ok(DataStatus { state_info: None, current_state })
                }
            }
        },
        Err(err) => {
            error!("Error fetching state info: {}", err);
            Ok(DataStatus { state_info: None, current_state })
        },
    }
}

pub async fn update_state(
    pool: &Pool,
    state_info: StateInfo
) -> Result<(), RefreshError> {
    info!("Downloading state version {} from {}", state_info.version, state_info.url);
    let req = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(&state_info.url);

    let resp_bytes = req
        .send()
        .await?
        .bytes()
        .await?;

    info!("Downloaded zip, size: {} MB", resp_bytes.len() / 1_000_000);
    info!("Searching for csv file");

    let conn = pool.get().unwrap();
    web::block(move || {
        process_data_response(
            state_info,
            &resp_bytes,
            &conn
        )
    })
    .await?;

    Ok(())
}

fn process_data_response(
    state_info: StateInfo,
    bytes: &bytes::Bytes,
    conn: &PgConnection
) -> Result<(), RefreshError> {
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = ZipArchive::new(reader)?;
    let re = Regex::new(r"nl.*\.csv").expect("Could not create regex");

    let mut found = false;
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        info!("File: {}", file.name());
        if re.is_match(file.name()) {
            info!("Found csv file");
            info!("Updating database records...");
            let mut reader = csv::Reader::from_reader(file);

            let mut batch = Vec::<AddressRecord>::with_capacity(BATCH_SIZE);
            let progress_bar = ProgressBar::new(state_info.address_count as u64);
            for record in reader.deserialize() {
                let address_record: AddressRecord = record?;
                batch.push(address_record);
                if batch.len() == BATCH_SIZE {
                    process_batch(&conn, &mut batch, &progress_bar)?;
                }
            };
            process_batch(&conn, &mut batch, &progress_bar)?;
            progress_bar.finish();

            create_new_state(&conn, &state_info)?;
            info!("Done");
            found = true;
            break;
        }
    }
    if !found {
        return Err(RefreshError::FileNotFound)
    }

    Ok(())
}

fn process_batch(
    conn: &PgConnection,
    batch: &mut Vec<AddressRecord>,
    progress_bar: &ProgressBar
) -> Result<(), diesel::result::Error> {
    create_or_update_addresses(&conn, &batch)?;
    progress_bar.inc(batch.len() as u64);
    batch.clear();

    Ok(())
}

