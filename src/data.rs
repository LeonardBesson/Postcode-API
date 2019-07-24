use actix_web::client::Client;
use diesel::prelude::*;
use futures::{Future, failed};
use log::{error, info};

use crate::*;
use crate::db::establish_connection;
use crate::models::State;
use crate::schema::states::dsl::*;
use crate::data::RefreshError::{OldData, NoData};
use std::fmt::Formatter;
use futures::future::{err, ok, Either};
use actix_web::web::Bytes;

#[derive(Debug)]
pub enum RefreshError {
    /// Couldn't fetch data and there is no old data to fallback to
    NoData,
    /// Couldn't fetch new data but there is old data to fallback to
    OldData,
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
    pub info: Option<(String, String)>,
    pub current_state: Option<State>
}

/// Returns true if the state was updated
pub fn get_state_info() -> impl Future<Item = StateInfo, Error = RefreshError> {
    Client::default()
        .get("http://results.openaddresses.io/state.txt")
        .send()
        .map_err(RefreshError::from)
        .and_then(|mut resp| {
            resp.body()
                .limit(1_048_576)
                .from_err()
                .map(|body| {
                    let info = extract_state_url(body);
                    let connection = establish_connection();
                    let current_state = current_state(&connection);

                    StateInfo { info, current_state }
                })
        })
}

fn extract_state_url(body: Bytes) -> Option<(String, String)> {
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
            (r[8].to_string(), r[10].to_string())
        })
}

fn current_state(connection: &PgConnection) -> Option<State> {
    states
        .order(processed_at.desc())
        .limit(1)
        .first(connection)
        .optional()
        .unwrap_or(None)
}

pub fn update_state(
    state_hash: String,
    url: String
) -> impl Future<Item = (), Error = data::RefreshError> {
    Client::default()
        .get(url)
        .send()
        .map_err(RefreshError::from)
        .and_then(|mut resp| {
            resp.body()
                .limit(1_048_576_000_000)
                .from_err()
                .map(|body| {
                    info!("body: {:#?}", body);
                    ()
                })
        })
}


// TODO: try to compose get_state_info and update_state
// Chaining futures with different return type is a pain
// Apparently Box<dyn Future<blabla>> can fix the issue
// But I couldn't get it to compile.
// Check "Returning from multiple branches" from https://tokio.rs/docs/futures/combinators/#use-impl-future

//pub fn refresh_state() -> impl Future<Item = bool, Error = RefreshError> {
//    Client::default()
//        .get("http://results.openaddresses.io/state.txt")
//        .send()
//        .map_err(|e| Err(RefreshError::from(e)))
//        .and_then(|mut resp| {
//            resp.body()
//                .limit(1_048_576)
//                .from_err()
//                .map_err(|e| Err(e))
//                .map(|body| {
//                    let state_info = state_info(body);
//                    let connection = establish_connection();
//                    let current_state = current_state(&connection);
//
//                    match state_info {
//                        Some((state_hash, url)) => {
//                            info!("(hash, url): ({}, {})", state_hash, url);
//                            if let Some(state) = current_state {
//                                if state.hash == state_hash {
//                                    info!("State is up to date");
//                                    Ok(false)
//                                } else {
//                                    update_state(state_hash, url);
//                                    Ok(true)
//                                }
//                            } else {
//                                Ok(false)
//
//                            }
//                        },
//                        None => {
//                            match current_state {
//                                Some(_) => { Err(OldData) },
//                                None => { Err(NoData) },
//                            }
//                        },
//                    }
//                })
//        })
//}

//pub fn refresh_state() -> Box<dyn Future<Item = bool, Error = RefreshError>> {
//    Box::new(Client::default()
//        .get("http://results.openaddresses.io/state.txt")
//        .send()
//        .map_err(|e| err(RefreshError::from(e)))
//        .and_then(|mut resp| {
//            Box::new(resp.body()
//                .limit(1_048_576)
//                .from_err()
//                .map_err(|e| err(e))
//                .map(|body| {
//                    let state_info = state_info(body);
//                    let connection = establish_connection();
//                    let current_state = current_state(&connection);
//
//                    match state_info {
//                        Some((state_hash, url)) => {
//                            info!("(hash, url): ({}, {})", state_hash, url);
//                            if let Some(state) = current_state {
//                                if state.hash == state_hash {
//                                    info!("State is up to date");
//                                    ok(false)
//                                } else {
//                                    update_state(state_hash, url);
//                                    ok(true)
//                                }
//                            } else {
//                                ok(false)
//                            }
//                        },
//                        None => {
//                            match current_state {
//                                Some(_) => { err(OldData) },
//                                None => { err(NoData) },
//                            }
//                        },
//                    }
//                }))
//        }))
//}
