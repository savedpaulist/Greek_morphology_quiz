use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};

#[path = "../paradigm.rs"]
mod paradigm;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let input_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/common.db"));
    let output_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/paradigms.sqlite3"));

    let source = Connection::open(&input_path)
        .with_context(|| format!("failed to open source database {}", input_path.display()))?;
    let mut target = Connection::open(&output_path)
        .with_context(|| format!("failed to open output database {}", output_path.display()))?;

    create_schema(&target)?;
    import_paradigms(&source, &mut target)?;

    println!(
        "rebuilt paradigm database: {} -> {}",
        input_path.display(),
        output_path.display()
    );
    Ok(())
}

fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        DROP TABLE IF EXISTS paradigm_cells;
        DROP TABLE IF EXISTS paradigm_rows;
        DROP TABLE IF EXISTS paradigm_tables;
        DROP TABLE IF EXISTS paradigms;

        CREATE TABLE paradigms (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL UNIQUE,
            family TEXT NOT NULL,
            raw_payload TEXT NOT NULL
        );

        CREATE TABLE paradigm_tables (
            id INTEGER PRIMARY KEY,
            paradigm_id INTEGER NOT NULL,
            table_kind TEXT NOT NULL,
            heading TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (paradigm_id) REFERENCES paradigms(id) ON DELETE CASCADE
        );

        CREATE TABLE paradigm_rows (
            id INTEGER PRIMARY KEY,
            table_id INTEGER NOT NULL,
            row_key TEXT NOT NULL,
            row_label TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (table_id) REFERENCES paradigm_tables(id) ON DELETE CASCADE
        );

        CREATE TABLE paradigm_cells (
            id INTEGER PRIMARY KEY,
            row_id INTEGER NOT NULL,
            column_key TEXT NOT NULL,
            column_label TEXT NOT NULL,
            value TEXT NOT NULL,
            raw_value TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            FOREIGN KEY (row_id) REFERENCES paradigm_rows(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_paradigm_tables_paradigm_id ON paradigm_tables(paradigm_id, sort_order);
        CREATE INDEX idx_paradigm_rows_table_id ON paradigm_rows(table_id, sort_order);
        CREATE INDEX idx_paradigm_cells_row_id ON paradigm_cells(row_id, sort_order);
        "#,
    )?;

    Ok(())
}

fn import_paradigms(source: &Connection, target: &mut Connection) -> Result<()> {
    let mut statement = source.prepare("SELECT title, forms FROM forms ORDER BY title COLLATE NOCASE")?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let transaction = target.transaction()?;
    let mut inserted = 0usize;

    for (title, payload) in rows {
        let parsed = match paradigm::parse_paradigm(&title, &payload) {
            Ok(parsed) => parsed,
            Err(error) => {
                return Err(anyhow!("failed to parse paradigm `{title}`: {error}"));
            }
        };

        transaction.execute(
            "INSERT INTO paradigms(title, family, raw_payload) VALUES (?, ?, ?)",
            params![parsed.title, parsed.family.as_str(), parsed.raw_payload],
        )?;
        let paradigm_id = transaction.last_insert_rowid();

        for (table_index, table) in parsed.tables.iter().enumerate() {
            transaction.execute(
                "INSERT INTO paradigm_tables(paradigm_id, table_kind, heading, sort_order) VALUES (?, ?, ?, ?)",
                params![paradigm_id, table.kind, table.heading, table_index as i64],
            )?;
            let table_id = transaction.last_insert_rowid();

            for (row_index, row) in table.rows.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO paradigm_rows(table_id, row_key, row_label, sort_order) VALUES (?, ?, ?, ?)",
                    params![table_id, row.key, row.label, row_index as i64],
                )?;
                let row_id = transaction.last_insert_rowid();

                for (cell_index, cell) in row.cells.iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO paradigm_cells(row_id, column_key, column_label, value, raw_value, sort_order) VALUES (?, ?, ?, ?, ?, ?)",
                        params![
                            row_id,
                            cell.column_key,
                            cell.column_label,
                            cell.alternatives.join(" / "),
                            cell.value,
                            cell_index as i64
                        ],
                    )?;
                }
            }
        }

        inserted += 1;
    }

    transaction.commit()?;
    println!("inserted {inserted} paradigms");
    Ok(())
}