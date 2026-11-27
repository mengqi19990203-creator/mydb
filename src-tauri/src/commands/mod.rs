mod ai;
mod backup;
mod connections;
mod context;
mod data;
mod data_sync;
mod database;
mod docs;
mod explain;
mod export;
mod generate;
mod history;
mod import_wizard;
mod mongo;
mod mongo_console;
mod query;
mod query_cancel;
mod redis;
mod script;
mod search;
mod server;
mod sync;
mod transaction;
mod transfer;
mod window;
mod xlsx;

pub use self::redis::*;
pub use ai::*;
pub use backup::*;
pub use connections::*;
pub use context::*;
pub use data::*;
pub use data_sync::*;
pub use database::*;
pub use docs::*;
pub use explain::*;
pub use export::*;
pub use generate::*;
pub use history::*;
pub use import_wizard::*;
pub use mongo::*;
pub use mongo_console::*;
pub use query::*;
pub use query_cancel::*;
pub use script::*;
pub use search::*;
pub use server::*;
pub use sync::*;
pub use transaction::*;
pub use transfer::*;
pub use window::*;
pub use xlsx::*;

use crate::db::DatabaseDriver;
use crate::db::DatabaseSession;
use crate::ssh::SshTunnel;
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

/// A statement currently running for a query tab. Kept so the frontend can
/// cancel it through another pooled connection without touching the executing
/// session.
#[derive(Clone)]
pub struct RunningQuery {
    pub connection_id: String,
    pub driver_kind: String,
    pub backend_id: Option<i64>,
    pub cancel_requested: Arc<AtomicBool>,
    pub started_at: std::time::Instant,
}

#[derive(Default)]
pub struct AppState {
    pub drivers: Mutex<HashMap<String, Arc<dyn DatabaseDriver>>>,
    pub redis_clients: Mutex<HashMap<String, MultiplexedConnection>>,
    /// SSH tunnels forwarding Redis traffic, kept alive with the cached client.
    pub redis_tunnels: Mutex<HashMap<String, SshTunnel>>,
    /// Last Redis logical database selected for a connection, reapplied when
    /// the multiplexed connection is recreated.
    pub redis_databases: Mutex<HashMap<String, i64>>,
    /// MongoDB clients keyed by connection id. The client is cloneable and
    /// already owns its connection pool.
    pub mongo_clients: Mutex<HashMap<String, mongodb::Client>>,
    /// Serialises first-time MongoDB client creation so parallel UI requests
    /// cannot create duplicate SSH tunnels or clients.
    pub mongo_connect_lock: Mutex<()>,
    /// SSH tunnels forwarding MongoDB traffic, kept alive with the cached client.
    pub mongo_tunnels: Mutex<HashMap<String, SshTunnel>>,
    pub passwords: Mutex<HashMap<String, String>>,
    /// Full MongoDB connection strings, kept apart from the database password.
    pub mongo_uri_secrets: Mutex<HashMap<String, String>>,
    pub ssh_passwords: Mutex<HashMap<String, String>>,
    pub ssh_key_passphrases: Mutex<HashMap<String, String>>,
    pub export_cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Running test-data generation operations.
    pub generate_cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Running import wizard operations.
    pub import_cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Statements currently executing, keyed by frontend operation id.
    pub running_queries: Mutex<HashMap<String, RunningQuery>>,
    /// Last QPS counter sample per connection: (taken at, counter value).
    pub server_samples: Mutex<HashMap<String, (std::time::Instant, u64)>>,
    /// Serialises backup jobs: at most one backup runs at a time.
    pub backup_running: Mutex<bool>,
    /// Interactive SQL-editor transactions keyed by query tab id.
    pub transactions: Mutex<HashMap<String, TransactionEntry>>,
    /// Standalone query windows keyed by window label, mapped to the
    /// connection they use. The driver keeps one active database / schema per
    /// connection, so scope changes are blocked while one of these windows is
    /// open to keep every window on the same context.
    pub query_windows: std::sync::Mutex<HashMap<String, String>>,
}

/// Whether a standalone query window currently uses `connection_id`.
pub fn query_window_uses_connection(state: &AppState, connection_id: &str) -> bool {
    state
        .query_windows
        .lock()
        .map(|windows| windows.values().any(|id| id == connection_id))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_query_windows_lock_only_their_connection() {
        let state = AppState::default();
        assert!(!query_window_uses_connection(&state, "conn-a"));
        state
            .query_windows
            .lock()
            .unwrap()
            .insert("query-1".to_string(), "conn-a".to_string());
        assert!(query_window_uses_connection(&state, "conn-a"));
        assert!(!query_window_uses_connection(&state, "conn-b"));
        state.query_windows.lock().unwrap().remove("query-1");
        assert!(!query_window_uses_connection(&state, "conn-a"));
    }
}
