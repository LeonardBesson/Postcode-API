use std::collections::HashMap;

use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use uuid::Uuid;

use crate::data::models::{Address, AddressRecord, NewAddress};
use crate::db::Pool;

const ADDRESSES_RESULT_LIMIT: i64 = 200;

pub fn get_addresses(
    pool: &Pool,
    pcode: &str,
    house_number: Option<&str>
) -> Result<Vec<Address>, diesel::result::Error> {
    use crate::data::schema::addresses::dsl::*;

    let mut query = addresses.filter(postcode.eq(pcode)).into_boxed();
    if let Some(nb) = house_number {
        query = query.filter(number.ilike(format!("{}%", nb)));
    }

    query
        .order(number.asc())
        .limit(ADDRESSES_RESULT_LIMIT)
        .load(&pool.get().unwrap())
}

pub fn create_or_update_addresses<'a>(
    conn: &PgConnection,
    records: &[AddressRecord]
) -> Result<usize, diesel::result::Error> {
    use crate::data::schema::addresses::dsl::*;

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
