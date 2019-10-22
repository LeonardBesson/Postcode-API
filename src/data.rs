use std::collections::HashMap;
use std::fmt::Formatter;

use actix_web::web;
use chrono::Utc;
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use futures::Future;
use indicatif::ProgressBar;
use log::{error, info};
use regex::Regex;
use uuid::Uuid;
use zip::ZipArchive;

use crate::data::RefreshError::{NoData, OldData};
use crate::db::Pool;
use crate::models::{Address, AddressRecord, NewAddress, NewState, State};

const APPROXIMATE_ZIP_SIZE_BYTES: usize = 200_097_152; // 200 MB

const BATCH_SIZE: usize = 2500;

const ADDRESSES_RESULT_LIMIT: i64 = 200;

#[derive(Debug)]
pub enum RefreshError {
    /// Couldn't fetch data and there is no old data to fallback to
    NoData,
    /// Couldn't fetch new data but there is old data to fallback to
    OldData,

    // TODO: add one with cause messsage
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let msg = match self {
            NoData => { "Error refresh state. No fallback data" },
            OldData => { "Error refresh state. Fallback data available" },
        };
        write!(f, "{}", msg)
    }
}

// TODO
impl From<diesel::result::Error> for RefreshError {
    fn from(_error: diesel::result::Error) -> Self {
//        use diesel::result::Error;
//
//        match error {
//            Error::InvalidCString(_) => {},
//            Error::DatabaseError(_, _) => {},
//            Error::NotFound => {},
//            Error::QueryBuilderError(_) => {},
//            Error::DeserializationError(_) => {},
//            Error::SerializationError(_) => {},
//            Error::RollbackTransaction => {},
//            Error::AlreadyInTransaction => {},
//            Error::__Nonexhaustive => {},
//        }

        OldData
    }
}

impl From<reqwest::Error> for RefreshError {
    fn from(_error: reqwest::Error) -> Self {
        OldData
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
pub struct StateRefresh {
    pub state_info: Option<StateInfo>,
    pub current_state: Option<State>
}

pub fn refresh_state(pool: &PgConnection) {
    let status = get_state_refresh(pool);
    match status {
        Ok(state_refresh) => {
            match state_refresh.state_info {
                Some(state_info) => {
                    let up_to_date = state_refresh
                        .current_state
                        .filter(|s| s.version == state_info.version)
                        .is_some();

                    if up_to_date {
                        info!("Data already up to date (state: {})", state_info.version);
                    } else {
                        info!("Updating data...");
                        match update_state(pool, state_info) {
                            Ok(_) => { info!("Successfuly updated data"); },
                            Err(err) => { error!("Error while updating state: {}", err); },
                        };
                    }
                },
                None => {
                    if state_refresh.current_state.is_none() {
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
}

fn get_info<R: std::io::Read>(response: R) -> Option<StateInfo> {
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

pub fn get_state_refresh(conn: &PgConnection) -> Result<StateRefresh, RefreshError> {
    let response = reqwest::get("http://results.openaddresses.io/state.txt")?;
    let state_info = get_info(response);
    let current_state = current_state(&conn);
    Ok(StateRefresh { state_info, current_state })
}

fn update_state(
    conn: &PgConnection,
    state_info: StateInfo
) -> Result<(), RefreshError> {
    info!("Downloading state version {} from {}", state_info.version, state_info.url);

    let mut resp = reqwest::get(&state_info.url)?;
    let mut buf: Vec<u8> = Vec::with_capacity(APPROXIMATE_ZIP_SIZE_BYTES);
    resp.copy_to(&mut buf)?;
    info!("Downloaded zip, size: {} MB", buf.len() / 1_000_000);
    info!("Searching for csv file");

    let reader = std::io::Cursor::new(&buf);
    let mut zip = ZipArchive::new(reader).expect("Could not create zip archive");
    let re = Regex::new(r"nl.*\.csv").expect("Could not create regex");
    for i in 0..zip.len() {
        let file = zip.by_index(i).unwrap();
        info!("File: {}", file.name());
        if re.is_match(file.name()) {
            info!("Found csv file");
            info!("Updating database records...");
            let mut reader = csv::Reader::from_reader(file);

            let mut batch = Vec::<AddressRecord>::with_capacity(BATCH_SIZE);
            let progress_bar = ProgressBar::new(state_info.address_count as u64);
            for record in reader.deserialize() {
                let address_record: AddressRecord = record.expect("Could not deserialize post code record");
                batch.push(address_record);
                if batch.len() == BATCH_SIZE {
                    process_batch(&conn, &mut batch, &progress_bar);
                }
            };
            process_batch(&conn, &mut batch, &progress_bar);
            progress_bar.finish();

            create_new_state(&conn, &state_info);
            info!("Done");
            break;
        }
    }

    Ok(())
}

fn current_state(connection: &PgConnection) -> Option<State> {
    use crate::schema::states::dsl::*;

    states
        .order(processed_at.desc())
        .limit(1)
        .first(connection)
        .optional()
        .unwrap_or(None)
}

fn process_batch(
    conn: &PgConnection,
    batch: &mut Vec<AddressRecord>,
    progress_bar: &ProgressBar
) {
    create_or_update_addresses(&conn, &batch);
    progress_bar.inc(batch.len() as u64);
    batch.clear();
}

pub fn create_or_update_addresses<'a>(
    conn: &PgConnection,
    records: &[AddressRecord]
) {
    use crate::schema::addresses::dsl::*;

    // Filter duplicates because the same address sometimes have different coordinates, for example:
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
        .map_err(|err|
            error!("Error saving new address: {}", err)
        );
}

fn create_new_state(conn: &PgConnection, state_info: &StateInfo) {
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
        .map_err(|err|
            error!("Could not create new state: {}", err)
        );
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
