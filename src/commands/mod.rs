pub mod create;
pub mod delete;
pub mod edit;
pub mod export;
pub mod graph;
pub mod health;
pub mod import;
pub mod links;
pub mod lookup;
pub mod retention;
pub mod set;
pub mod stats;
pub(crate) mod support;
pub mod sync;
pub mod target_users;

use poise::Command;

use crate::Error;
use crate::state::BotData;

pub fn all() -> Vec<Command<BotData, Error>> {
    vec![
        create::create(),
        edit::edit(),
        links::links(),
        lookup::lookup(),
        retention::retention(),
        graph::graph(),
        health::health(),
        sync::sync(),
        target_users::target_users(),
        set::r#set(),
        import::import(),
        export::export(),
        stats::stats(),
        delete::delete(),
    ]
}
