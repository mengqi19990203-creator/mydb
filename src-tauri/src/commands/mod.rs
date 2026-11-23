mod connections;
mod context;
mod data;
mod data_sync;
mod history;
mod query;
mod redis;
mod sync;
mod transaction;

pub use connections::*;
pub use context::*;
pub use data::*;
pub use data_sync::*;
pub use history::*;
pub use query::*;
pub use self::redis::*;
pub use sync::*;
pub use transaction::*;

use crate::db::DatabaseDriver;
use crate::db::DatabaseSession;
use ::redis::aio::MultiplexedConnection;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct TransactionEntry {
    pub connection_id: String,
    pub session: Arc<Mutex<Box<dyn DatabaseSession>>>,
}

#[derive(Default)]
pub struct AppState {
    pub drivers: Mutex<HashMap<String, Arc<dyn DatabaseDriver>>>,
    pub redis_clients: Mutex<HashMap<String, MultiplexedConnection>>,
    /// Last Redis logical database selected for a connection, reapplied when
    /// the multiplexed connection is recreated.
    pub redis_databases: Mutex<HashMap<String, i64>>,
    pub passwords: Mutex<HashMap<String, String>>,
    pub ssh_passwords: Mutex<HashMap<String, String>>,
    pub export_cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Interactive SQL-editor transactions keyed by query tab id.
    pub transactions: Mutex<HashMap<String, TransactionEntry>>,
}
