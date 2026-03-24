use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, Result};
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::models::{
    FilterSection, FilterState, GrammarConstant, LemmaOption, QuizRow, StemView,
};

const EMBEDDED_DB_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/paradigms.json"));

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Database {
    constants: HashMap<i64, GrammarConstant>,
    filter_sections: Vec<FilterSection>,
    rows: Vec<QuizRow>,
    stem_views: HashMap<i64, StemView>,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        serde_json::from_str(EMBEDDED_DB_JSON).context("failed to load embedded paradigms.json")
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

fn matches_filters(
    row: &QuizRow,
    filters: &FilterState,
    excluded_category: Option<&str>,
    exclude_selected_lemma: bool,
) -> bool {
    if !exclude_selected_lemma && !filters.selected_lemma_ids.is_empty() {
        if !filters.selected_lemma_ids.contains(&row.lemma_id) {
            return false;
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
