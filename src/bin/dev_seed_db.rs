use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tags_json = std::fs::read_to_string("assets/tags.popular.en.json")?;
    let genres_json = std::fs::read_to_string("assets/genres.json")?;
    let categories_json = std::fs::read_to_string("assets/categories.json")?;

    let tags: Value = serde_json::from_str(&tags_json)?;
    let genres: Value = serde_json::from_str(&genres_json)?;
    let categories: Value = serde_json::from_str(&categories_json)?;

    let out_path = Path::new("assets/steam.db");
    if out_path.exists() {
        std::fs::remove_file(out_path)?;
    }

    let conn = Connection::open(out_path)?;
    conn.execute_batch(
        "
        CREATE TABLE tags(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE genres(id TEXT PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE categories(id INTEGER PRIMARY KEY, name TEXT NOT NULL);

        CREATE VIRTUAL TABLE tags_fts USING fts5(id UNINDEXED, name);
        CREATE VIRTUAL TABLE genres_fts USING fts5(id UNINDEXED, name);
        CREATE VIRTUAL TABLE categories_fts USING fts5(id UNINDEXED, name);

        CREATE TABLE app_cache(
            appid INTEGER PRIMARY KEY,
            payload_json TEXT NOT NULL,
            fetched_at INTEGER NOT NULL
        );
        ",
    )?;

    let tx = conn.unchecked_transaction()?;

    if let Value::Array(items) = tags {
        for item in items {
            let id = item
                .get("tagid")
                .and_then(|v| v.as_i64())
                .ok_or("tagid missing")?;
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("tag name missing")?;

            tx.execute("INSERT INTO tags(id, name) VALUES(?, ?)", params![id, name])?;
            tx.execute(
                "INSERT INTO tags_fts(id, name) VALUES(?, ?)",
                params![id.to_string(), name],
            )?;
        }
    } else {
        return Err("tags payload is not array".into());
    }

    insert_map_dict(&tx, "genres", "genres_fts", &genres)?;
    insert_map_dict(&tx, "categories", "categories_fts", &categories)?;

    tx.commit()?;

    let tags_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?;
    let genres_count: i64 = conn.query_row("SELECT COUNT(*) FROM genres", [], |r| r.get(0))?;
    let categories_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;

    println!(
        "seed db generated at assets/steam.db (tags={}, genres={}, categories={})",
        tags_count, genres_count, categories_count
    );

    Ok(())
}

fn insert_map_dict(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    fts_table: &str,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{} payload is not object", table))?;

    for (id, name_value) in obj {
        let name = name_value
            .as_str()
            .ok_or_else(|| format!("{} name value invalid", table))?;

        tx.execute(
            &format!("INSERT INTO {}(id, name) VALUES(?, ?)", table),
            params![id, name],
        )?;
        tx.execute(
            &format!("INSERT INTO {}(id, name) VALUES(?, ?)", fts_table),
            params![id, name],
        )?;
    }

    Ok(())
}
