//! JSONL export in the T4 line format (`RecordedEvent` per line).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::error::SessionError;
use crate::paging::decrypt_payload;
use crate::session::SessionHandle;

impl SessionHandle<'_> {
    /// Exports the session to `{session}/events.jsonl` in the T4 line
    /// format. Returns the file path.
    ///
    /// # Errors
    /// Typed: decrypt (encrypted sessions must be unlocked), IO.
    pub fn export_jsonl(&self) -> Result<PathBuf, SessionError> {
        let path = self
            .store
            .layout()
            .session_dir(&self.meta.id)
            .join("events.jsonl");
        self.export_jsonl_to(&path)?;
        Ok(path)
    }

    /// Exports to an explicit path. Returns the number of lines written.
    ///
    /// # Errors
    /// Same typed conditions as [`Self::export_jsonl`].
    pub fn export_jsonl_to(&self, path: &std::path::Path) -> Result<u64, SessionError> {
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT payload FROM events WHERE session_id = ?1 ORDER BY seq")
            .map_err(SessionError::sqlite)?;
        let payloads = stmt
            .query_map([&self.meta.id], |row| row.get::<_, Vec<u8>>(0))
            .map_err(SessionError::sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::sqlite)?;
        drop(stmt);
        drop(conn);
        let mut file = fs::File::create(path).map_err(SessionError::io)?;
        let mut written = 0u64;
        for payload in payloads {
            let bytes = decrypt_payload(self, &payload)?;
            file.write_all(&bytes).map_err(SessionError::io)?;
            file.write_all(b"\n").map_err(SessionError::io)?;
            written += 1;
        }
        file.flush().map_err(SessionError::io)?;
        Ok(written)
    }
}
