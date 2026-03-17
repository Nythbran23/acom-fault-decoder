// ============================================================================
// database.rs — Remote database connection (stub / future implementation).
//
// TODO: Define schema and implement before enabling.
// Suggested tables:
//   - captures(id, timestamp, callsign, serial_number, lines TEXT[4])
//   - faults(capture_id, code, condition, fault_type)
//   - parameters(capture_id, key, value, unit)
//
// Suggested approach: sqlx with SQLite for local caching + optional sync
// to a remote Postgres instance hosted at The DX Shop.
//
// To enable:
//   1. Add to Cargo.toml:
//        sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio", "macros"] }
//   2. Set DATABASE_URL env var or embed path in tauri.conf.json
//   3. Run `sqlx migrate run`
// ============================================================================

#[allow(dead_code)]
pub struct Database;

#[allow(dead_code)]
impl Database {
    pub async fn connect(_url: &str) -> Result<Self, String> {
        // TODO: sqlx::SqlitePool::connect(url).await
        Err("Database not yet implemented".into())
    }

    pub async fn save_capture(&self, _lines: &[String; 4]) -> Result<i64, String> {
        // TODO: INSERT INTO captures ...
        Err("Not implemented".into())
    }
}
