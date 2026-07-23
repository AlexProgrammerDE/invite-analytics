pub mod create;
pub mod delete;
pub mod export;
pub mod graph;
pub mod import;
pub mod links;
pub mod lookup;
pub mod set;
pub mod stats;

use poise::Command;

use crate::Error;
use crate::state::BotData;

pub fn all() -> Vec<Command<BotData, Error>> {
    vec![
        create::create(),
        links::links(),
        lookup::lookup(),
        graph::graph(),
        set::r#set(),
        import::import(),
        export::export(),
        stats::stats(),
        delete::delete(),
    ]
}
