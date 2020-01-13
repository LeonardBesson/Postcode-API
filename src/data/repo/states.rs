use chrono::Utc;
use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::data::models::{NewState, State};
use crate::data::state::StateInfo;

pub fn current_state(conn: &PgConnection) -> Result<Option<State>, diesel::result::Error> {
    use crate::data::schema::states::dsl::*;

    states
        .order(processed_at.desc())
        .limit(1)
        .first(conn)
        .optional()
}

pub fn create_new_state(
    conn: &PgConnection,
    state_info: &StateInfo
) -> Result<usize, diesel::result::Error> {
    use crate::data::schema::states::dsl::*;

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

