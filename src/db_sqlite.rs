use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use rand::prelude::SliceRandom;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::models::{
    FilterSection, FilterState, GrammarConstant, LemmaAttributes, LemmaOption, QuizRow, StemTable,
    StemTableRow, StemView,
};

const EMBEDDED_DB_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/paradigms.sqlite3"));
static EXTRACTED_DB_PATH: OnceLock<PathBuf> = OnceLock::new();

const FILTER_CATEGORY_ORDER: &[&str] = &[
    "part_of_speech",
    "type",
    "tense",
    "voice",
    "mood",
    "case",
    "number",
    "gender",
    "person",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Database {
    constants: HashMap<i64, GrammarConstant>,
    filter_sections: Vec<FilterSection>,
    rows: Vec<QuizRow>,
    stem_views: HashMap<i64, StemView>,
}

#[derive(Clone, Debug)]
struct LoadedParadigm {
    id: i64,
    title: String,
    family: String,
    tables: Vec<LoadedTable>,
}

#[derive(Clone, Debug)]
struct LoadedTable {
    kind: String,
    heading: String,
    columns: Vec<LoadedColumn>,
    rows: Vec<LoadedRow>,
}

#[derive(Clone, Debug)]
struct LoadedColumn {
    key: String,
    label: String,
}

#[derive(Clone, Debug)]
struct LoadedRow {
    key: String,
    label: String,
    cells: Vec<LoadedCell>,
}

#[derive(Clone, Debug)]
struct LoadedCell {
    id: i64,
    column_key: String,
    display_value: String,
}

#[derive(Debug)]
struct ConstantCatalog {
    next_id: i64,
    by_key: HashMap<(String, String), i64>,
    constants: HashMap<i64, GrammarConstant>,
}

impl Default for ConstantCatalog {
    fn default() -> Self {
        Self {
            next_id: 1,
            by_key: HashMap::new(),
            constants: HashMap::new(),
        }
    }
}

impl ConstantCatalog {
    fn ensure(&mut self, category: &str, canonical: &str, display_label: &str) -> i64 {
        let key = (category.to_string(), canonical.to_string());
        if let Some(id) = self.by_key.get(&key) {
            return *id;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.by_key.insert(key, id);
        self.constants.insert(
            id,
            GrammarConstant {
                id,
                category: category.to_string(),
                label: canonical.to_string(),
                display_label: display_label.to_string(),
                sort_order: sort_order_for(category, canonical),
            },
        );
        id
    }
}

impl Database {
    pub fn open_default() -> Result<Self> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("paradigms.sqlite3");
        if manifest.exists() {
            return Self::load_from_path(&manifest);
        }

        let cwd = std::env::current_dir()
            .context("failed to determine current working directory")?
            .join("assets")
            .join("paradigms.sqlite3");
        if cwd.exists() {
            return Self::load_from_path(&cwd);
        }

        let embedded = extract_embedded_database()?;
        if embedded.exists() {
            return Self::load_from_path(&embedded);
        }

        Err(anyhow!("assets/paradigms.sqlite3 not found"))
    }

    fn load_from_path(db_path: &Path) -> Result<Self> {
        let connection = Connection::open(db_path)
            .with_context(|| format!("failed to open {}", db_path.display()))?;
        let paradigms = load_paradigms(&connection)?;

        let mut catalog = ConstantCatalog::default();
        let mut rows = Vec::new();
        let mut stem_views = HashMap::new();

        for paradigm in &paradigms {
            stem_views.insert(paradigm.id, build_stem_view(paradigm));
            rows.extend(build_quiz_rows(paradigm, &mut catalog));
        }

        let constants = catalog.constants;
        let filter_sections = build_filter_sections(&constants, &rows);

        Ok(Self {
            constants,
            filter_sections,
            rows,
            stem_views,
        })
    }

    pub fn load_constants(&self) -> Result<HashMap<i64, GrammarConstant>> {
        Ok(self.constants.clone())
    }

    pub fn load_filter_sections(&self) -> Result<Vec<FilterSection>> {
        Ok(self.filter_sections.clone())
    }

    pub fn load_lemma_options(&self, filters: &FilterState) -> Result<Vec<LemmaOption>> {
        let mut grouped = HashMap::<i64, String>::new();
        for row in self.matching_rows(filters, None, true) {
            grouped.entry(row.lemma_id).or_insert_with(|| row.lemma.clone());
        }

        let mut lemmas = grouped
            .into_iter()
            .map(|(id, label)| LemmaOption { id, label })
            .collect::<Vec<_>>();
        lemmas.sort_by(|left, right| left.label.to_lowercase().cmp(&right.label.to_lowercase()));
        Ok(lemmas)
    }

    pub fn load_filter_availability(
        &self,
        filters: &FilterState,
        sections: &[FilterSection],
    ) -> Result<HashMap<String, BTreeSet<i64>>> {
        let mut availability = HashMap::new();

        for section in sections {
            let ids = self
                .matching_rows(filters, Some(&section.key), false)
                .into_iter()
                .filter_map(|row| row.constant_ids.get(&section.key).copied())
                .collect::<BTreeSet<_>>();
            availability.insert(section.key.clone(), ids);
        }

        Ok(availability)
    }

    pub fn load_candidate_rows(&self, filters: &FilterState, limit: usize) -> Result<Vec<QuizRow>> {
        let mut rows = self
            .matching_rows(filters, None, false)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        rows.shuffle(&mut rand::rng());
        rows.truncate(limit);
        Ok(rows)
    }

    pub fn load_stem_view(&self, lemma_id: i64) -> Result<Option<StemView>> {
        Ok(self.stem_views.get(&lemma_id).cloned())
    }

    pub fn write_json_to_path(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("failed to serialize database snapshot")?;
        fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
    }

    fn matching_rows<'a>(
        &'a self,
        filters: &FilterState,
        excluded_category: Option<&str>,
        exclude_selected_lemma: bool,
    ) -> Vec<&'a QuizRow> {
        self.rows
            .iter()
            .filter(|row| matches_filters(row, filters, excluded_category, exclude_selected_lemma))
            .collect()
    }
}

fn load_paradigms(connection: &Connection) -> Result<Vec<LoadedParadigm>> {
    let mut statement = connection.prepare(
        "SELECT id, title, family FROM paradigms ORDER BY title COLLATE NOCASE",
    )?;
    let headers = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut paradigms = Vec::with_capacity(headers.len());
    for (id, title, family) in headers {
        paradigms.push(LoadedParadigm {
            id,
            title,
            family,
            tables: load_tables_for_paradigm(connection, id)?,
        });
    }

    Ok(paradigms)
}

fn load_tables_for_paradigm(connection: &Connection, paradigm_id: i64) -> Result<Vec<LoadedTable>> {
    let mut statement = connection.prepare(
        "SELECT t.id, t.table_kind, t.heading,
                r.id, r.row_key, r.row_label,
                c.id, c.column_key, c.column_label, c.value, c.raw_value
         FROM paradigm_tables t
         JOIN paradigm_rows r ON r.table_id = t.id
         JOIN paradigm_cells c ON c.row_id = r.id
         WHERE t.paradigm_id = ?
         ORDER BY t.sort_order, r.sort_order, c.sort_order",
    )?;

    let mut rows = statement.query(params![paradigm_id])?;
    let mut tables = Vec::<LoadedTable>::new();
    let mut current_table_id: Option<i64> = None;
    let mut current_table: Option<LoadedTable> = None;
    let mut current_row_id: Option<i64> = None;
    let mut current_row: Option<LoadedRow> = None;

    while let Some(row) = rows.next()? {
        let table_id = row.get::<_, i64>(0)?;
        let table_kind = row.get::<_, String>(1)?;
        let heading = row.get::<_, String>(2)?;
        let row_id = row.get::<_, i64>(3)?;
        let row_key = row.get::<_, String>(4)?;
        let row_label = row.get::<_, String>(5)?;
        let cell_id = row.get::<_, i64>(6)?;
        let column_key = row.get::<_, String>(7)?;
        let column_label = row.get::<_, String>(8)?;
        let alternatives = row.get::<_, String>(9)?;
        let raw_value = row.get::<_, String>(10)?;

        if current_table_id != Some(table_id) {
            if let Some(finished_row) = current_row.take() {
                if let Some(table) = current_table.as_mut() {
                    table.rows.push(finished_row);
                }
            }
            if let Some(finished_table) = current_table.take() {
                tables.push(finished_table);
            }

            current_table_id = Some(table_id);
            current_row_id = None;
            current_table = Some(LoadedTable {
                kind: table_kind,
                heading,
                columns: Vec::new(),
                rows: Vec::new(),
            });
        }

        if current_row_id != Some(row_id) {
            if let Some(finished_row) = current_row.take() {
                if let Some(table) = current_table.as_mut() {
                    table.rows.push(finished_row);
                }
            }
            current_row_id = Some(row_id);
            current_row = Some(LoadedRow {
                key: row_key,
                label: row_label,
                cells: Vec::new(),
            });
        }

        if let Some(table) = current_table.as_mut() {
            if table.columns.iter().all(|column| column.key != column_key) {
                table.columns.push(LoadedColumn {
                    key: column_key.clone(),
                    label: column_label.clone(),
                });
            }
        }

        if let Some(loaded_row) = current_row.as_mut() {
            loaded_row.cells.push(LoadedCell {
                id: cell_id,
                column_key,
                display_value: combine_cell_values(&raw_value, &alternatives),
            });
        }
    }

    if let Some(finished_row) = current_row {
        if let Some(table) = current_table.as_mut() {
            table.rows.push(finished_row);
        }
    }
    if let Some(finished_table) = current_table {
        tables.push(finished_table);
    }

    Ok(tables)
}

fn build_quiz_rows(paradigm: &LoadedParadigm, catalog: &mut ConstantCatalog) -> Vec<QuizRow> {
    let mut rows = Vec::new();

    for table in &paradigm.tables {
        if table.kind == "derivatives" || table.heading.contains("Dual") {
            continue;
        }

        for row in &table.rows {
            for (column_index, cell) in row.cells.iter().enumerate() {
                if cell.display_value.trim().is_empty() || cell.display_value == "-" {
                    continue;
                }

                if cell.column_key == "du" {
                    continue;
                }

                let mut constant_ids = HashMap::new();
                insert_known_tag(
                    catalog,
                    &mut constant_ids,
                    "part_of_speech",
                    family_canonical(&paradigm.family),
                    family_display(&paradigm.family),
                );
                insert_known_tag(
                    catalog,
                    &mut constant_ids,
                    "type",
                    table_type_canonical(&table.kind),
                    table_type_display(&table.kind),
                );
                apply_heading_tags(&table.kind, &table.heading, catalog, &mut constant_ids);

                match table.kind.as_str() {
                    "nominal" => {
                        insert_tag_from_key(catalog, &mut constant_ids, "case", &row.key, &row.label);
                        if let Some(column) = table.columns.get(column_index) {
                            insert_tag_from_key(catalog, &mut constant_ids, "number", &column.key, &column.label);
                        }
                    }
                    "gendered_nominal" => {
                        insert_tag_from_key(catalog, &mut constant_ids, "case", &row.key, &row.label);
                        if let Some(column) = table.columns.get(column_index) {
                            insert_tag_from_key(catalog, &mut constant_ids, "gender", &column.key, &column.label);
                        }
                    }
                    "verb" => {
                        insert_tag_from_key(catalog, &mut constant_ids, "person", &row.key, &row.label);
                        if let Some(column) = table.columns.get(column_index) {
                            insert_tag_from_key(catalog, &mut constant_ids, "number", &column.key, &column.label);
                        }
                    }
                    "participle" => {
                        insert_tag_from_key(catalog, &mut constant_ids, "gender", &row.key, &row.label);
                        if cell.column_key != "form" {
                            if let Some(column) = table.columns.get(column_index) {
                                insert_tag_from_key(catalog, &mut constant_ids, "voice", &column.key, &column.label);
                            }
                        }
                    }
                    "infinitive" => {
                        if cell.column_key != "form" {
                            if let Some(column) = table.columns.get(column_index) {
                                insert_tag_from_key(catalog, &mut constant_ids, "voice", &column.key, &column.label);
                            }
                        }
                    }
                    "verb_irregular" | "nominal_irregular" => {}
                    _ => {}
                }

                rows.push(QuizRow {
                    inflection_id: cell.id,
                    lemma_id: paradigm.id,
                    lemma: paradigm.title.clone(),
                    form: sanitize_quiz_form(&table.kind, &cell.display_value),
                    constant_ids,
                    lemma_attributes: LemmaAttributes::default(),
                });
            }
        }
    }

    rows
}

fn build_stem_view(paradigm: &LoadedParadigm) -> StemView {
    StemView {
        lemma: paradigm.title.clone(),
        stemtypes: Vec::new(),
        common_tags: vec![family_display(&paradigm.family).to_string()],
        tables: paradigm
            .tables
            .iter()
            .filter(|table| !table.heading.contains("Dual"))
            .map(|table| StemTable {
                heading: split_heading(&table.heading),
                columns: table
                    .columns
                    .iter()
                    .filter(|column| column.key != "du")
                    .map(|column| shorten_axis_label(&column.label))
                    .collect(),
                rows: table
                    .rows
                    .iter()
                    .map(|row| StemTableRow {
                        label: shorten_axis_label(&row.label),
                        cells: row
                            .cells
                            .iter()
                            .filter(|cell| cell.column_key != "du")
                            .map(|cell| cell.display_value.clone())
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn build_filter_sections(
    constants: &HashMap<i64, GrammarConstant>,
    rows: &[QuizRow],
) -> Vec<FilterSection> {
    let mut categories = rows
        .iter()
        .flat_map(|row| row.constant_ids.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    categories.sort_by(|left, right| category_rank(left).cmp(&category_rank(right)).then(left.cmp(right)));

    let mut sections = Vec::new();
    for category in categories {
        let mut options = constants
            .values()
            .filter(|constant| constant.category == category)
            .cloned()
            .collect::<Vec<_>>();
        options.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then(left.display_label.to_lowercase().cmp(&right.display_label.to_lowercase()))
        });
        options.dedup_by(|left, right| left.label == right.label);

        if options.len() <= 1 {
            continue;
        }

        sections.push(FilterSection {
            key: category.clone(),
            title: title_case_identifier(&category),
            options,
        });
    }

    sections
}

fn matches_filters(
    row: &QuizRow,
    filters: &FilterState,
    excluded_category: Option<&str>,
    exclude_selected_lemma: bool,
) -> bool {
    if !exclude_selected_lemma {
        if !filters.selected_lemma_ids.is_empty() {
            if !filters.selected_lemma_ids.contains(&row.lemma_id) {
                return false;
            }
        }
    }

    for (category, selected_ids) in &filters.selected_constants {
        if selected_ids.is_empty() || excluded_category == Some(category.as_str()) {
            continue;
        }

        let Some(row_value) = row.constant_ids.get(category) else {
            return false;
        };
        if !selected_ids.contains(row_value) {
            return false;
        }
    }

    true
}

fn apply_heading_tags(
    table_kind: &str,
    heading: &str,
    catalog: &mut ConstantCatalog,
    constant_ids: &mut HashMap<String, i64>,
) {
    for part in heading.split(" | ").map(str::trim).filter(|part| !part.is_empty()) {
        let normalized = part.to_lowercase();
        match normalized.as_str() {
            "present" | "imperfect" | "future" | "aorist" | "perfect" | "pluperfect" | "future perfect" => {
                insert_known_tag(catalog, constant_ids, "tense", normalized.as_str(), part);
            }
            "indicative" | "subjunctive" | "optative" | "imperative" | "infinitive" | "participle" => {
                insert_known_tag(catalog, constant_ids, "mood", normalized.as_str(), part);
            }
            "singular forms" => insert_known_tag(catalog, constant_ids, "number", "singular", "Singular"),
            "dual forms" => insert_known_tag(catalog, constant_ids, "number", "dual", "Dual"),
            "plural forms" => insert_known_tag(catalog, constant_ids, "number", "plural", "Plural"),
            "active" | "middle" | "passive" | "middle / passive" | "middle/passive" | "active / middle" | "active/middle"
                if table_kind == "verb" || table_kind.ends_with("_irregular") =>
            {
                let (canonical, display) = normalize_voice(normalized.as_str(), part);
                insert_known_tag(catalog, constant_ids, "voice", canonical, &display);
            }
            _ => {}
        }
    }
}

fn insert_tag_from_key(
    catalog: &mut ConstantCatalog,
    constant_ids: &mut HashMap<String, i64>,
    category: &str,
    key: &str,
    display_label: &str,
) {
    let canonical = canonical_from_key(category, key, display_label);
    let display = display_from_category(category, &canonical, display_label);
    insert_known_tag(catalog, constant_ids, category, &canonical, &display);
}

fn insert_known_tag(
    catalog: &mut ConstantCatalog,
    constant_ids: &mut HashMap<String, i64>,
    category: &str,
    canonical: &str,
    display_label: &str,
) {
    let id = catalog.ensure(category, canonical, display_label);
    constant_ids.insert(category.to_string(), id);
}

fn canonical_from_key(category: &str, key: &str, display_label: &str) -> String {
    let normalized_key = key.trim().to_lowercase();
    match category {
        "case" | "number" | "gender" | "person" | "voice" => {
            if normalized_key.is_empty() {
                display_label.trim().to_lowercase()
            } else {
                normalized_key
            }
        }
        _ => display_label.trim().to_lowercase(),
    }
}

fn display_from_category(category: &str, canonical: &str, fallback: &str) -> String {
    match category {
        "number" => match canonical {
            "sg" | "singular" => "Singular".to_string(),
            "du" | "dual" => "Dual".to_string(),
            "pl" | "plural" => "Plural".to_string(),
            _ => fallback.to_string(),
        },
        "case" => match canonical {
            "nom" | "nominative" => "Nominative".to_string(),
            "gen" | "genitive" => "Genitive".to_string(),
            "dat" | "dative" => "Dative".to_string(),
            "acc" | "accusative" => "Accusative".to_string(),
            "voc" | "vocative" => "Vocative".to_string(),
            _ => fallback.to_string(),
        },
        "gender" => match canonical {
            "masc" | "masculine" => "Masc".to_string(),
            "fem" | "feminine" => "Fem".to_string(),
            "neut" | "neuter" => "Neut".to_string(),
            "common" => "Common".to_string(),
            _ => fallback.to_string(),
        },
        "voice" => normalize_voice(canonical, fallback).1,
        _ => fallback.to_string(),
    }
}

fn family_canonical(family: &str) -> &'static str {
    match family {
        "nominal" => "nominal",
        "gendered_nominal" => "gendered nominal",
        "verb" => "verb",
        _ => "unknown",
    }
}

fn family_display(family: &str) -> &'static str {
    match family {
        "nominal" => "Nominal",
        "gendered_nominal" => "Gendered nominal",
        "verb" => "Verb",
        _ => "Unknown",
    }
}

fn table_type_canonical(kind: &str) -> &'static str {
    match kind {
        "verb" => "finite",
        "infinitive" => "infinitive",
        "participle" => "participle",
        "nominal" => "nominal",
        "gendered_nominal" => "gendered nominal",
        "derivatives" => "derived forms",
        "verb_irregular" => "irregular verb",
        "nominal_irregular" => "irregular nominal",
        _ => "other",
    }
}

fn table_type_display(kind: &str) -> &'static str {
    match kind {
        "verb" => "Finite",
        "infinitive" => "Infinitive",
        "participle" => "Participle",
        "nominal" => "Nominal",
        "gendered_nominal" => "Gendered nominal",
        "derivatives" => "Derived forms",
        "verb_irregular" => "Irregular verb",
        "nominal_irregular" => "Irregular nominal",
        _ => "Other",
    }
}

fn normalize_voice<'a>(canonical: &'a str, fallback: &str) -> (&'a str, String) {
    match canonical {
        "active" => ("active", "Active".to_string()),
        "middle" => ("middle", "Middle".to_string()),
        "passive" => ("passive", "Passive".to_string()),
        "middle/passive" | "middle / passive" => ("middle / passive", "Middle / Passive".to_string()),
        "active/middle" | "active / middle" => ("active / middle", "Active / Middle".to_string()),
        other => (other, fallback.to_string()),
    }
}

fn category_rank(category: &str) -> usize {
    FILTER_CATEGORY_ORDER
        .iter()
        .position(|candidate| *candidate == category)
        .unwrap_or(FILTER_CATEGORY_ORDER.len())
}

fn sort_order_for(category: &str, canonical: &str) -> i64 {
    match category {
        "part_of_speech" => match canonical {
            "nominal" => 0,
            "gendered nominal" => 1,
            "verb" => 2,
            _ => 99,
        },
        "type" => match canonical {
            "nominal" => 0,
            "gendered nominal" => 1,
            "finite" => 2,
            "participle" => 3,
            "infinitive" => 4,
            "derived forms" => 5,
            "irregular nominal" => 6,
            "irregular verb" => 7,
            _ => 99,
        },
        "tense" => match canonical {
            "present" => 0,
            "imperfect" => 1,
            "future" => 2,
            "aorist" => 3,
            "perfect" => 4,
            "pluperfect" => 5,
            "future perfect" => 6,
            _ => 99,
        },
        "voice" => match canonical {
            "active" => 0,
            "middle" => 1,
            "passive" => 2,
            "middle / passive" => 3,
            "active / middle" => 4,
            _ => 99,
        },
        "mood" => match canonical {
            "indicative" => 0,
            "subjunctive" => 1,
            "optative" => 2,
            "imperative" => 3,
            "infinitive" => 4,
            "participle" => 5,
            _ => 99,
        },
        "case" => match canonical {
            "nom" | "nominative" => 0,
            "gen" | "genitive" => 1,
            "dat" | "dative" => 2,
            "acc" | "accusative" => 3,
            "voc" | "vocative" => 4,
            _ => 99,
        },
        "number" => match canonical {
            "sg" | "singular" => 0,
            "du" | "dual" => 1,
            "pl" | "plural" => 2,
            _ => 99,
        },
        "gender" => match canonical {
            "masc" | "masculine" => 0,
            "fem" | "feminine" => 1,
            "neut" | "neuter" => 2,
            "common" => 3,
            _ => 99,
        },
        "person" => match canonical {
            "1" | "1st" => 0,
            "2" | "2nd" => 1,
            "3" | "3rd" => 2,
            _ => 99,
        },
        _ => 99,
    }
}

fn combine_cell_values(raw_value: &str, alternatives: &str) -> String {
    let mut values = Vec::<String>::new();
    push_unique_values(&mut values, raw_value);
    push_unique_values(&mut values, alternatives);

    if values.is_empty() {
        String::from("-")
    } else {
        values.join(" / ")
    }
}

fn sanitize_quiz_form(table_kind: &str, display_value: &str) -> String {
    match table_kind {
        "nominal" | "gendered_nominal" | "nominal_irregular" => display_value
            .split(" / ")
            .map(strip_leading_article)
            .collect::<Vec<_>>()
            .join(" / "),
        _ => display_value.to_string(),
    }
}

fn strip_leading_article(value: &str) -> String {
    let trimmed = value.trim();
    let mut parts = trimmed.split_whitespace();
    let Some(first) = parts.next() else {
        return trimmed.to_string();
    };

    if !is_greek_article(first) {
        return trimmed.to_string();
    }

    let remainder = parts.collect::<Vec<_>>().join(" ");
    if remainder.is_empty() {
        trimmed.to_string()
    } else {
        remainder
    }
}

fn is_greek_article(token: &str) -> bool {
    matches!(
        normalize_greek_token(token).as_str(),
        "ο"
            | "η"
            | "το"
            | "του"
            | "της"
            | "τω"
            | "τον"
            | "την"
            | "οι"
            | "αι"
            | "τα"
            | "των"
            | "τοις"
            | "ταις"
        )
}

fn normalize_greek_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| !ch.is_alphabetic())
        .nfd()
        .filter(|ch| !is_combining_mark(*ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn push_unique_values(values: &mut Vec<String>, source: &str) {
    for candidate in source
        .split(|ch: char| ch == ',' || ch == '/')
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        if !values.iter().any(|existing| existing == candidate) {
            values.push(candidate.to_string());
        }
    }
}

fn split_heading(heading: &str) -> Vec<String> {
    heading
        .split(" | ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn shorten_axis_label(label: &str) -> String {
    match label.to_lowercase().as_str() {
        "1" | "1st" => String::from("1"),
        "2" | "2nd" => String::from("2"),
        "3" | "3rd" => String::from("3"),
        "singular" => String::from("sg"),
        "dual" => String::from("du"),
        "plural" => String::from("pl"),
        "nominative" => String::from("N"),
        "genitive" => String::from("G"),
        "dative" => String::from("D"),
        "accusative" => String::from("A"),
        "vocative" => String::from("V"),
        "masculine" => String::from("M"),
        "feminine" => String::from("F"),
        "neuter" => String::from("N"),
        "infinitive" => String::from("Inf"),
        _ => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quiz::render_analysis;

    #[test]
    fn strips_leading_articles_from_nominal_quiz_forms() {
        let paradigm = LoadedParadigm {
            id: 1,
            title: "βίος".to_string(),
            family: "nominal".to_string(),
            tables: vec![LoadedTable {
                kind: "nominal".to_string(),
                heading: "Nominal paradigm".to_string(),
                columns: vec![LoadedColumn {
                    key: "sg".to_string(),
                    label: "Singular".to_string(),
                }],
                rows: vec![LoadedRow {
                    key: "nom".to_string(),
                    label: "Nominative".to_string(),
                    cells: vec![LoadedCell {
                        id: 10,
                        column_key: "sg".to_string(),
                        display_value: "ὁ βίος".to_string(),
                    }],
                }],
            }],
        };

        let mut catalog = ConstantCatalog::default();
        let rows = build_quiz_rows(&paradigm, &mut catalog);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].form, "βίος");
    }

    #[test]
    fn renders_gender_for_participle_quiz_rows() {
        let paradigm = LoadedParadigm {
            id: 2,
            title: "αἰσθάνομαι".to_string(),
            family: "verb".to_string(),
            tables: vec![LoadedTable {
                kind: "participle".to_string(),
                heading: "Present | Middle | Participle".to_string(),
                columns: vec![LoadedColumn {
                    key: "middle".to_string(),
                    label: "Middle".to_string(),
                }],
                rows: vec![LoadedRow {
                    key: "masc".to_string(),
                    label: "Masculine".to_string(),
                    cells: vec![LoadedCell {
                        id: 20,
                        column_key: "middle".to_string(),
                        display_value: "αἰσθανόμενος".to_string(),
                    }],
                }],
            }],
        };

        let mut catalog = ConstantCatalog::default();
        let rows = build_quiz_rows(&paradigm, &mut catalog);
        let analysis = render_analysis(&rows[0], &catalog.constants);

        assert!(analysis.contains("Masc"));
        assert!(analysis.contains("Participle"));
    }
}

fn title_case_identifier(category: &str) -> String {
    category
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_embedded_database() -> Result<PathBuf> {
    if let Some(path) = EXTRACTED_DB_PATH.get() {
        return Ok(path.clone());
    }

    let base_dir = std::env::temp_dir().join("morph_app");
    fs::create_dir_all(&base_dir)
        .with_context(|| format!("failed to create {}", base_dir.display()))?;

    let db_path = base_dir.join("paradigms.sqlite3");
    let should_write = match fs::metadata(&db_path) {
        Ok(metadata) => metadata.len() != EMBEDDED_DB_BYTES.len() as u64,
        Err(_) => true,
    };

    if should_write {
        fs::write(&db_path, EMBEDDED_DB_BYTES)
            .with_context(|| format!("failed to extract embedded SQLite to {}", db_path.display()))?;
    }

    let _ = EXTRACTED_DB_PATH.set(db_path.clone());
    Ok(db_path)
}