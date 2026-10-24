use crate::models::*;
use rusqlite::{Connection, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

pub struct Store {
    pub path: PathBuf,
}
impl Store {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let store = Self { path };
        let conn = store.conn()?;
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS records (kind TEXT NOT NULL, id TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(kind,id)); CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, at TEXT NOT NULL, data TEXT NOT NULL);").map_err(|e| e.to_string())?;
        // A crash may interrupt side effects. Never automatically replay an executing step.
        Ok(store)
    }
    fn conn(&self) -> Result<Connection, String> {
        let c = Connection::open(&self.path).map_err(|e| e.to_string())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        Ok(c)
    }
    pub fn put<T: Serialize>(&self, kind: &str, id: &str, data: &T) -> Result<(), String> {
        self.conn()?.execute("INSERT INTO records VALUES (?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET data=excluded.data", params![kind,id,serde_json::to_string(data).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<T, String> {
        let data: String = self
            .conn()?
            .query_row(
                "SELECT data FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
    pub fn list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>, String> {
        let c = self.conn()?;
        let mut q = c
            .prepare("SELECT data FROM records WHERE kind=?1 ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([kind], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn delete(&self, kind: &str, id: &str) -> Result<(), String> {
        self.conn()?
            .execute(
                "DELETE FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn log(
        &self,
        action: &str,
        target: &str,
        status: &str,
        detail: &str,
    ) -> Result<(), String> {
        let entry = History {
            id: id(),
            at: now(),
            action: action.into(),
            target: target.into(),
            status: status.into(),
            detail: detail.into(),
        };
        self.conn()?
            .execute(
                "INSERT INTO history VALUES (?1,?2,?3)",
                params![
                    entry.id,
                    entry.at,
                    serde_json::to_string(&entry).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<History>, String> {
        let c = self.conn()?;
        let mut q = c
            .prepare("SELECT data FROM history ORDER BY at DESC LIMIT 300")
            .map_err(|e| e.to_string())?;
        q.query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .map(|r| {
                serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
            })
            .collect()
    }
}
