use std::{path::Path, time::SystemTime};

use apppw_core::{HttpRequestInfo, NetworkConnection, ProcessInfo};
use rusqlite::{Connection, Result, params};

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let database = Self {
            connection: Connection::open(path)?,
        };
        database.initialise()?;
        Ok(database)
    }

    pub fn in_memory() -> Result<Self> {
        let database = Self {
            connection: Connection::open_in_memory()?,
        };
        database.initialise()?;
        Ok(database)
    }

    pub fn save_process(&self, process: &ProcessInfo) -> Result<()> {
        self.connection.execute(
            "INSERT INTO processes (pid, name, executable_path)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(pid) DO UPDATE SET
                name = excluded.name,
                executable_path = excluded.executable_path",
            params![
                process.pid,
                process.name,
                process
                    .executable_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
            ],
        )?;
        Ok(())
    }

    pub fn save_connection(&self, connection: &NetworkConnection) -> Result<()> {
        if let Some(process) = &connection.process {
            self.save_process(process)?;
        }
        let id = sqlite_integer(connection.id)?;
        let now = unix_milliseconds(SystemTime::now())?;
        let process_pid = connection.process.as_ref().map(|process| process.pid);
        self.connection.execute(
            "INSERT INTO connections (
                id, process_id, protocol, local_ip, local_port, remote_ip, remote_port,
                hostname, started_at, ended_at, bytes_sent, bytes_received
             ) VALUES (
                ?1, (SELECT id FROM processes WHERE pid = ?2), ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, NULL, ?10, ?11
             )
             ON CONFLICT(id) DO UPDATE SET
                process_id = excluded.process_id,
                protocol = excluded.protocol,
                local_ip = excluded.local_ip,
                local_port = excluded.local_port,
                remote_ip = excluded.remote_ip,
                remote_port = excluded.remote_port,
                hostname = excluded.hostname,
                bytes_sent = excluded.bytes_sent,
                bytes_received = excluded.bytes_received",
            params![
                id,
                process_pid,
                format!("{:?}", connection.protocol),
                connection.local_ip.to_string(),
                connection.local_port,
                connection.remote_ip.to_string(),
                connection.remote_port,
                connection.hostname,
                now,
                sqlite_integer(connection.bytes_sent)?,
                sqlite_integer(connection.bytes_received)?,
            ],
        )?;
        Ok(())
    }

    pub fn save_http_request(&self, request: &HttpRequestInfo) -> Result<()> {
        self.connection.execute(
            "INSERT INTO http_requests (
                id, connection_id, method, scheme, host, path, status, started_at, duration_ms, ja4
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                connection_id = excluded.connection_id,
                method = excluded.method,
                scheme = excluded.scheme,
                host = excluded.host,
                path = excluded.path,
                status = excluded.status,
                duration_ms = excluded.duration_ms,
                ja4 = excluded.ja4",
            params![
                sqlite_integer(request.id)?,
                request.connection_id.map(sqlite_integer).transpose()?,
                request.method,
                request.scheme,
                request.host,
                request.path,
                request.status_code,
                unix_milliseconds(request.started_at)?,
                request.duration_ms.map(sqlite_integer_u128).transpose()?,
                request.ja4,
            ],
        )?;
        Ok(())
    }

    fn initialise(&self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS processes (
                id INTEGER PRIMARY KEY,
                pid INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL,
                executable_path TEXT
             );
             CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY,
                process_id INTEGER REFERENCES processes(id),
                protocol TEXT NOT NULL,
                local_ip TEXT NOT NULL,
                local_port INTEGER NOT NULL,
                remote_ip TEXT NOT NULL,
                remote_port INTEGER NOT NULL,
                hostname TEXT,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                bytes_sent INTEGER NOT NULL,
                bytes_received INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS http_requests (
                id INTEGER PRIMARY KEY,
                connection_id INTEGER REFERENCES connections(id),
                method TEXT NOT NULL,
                scheme TEXT NOT NULL,
                host TEXT NOT NULL,
                path TEXT NOT NULL,
                status INTEGER,
                started_at INTEGER NOT NULL,
                duration_ms INTEGER,
                ja4 TEXT
             );",
        )?;
        let has_ja4: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('http_requests') WHERE name = 'ja4')",
            [],
            |row| row.get(0),
        )?;
        if !has_ja4 {
            self.connection
                .execute("ALTER TABLE http_requests ADD COLUMN ja4 TEXT", [])?;
        }
        Ok(())
    }
}

fn sqlite_integer(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn sqlite_integer_u128(value: u128) -> Result<i64> {
    i64::try_from(value).map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn unix_milliseconds(time: SystemTime) -> Result<i64> {
    let milliseconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
        .as_millis();
    sqlite_integer_u128(milliseconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apppw_core::{ConnectionDirection, IpVersion, NetworkProtocol};

    #[test]
    fn creates_schema_and_upserts_models() {
        let database = Database::in_memory().unwrap();
        let process = ProcessInfo::new(42, "client.exe".into(), None);
        let connection = NetworkConnection {
            id: 7,
            process: Some(process.clone()),
            protocol: NetworkProtocol::TCP,
            ip_version: IpVersion::V4,
            direction: ConnectionDirection::Outbound,
            local_ip: "127.0.0.1".parse().unwrap(),
            local_port: 50_000,
            remote_ip: "93.184.216.34".parse().unwrap(),
            remote_port: 80,
            hostname: Some("example.com".into()),
            bytes_sent: 100,
            bytes_received: 200,
        };
        let request = HttpRequestInfo {
            id: 9,
            connection_id: Some(connection.id),
            process: Some(process.clone()),
            method: "GET".into(),
            scheme: "http".into(),
            host: "example.com".into(),
            path: "/".into(),
            request_headers: Vec::new(),
            request_body: None,
            request_body_size: 0,
            status_code: Some(200),
            response_headers: Vec::new(),
            response_body: None,
            response_body_size: Some(0),
            started_at: SystemTime::now(),
            duration_ms: Some(12),
            ja4: Some("t13d1516h2_example_example".into()),
        };

        database.save_process(&process).unwrap();
        database.save_connection(&connection).unwrap();
        database.save_http_request(&request).unwrap();
        database.save_connection(&connection).unwrap();

        for table in ["processes", "connections", "http_requests"] {
            let count: i64 = database
                .connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn adds_ja4_to_an_existing_database() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute("CREATE TABLE http_requests (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        let database = Database { connection };
        database.initialise().unwrap();
        let has_ja4: bool = database
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('http_requests') WHERE name = 'ja4')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_ja4);
    }
}
