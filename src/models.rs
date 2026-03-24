use std::collections::{BTreeMap, BTreeSet, HashMap};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarConstant {
    pub id: i64,
    pub category: String,
    pub label: String,
    pub display_label: String,
    pub sort_order: i64,
}

#[derive(Clone, Debug)]
pub struct FilterSection {
    pub key: String,
    pub title: String,
    pub options: Vec<GrammarConstant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LemmaOption {
    pub id: i64,
    pub label: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FilterState {
    pub selected_constants: BTreeMap<String, BTreeSet<i64>>,
    pub selected_lemma_ids: BTreeSet<i64>,
}

impl FilterState {
    pub fn toggle_lemma(&mut self, lemma_id: i64) {
        if !self.selected_lemma_ids.insert(lemma_id) {
            self.selected_lemma_ids.remove(&lemma_id);
        }
    }

    pub fn toggle_constant(&mut self, category: &str, constant_id: i64) {
        let selected = self.selected_constants.entry(category.to_string()).or_default();
        if !selected.insert(constant_id) {
            selected.remove(&constant_id);
        }
        if selected.is_empty() {
            self.selected_constants.remove(category);
        }
    }

    pub fn is_selected(&self, category: &str, constant_id: i64) -> bool {
        self.selected_constants
            .get(category)
            .is_some_and(|selected| selected.contains(&constant_id))
    }
}

#[derive(Clone, Debug, Default)]
pub struct LemmaAttributes {
    pub by_category: HashMap<String, Vec<GrammarConstant>>,
}

#[derive(Clone, Debug)]
pub struct QuizRow {
    pub inflection_id: i64,
    pub lemma_id: i64,
    pub lemma: String,
    pub form: String,
    pub constant_ids: HashMap<String, i64>,
    pub lemma_attributes: LemmaAttributes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuizMode {
    ParseForm,
    BuildForm,
    InferForm,
}

impl QuizMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ParseForm => "Form -> analysis",
            Self::BuildForm => "Analysis -> form",
            Self::InferForm => "Form -> typed form",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnswerOption {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct Question {
    pub lemma_id: i64,
    pub mode: QuizMode,
    pub prompt: String,
    pub clue: Option<String>,
    pub options: Vec<AnswerOption>,
    pub correct_option_ids: BTreeSet<String>,
    pub selected_option_id: Option<String>,
    pub accepted_text_answers: Vec<String>,
    pub text_entry: String,
    pub submitted_text: Option<String>,
}

impl Question {
    pub fn is_answered(&self) -> bool {
        self.selected_option_id.is_some() || self.submitted_text.is_some()
    }

    pub fn is_correct(&self, option_id: &str) -> bool {
        self.correct_option_ids.contains(option_id)
    }

    pub fn uses_text_entry(&self) -> bool {
        !self.accepted_text_answers.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct StemTableRow {
    pub label: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StemTable {
    pub heading: Vec<String>,
    pub columns: Vec<String>,
    pub rows: Vec<StemTableRow>,
}

#[derive(Clone, Debug)]
pub struct StemView {
    pub lemma: String,
    pub stemtypes: Vec<String>,
    pub common_tags: Vec<String>,
    pub tables: Vec<StemTable>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub answered: usize,
    pub correct: usize,
    pub streak: usize,
    pub best_streak: usize,
}

impl SessionStats {
    pub fn register(&mut self, was_correct: bool) {
        self.answered += 1;
        if was_correct {
            self.correct += 1;
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
        } else {
            self.streak = 0;
        }
    }

    pub fn accuracy(&self) -> usize {
        if self.answered == 0 {
            return 0;
        }
        (self.correct * 100) / self.answered
    }
}
