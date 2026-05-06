use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::SqliteStore;

pub(super) fn with_sqlite_store(label: &str, exercise: impl FnOnce(&mut SqliteStore)) {
    let path = test_db_path(label);
    let mut store = SqliteStore::open(&path).expect("store should open");
    exercise(&mut store);
    drop(store);
    let _ = std::fs::remove_file(path);
}

fn test_db_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("taugentic-sqlite-store-{label}-{nanos}.sqlite3"))
}
