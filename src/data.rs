use std::collections::HashMap;
use std::fmt::Formatter;

use actix_web::web;
use chrono::Utc;
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use indicatif::ProgressBar;
use log::{error, info};
use regex::Regex;
use uuid::Uuid;
use zip::ZipArchive;

use crate::db::Pool;
use crate::models::{Address, AddressRecord, NewAddress, NewState, State};
use crate::utils::ExistsExtension;

const BATCH_SIZE: usize = 2500;

const ADDRESSES_RESULT_LIMIT: i64 = 200;

const STATE_INFO_URL: &str = "http://results.openaddresses.io/state.txt";

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/79.0.3945.117 Safari/537.36";

#[derive(Debug)]
pub enum RefreshError {
    IO(Box<dyn std::fmt::Debug + Send>),
    InvalidZip(Box<zip::result::ZipError>),
    InvalidData(Box<dyn std::fmt::Debug + Send>),
    FileNotFound
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let msg = match self {
            RefreshError::IO(inner) => {
                format!("IO: {:?}", inner)
            },
            RefreshError::InvalidZip(inner) => {
                format!("Invalid zip data file: {}", inner)
            },
            RefreshError::InvalidData(inner) => {
                format!("Invalid address data: {:?}", inner)
            },
            RefreshError::FileNotFound => {
                "Could not find data file".into()
            }
        };
        write!(f, "Refresh error: {}", msg)
    }
}

impl From<diesel::result::Error> for RefreshError {
    fn from(error: diesel::result::Error) -> Self {
        RefreshError::IO(Box::new(error))
    }
}

impl <E> From<actix_web::error::BlockingError<E>> for RefreshError
where
    E: std::fmt::Debug + Send + 'static,
{
    fn from(error: actix_web::error::BlockingError<E>) -> Self {
        match error {
            actix_web::error::BlockingError::Error(inner) =>
                RefreshError::IO(Box::new(inner)),
            _ =>
                RefreshError::IO(Box::new(error)),
        }
    }
}

impl From<reqwest::Error> for RefreshError {
    fn from(error: reqwest::Error) -> Self {
        RefreshError::IO(Box::new(error))
    }
}

impl From<zip::result::ZipError> for RefreshError {
    fn from(error: zip::result::ZipError) -> Self {
        RefreshError::InvalidZip(Box::new(error))
    }
}

impl From<csv::Error> for RefreshError {
    fn from(error: csv::Error) -> Self {
        RefreshError::InvalidData(Box::new(error))
    }
}

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

fn get_state_info<R: std::io::Read>(response: R) -> Option<StateInfo> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(response);

    reader
        .records()
        .find(|record| {
            record.is_ok() &&
            record
                .as_ref()
                .unwrap()
                .as_slice()
                .starts_with("nl/countrywide.json")
        })
        .map(|record| {
            let r = record.unwrap();
            StateInfo {
                address_count: r[4].parse::<usize>().unwrap(),
                url: r[8].to_owned(),
                hash: r[10].to_owned(),
                version: r[15].to_owned()
            }
        })
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
                       let state_info = get_state_info(std::io::Cursor::new(&bytes));
                       // To help web::block type inference
                       Ok(state_info) as Result<Option<StateInfo>, RefreshError>
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

async fn update_state(
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

fn current_state(conn: &PgConnection) -> Result<Option<State>, diesel::result::Error> {
    use crate::schema::states::dsl::*;

    states
        .order(processed_at.desc())
        .limit(1)
        .first(conn)
        .optional()
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

pub fn create_or_update_addresses<'a>(
    conn: &PgConnection,
    records: &[AddressRecord]
) -> Result<usize, diesel::result::Error> {
    use crate::schema::addresses::dsl::*;

    // Filter duplicates because the same address sometimes has multiple
    // entries in the CSV with different coordinates, for example:
    // 4.6863255,52.3094285,1115,Kruisweg,,Hoofddorp,,Noord-Holland,2131CV,,7c3c022a3d3d5f99
    // 4.6863538,52.3094487,1115,Kruisweg,,Hoofddorp,,Noord-Holland,2131CV,,857b39c71594b270
    let mut address_map: HashMap<(String, String), NewAddress> =
        HashMap::with_capacity(records.len());

    for record in records {
        let key = (record.postcode.clone(), record.number.clone());
        address_map.insert(key, NewAddress {
            id: Uuid::new_v4(),
            lat: record.lat as f64,
            lon: record.lon as f64,
            number: record.number.as_str(),
            street: record.street.as_str(),
            city: record.city.as_str(),
            region: record.region.as_str(),
            postcode: record.postcode.as_str()
        });
    }
    let new_addresses = address_map
        .values()
        .collect::<Vec<&NewAddress>>();

    diesel::insert_into(addresses)
        .values(new_addresses)
        .on_conflict((postcode, number))
        .do_update()
        .set((
            lat.eq(excluded(lat)),
            lon.eq(excluded(lon)),
            street.eq(excluded(street)),
            city.eq(excluded(city)),
            region.eq(excluded(region))
        ))
        .execute(conn)
}

fn create_new_state(
    conn: &PgConnection,
    state_info: &StateInfo
) -> Result<usize, diesel::result::Error> {
    use crate::schema::states::dsl::*;

    let new_state = NewState {
        id: Uuid::new_v4(),
        hash: &state_info.hash,
        version: &state_info.version,
        processed_at: Utc::now().naive_utc()
    };

    diesel::insert_into(states)
        .values(new_state)
        .execute(conn)
}

pub fn get_addresses(
    pool: web::Data<Pool>,
    pcode: &str,
    house_number: Option<&str>
) -> Result<Vec<Address>, diesel::result::Error> {
    use crate::schema::addresses::dsl::*;

    let mut query = addresses.filter(postcode.eq(pcode)).into_boxed();
    if let Some(nb) = house_number {
        query = query.filter(number.ilike(format!("{}%", nb)));
    }

    query
        .order(number.asc())
        .limit(ADDRESSES_RESULT_LIMIT)
        .load(&pool.get().unwrap())
}
