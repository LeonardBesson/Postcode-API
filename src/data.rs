use futures::Future;
use actix_web::Error;
use actix_web::client::Client;
use log::info;

pub fn fetch_status() -> impl Future<Item = String, Error = Error> {
    Client::default()
        .get("http://results.openaddresses.io/state.txt")
        .send()
        .map_err(Error::from)
        .and_then(|mut resp| {
            resp.body()
                .limit(1_048_576)
                .from_err()
                .map(|body| {
                    let mut reader = csv::ReaderBuilder::new()
                        .delimiter(b'\t')
                        .from_reader(&body[..]);

                    let data_info = reader
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
                        });

                    if let Some((hash, url)) = data_info {
                        info!("(hash, url): ({}, {})", hash, url);
                    }

                    String::from_utf8(body.to_vec()).unwrap()
                })
        })
}