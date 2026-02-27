use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, params};

use crate::error::AppError;
use crate::models::{DictFindItem, DictItem};

const EMBED_SEED_DB: &[u8] = include_bytes!("../assets/steam.db");

#[derive(Debug, Clone, Copy)]
pub enum DictKind {
    Tags,
    Genres,
    Categories,
}

impl DictKind {
    fn table(self) -> &'static str {
        match self {
            Self::Tags => "tags",
            Self::Genres => "genres",
            Self::Categories => "categories",
        }
    }

    fn fts_table(self) -> &'static str {
        match self {
            Self::Tags => "tags_fts",
            Self::Genres => "genres_fts",
            Self::Categories => "categories_fts",
        }
    }
}

pub struct LocalStore {
    conn: Connection,
}

impl LocalStore {
    pub fn open() -> Result<Self, AppError> {
        let mut db_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Internal("home directory not found".to_string()))?;
        db_dir.push(".steam-cli-rs");
        fs::create_dir_all(&db_dir).map_err(|e| AppError::Internal(e.to_string()))?;

        let mut db_path = db_dir;
        db_path.push("steam.db");

        if !db_path.exists() {
            fs::write(&db_path, EMBED_SEED_DB).map_err(|e| AppError::Internal(e.to_string()))?;
        }

        let conn = Connection::open(PathBuf::from(db_path))?;
        let store = Self { conn };
        store.init_schema()?;
        store.ensure_seeded()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), AppError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tags(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS genres(id TEXT PRIMARY KEY, name TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS categories(id INTEGER PRIMARY KEY, name TEXT NOT NULL);

            CREATE VIRTUAL TABLE IF NOT EXISTS tags_fts USING fts5(id UNINDEXED, name);
            CREATE VIRTUAL TABLE IF NOT EXISTS genres_fts USING fts5(id UNINDEXED, name);
            CREATE VIRTUAL TABLE IF NOT EXISTS categories_fts USING fts5(id UNINDEXED, name);

            CREATE TABLE IF NOT EXISTS app_cache(
                appid INTEGER PRIMARY KEY,
                payload_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn ensure_seeded(&self) -> Result<(), AppError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;

        if count == 0 {
            self.seed_from_embedded_db()?;
        }
        Ok(())
    }

    fn seed_from_embedded_db(&self) -> Result<(), AppError> {
        let mut seed_path = std::env::temp_dir();
        seed_path.push(format!("steam-seed-{}.db", std::process::id()));

        fs::write(&seed_path, EMBED_SEED_DB).map_err(|e| AppError::Internal(e.to_string()))?;

        let escaped = seed_path.to_string_lossy().replace('\'', "''");
        let sql = format!(
            "
            ATTACH DATABASE '{escaped}' AS seed;

            DELETE FROM tags;
            DELETE FROM genres;
            DELETE FROM categories;
            DELETE FROM tags_fts;
            DELETE FROM genres_fts;
            DELETE FROM categories_fts;

            INSERT INTO tags(id, name) SELECT id, name FROM seed.tags;
            INSERT INTO genres(id, name) SELECT id, name FROM seed.genres;
            INSERT INTO categories(id, name) SELECT id, name FROM seed.categories;
            INSERT INTO tags_fts(id, name) SELECT id, name FROM seed.tags_fts;
            INSERT INTO genres_fts(id, name) SELECT id, name FROM seed.genres_fts;
            INSERT INTO categories_fts(id, name) SELECT id, name FROM seed.categories_fts;

            DETACH DATABASE seed;
            "
        );

        let result = self.conn.execute_batch(&sql);
        let _ = fs::remove_file(&seed_path);
        result.map_err(AppError::from)
    }

    pub fn list_dict(
        &self,
        kind: DictKind,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<DictItem>, usize), AppError> {
        let table = kind.table();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT CAST(id AS TEXT) AS id, name FROM {} ORDER BY name ASC LIMIT ? OFFSET ?",
            table
        ))?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(DictItem {
                id: row.get::<_, String>(0)?,
                name: row.get(1)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }

        let total: usize =
            self.conn
                .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |row| {
                    row.get(0)
                })?;

        Ok((out, total))
    }

    pub fn find_dict(
        &self,
        kind: DictKind,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<DictFindItem>, usize), AppError> {
        let fts = kind.fts_table();
        let q = to_fts_query(query);
        let mut out = Vec::new();
        let mut total = 0usize;

        if !q.is_empty() {
            let sql = format!(
                "SELECT id, name, bm25({}) as rank FROM {} WHERE {} MATCH ? ORDER BY rank LIMIT ? OFFSET ?",
                fts, fts, fts
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(params![q, limit as i64, offset as i64], |row| {
                Ok(DictFindItem {
                    id: row.get::<_, String>(0)?,
                    name: row.get(1)?,
                    rank: row.get(2)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }

            let count_sql = format!("SELECT COUNT(*) FROM {} WHERE {} MATCH ?", fts, fts);
            total = self
                .conn
                .query_row(&count_sql, params![to_fts_query(query)], |row| row.get(0))?;
        }

        if out.is_empty() {
            let table = kind.table();
            let normalized_query = query
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();
            let mut stmt = self.conn.prepare(&format!(
                "SELECT CAST(id AS TEXT), name FROM {} WHERE REPLACE(REPLACE(LOWER(name), '-', ''), ' ', '') LIKE ? ORDER BY name ASC LIMIT ? OFFSET ?",
                table
            ))?;
            let rows = stmt.query_map(
                params![
                    format!("%{}%", normalized_query),
                    limit as i64,
                    offset as i64
                ],
                |row| {
                    Ok(DictFindItem {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        rank: 1_000.0,
                    })
                },
            )?;
            for row in rows {
                out.push(row?);
            }
            total = self.conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM {} WHERE REPLACE(REPLACE(LOWER(name), '-', ''), ' ', '') LIKE ?",
                    table
                ),
                params![format!("%{}%", normalized_query)],
                |row| row.get(0),
            )?;
        }

        Ok((out, total))
    }

    pub fn get_cached_app(
        &self,
        appid: i64,
        min_fetched_at: i64,
    ) -> Result<Option<String>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload_json FROM app_cache WHERE appid = ? AND fetched_at >= ?")?;
        let mut rows = stmt.query(params![appid, min_fetched_at])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    pub fn put_cached_app(
        &self,
        appid: i64,
        payload_json: &str,
        fetched_at: i64,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO app_cache(appid, payload_json, fetched_at) VALUES(?, ?, ?) ON CONFLICT(appid) DO UPDATE SET payload_json = excluded.payload_json, fetched_at = excluded.fetched_at",
            params![appid, payload_json, fetched_at],
        )?;
        Ok(())
    }
}

fn to_fts_query(input: &str) -> String {
    let mut terms = Vec::new();
    let normalized = input
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>();
    for term in normalized.split_whitespace() {
        let clean = term.replace('"', "");
        if !clean.is_empty() {
            terms.push(format!("{}*", clean));
        }
    }
    terms.join(" AND ")
}
