use std::collections::HashMap;
use std::fmt::Formatter;
use std::fs::File;
use std::io::Read;

use actix::{System, SystemRunner};
use actix_web::client::Client;
use actix_web::web;
use actix_web::web::Bytes;
use chrono::{NaiveDateTime, Utc};
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use diesel::sql_types::Bool;
use futures::{failed, Future};
use futures::future::{Either};
use indicatif::ProgressBar;
use log::{error, info};
use regex::Regex;
use uuid::Uuid;
use zip::ZipArchive;

use crate::data::RefreshError::{NoData, OldData};
use crate::db::Pool;
use crate::models::{Address, NewAddress, NewState, State};
use crate::postcode::AddressRecord;

const STATE_BODY_LIMIT_BYTES: usize = 2_097_152; // 2MB
const ZIP_BODY_LIMIT_BYTES: usize = 1_074_000_000; // 1GB

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
impl From<actix_web::client::SendRequestError> for RefreshError {
    fn from(error: actix_web::client::SendRequestError) -> Self {
        OldData
    }
}

// TODO
impl From<actix_web::client::PayloadError> for RefreshError {
    fn from(error: actix_web::client::PayloadError) -> Self {
        OldData
    }
}

// TODO
impl From<diesel::result::Error> for RefreshError {
    fn from(error: diesel::result::Error) -> Self {
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

pub struct StateInfo {
    // hash + url
    pub info: Option<(usize, String, String)>, // TODO: use struct
    pub current_state: Option<State>
}

pub fn refresh_state(
    system: &mut SystemRunner,
    pool: &Pool
) {
    let status = system.block_on(futures::lazy(|| { get_state_info(pool) }));
    match status {
        Ok(state_info) => {
            match state_info.info {
                Some((count, url, state_hash)) => {
                    let up_to_date = state_info
                        .current_state
                        .filter(|s| s.hash == state_hash)
                        .is_some();

                    if up_to_date {
                        info!("Data already up to date (state: {})", state_hash);
                    } else {
                        info!("Updating data...");
                        match system.block_on(futures::lazy(|| { update_state(pool, count, url, state_hash) })) {
                            Ok(_) => { info!("Successfuly updated data"); },
                            Err(err) => { error!("Error while updating state: {}", err); },
                        };
                    }
                },
                None => {
                    if state_info.current_state.is_none() {
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

/// Returns true if the state was updated
fn get_state_info<'a>(pool: &'a Pool) -> impl Future<Item = StateInfo, Error = RefreshError> + 'a {
    Client::default()
        .get("http://results.openaddresses.io/state.txt")
        .send()
        .from_err()
        .and_then(move |mut resp| {
            resp.body()
                .limit(STATE_BODY_LIMIT_BYTES)
                .from_err()
                .map(move |body| {
                    let info = get_info(body);
                    let connection = pool.get().unwrap();
                    let current_state = current_state(&connection);
                    StateInfo { info, current_state }
                })
        })
}

fn get_info(body: Bytes) -> Option<(usize, String, String)> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(&body[..]);

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
            // TODO: we might want to use the version instead of hash because it seems like the
            // same version can be re-uploaded multiple times on http://results.openaddresses.io/
            (r[4].parse::<usize>().unwrap(), r[8].to_owned(), r[10].to_owned())
        })
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

fn update_state<'a>(
    pool: &'a Pool,
    address_count: usize,
    url: String,
    state_hash: String,
) -> impl Future<Item = (), Error = RefreshError> + 'a {
    info!("Downloading state from {}", url);

    Client::default()
        .get(url)
        .send()
        .map_err(RefreshError::from)
        .and_then(move |mut resp| {
            resp.body()
                .limit(ZIP_BODY_LIMIT_BYTES)
                .from_err()
                .map(move |body| {
                    info!("Downloaded zip, size: {} MB", body.len() / 1_000_000);
                    info!("Searching for csv file");

                    let mut reader = std::io::Cursor::new(body);
                    let mut zip = ZipArchive::new(reader).expect("Could not create zip archive");
                    let re = Regex::new(r"nl.*\.csv").expect("Could not create regex");
                    for i in 0..zip.len()
                    {
                        let file = zip.by_index(i).unwrap();
                        info!("Filename: {}", file.name());
                        if re.is_match(file.name()) {
                            info!("Found csv file");
                            info!("Updating database records...");
                            let conn = pool.get().unwrap();
                            let mut reader = csv::Reader::from_reader(file);

                            let mut batch = Vec::<AddressRecord>::with_capacity(BATCH_SIZE);
                            let progress_bar = ProgressBar::new(address_count as u64);
                            for record in reader.deserialize() {
                                let address_record: AddressRecord = record.expect("Could not deserialize post code record");
                                batch.push(address_record);
                                if batch.len() == BATCH_SIZE {
                                    process_batch(&conn, &mut batch, &progress_bar);
                                }
                            };
                            process_batch(&conn, &mut batch, &progress_bar);
                            progress_bar.finish();

                            create_new_state(&conn, &state_hash);
                            info!("Done");
                            break;
                        }
                    }

                    ()
                })
        })
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

fn create_or_update_addresses<'a>(
    conn: &PgConnection,
    records: &Vec<AddressRecord>
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
            lon: record.lon as f64,
            lat: record.lat as f64,
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

fn create_new_state(conn: &PgConnection, state_hash: &str) {
    use crate::schema::states::dsl::*;

    let new_state = NewState {
        id: Uuid::new_v4(),
        hash: state_hash,
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
        .limit(ADDRESSES_RESULT_LIMIT)
        .load(&pool.get().unwrap())
}

// TODO: try to compose get_state_info and update_state
// Chaining futures with different return type is a pain
// Apparently Box<dyn Future<blabla>> can fix the issue
// But I couldn't get it to compile.
// Check "Returning from multiple branches" from https://tokio.rs/docs/futures/combinators/#use-impl-future