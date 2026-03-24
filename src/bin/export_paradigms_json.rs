use std::path::PathBuf;

use anyhow::Result;

#[allow(dead_code)]
#[path = "../db_sqlite.rs"]
mod db;
#[allow(dead_code)]
#[path = "../models.rs"]
mod models;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../quiz.rs"]
mod quiz;

fn main() -> Result<()> {
    let output_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/paradigms.json"));

    let database = db::Database::open_default()?;
    database.write_json_to_path(&output_path)?;

    println!("exported paradigm snapshot: {}", output_path.display());
    Ok(())
}