use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Local OSM database using SQLite with spatial indexing.
#[derive(Clone)]
pub struct OsmDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl OsmDatabase {
    /// Open or create a local OSM database at the given path.
    pub fn open(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS places (
                id INTEGER PRIMARY KEY,
                osm_type TEXT NOT NULL,
                lat REAL NOT NULL,
                lon REAL NOT NULL,
                name TEXT,
                category TEXT NOT NULL,
                tags TEXT NOT NULL,
                wikidata TEXT,
                wikipedia TEXT,
                website TEXT,
                phone TEXT,
                opening_hours TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_places_lat ON places(lat);
            CREATE INDEX IF NOT EXISTS idx_places_lon ON places(lon);
            CREATE INDEX IF NOT EXISTS idx_places_category ON places(category);",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Insert a place into the database.
    pub fn insert_place(
        &self,
        osm_type: &str,
        osm_id: i64,
        lat: f64,
        lon: f64,
        name: Option<&str>,
        category: &str,
        tags: &str,
        wikidata: Option<&str>,
        wikipedia: Option<&str>,
        website: Option<&str>,
        phone: Option<&str>,
        opening_hours: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO places (id, osm_type, lat, lon, name, category, tags, wikidata, wikipedia, website, phone, opening_hours)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![osm_id, osm_type, lat, lon, name, category, tags, wikidata, wikipedia, website, phone, opening_hours],
        )?;
        Ok(())
    }

    /// Search for places within a radius of given coordinates.
    pub fn search_nearby(
        &self,
        lat: f64,
        lon: f64,
        radius_m: u32,
        limit: usize,
    ) -> SqlResult<Vec<LocalPlace>> {
        let conn = self.conn.lock().unwrap();
        let lat_delta = radius_m as f64 / 111_000.0;
        let lon_delta = radius_m as f64 / (111_000.0 * lat.to_radians().cos().abs().max(0.01));

        let mut stmt = conn.prepare(
            "SELECT id, osm_type, lat, lon, name, category, tags, wikidata, wikipedia, website, phone, opening_hours
             FROM places
             WHERE lat BETWEEN ?1 AND ?2 AND lon BETWEEN ?3 AND ?4
             LIMIT ?5",
        )?;

        let limit_i64 = (limit * 4) as i64;
        let rows = stmt.query_map(
            rusqlite::params![
                lat - lat_delta,
                lat + lat_delta,
                lon - lon_delta,
                lon + lon_delta,
                limit_i64,
            ],
            |row| {
                Ok(LocalPlace {
                    id: row.get(0)?,
                    osm_type: row.get(1)?,
                    lat: row.get(2)?,
                    lon: row.get(3)?,
                    name: row.get(4)?,
                    category: row.get(5)?,
                    tags: row.get(6)?,
                    wikidata: row.get(7)?,
                    wikipedia: row.get(8)?,
                    website: row.get(9)?,
                    phone: row.get(10)?,
                    opening_hours: row.get(11)?,
                })
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            let place = row?;
            let dist = haversine(lat, lon, place.lat, place.lon);
            if dist <= radius_m as f64 {
                results.push((place, dist));
            }
        }
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results.into_iter().map(|(p, _)| p).collect())
    }

    /// Get total place count.
    pub fn count(&self) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM places", [], |row| row.get(0))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalPlace {
    pub id: i64,
    pub osm_type: String,
    pub lat: f64,
    pub lon: f64,
    pub name: Option<String>,
    pub category: String,
    pub tags: String,
    pub wikidata: Option<String>,
    pub wikipedia: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub opening_hours: Option<String>,
}

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371000.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    R * c
}
