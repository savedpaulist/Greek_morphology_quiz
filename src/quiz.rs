use std::collections::{BTreeSet, HashMap, HashSet};

use rand::prelude::SliceRandom;

use crate::models::{AnswerOption, GrammarConstant, Question, QuizMode, QuizRow};

const DESCRIPTION_ORDER: &[&str] = &[
    "declension",
    "case",
    "gender",
    "tense",
    "voice",
    "mood",
    "number",
    "person",
    "dialect",
    "stemtype",
];

pub fn generate_question(
    rows: &[QuizRow],
    constants: &HashMap<i64, GrammarConstant>,
    mode: QuizMode,
) -> Option<Question> {
    if rows.len() < 6 {
        return None;
    }

    let mut candidates = rows.to_vec();
    candidates.shuffle(&mut rand::rng());

    for correct in candidates {
        let question = match mode {
            QuizMode::ParseForm => generate_parse_question(rows, constants, correct),
            QuizMode::BuildForm => generate_build_question(rows, constants, correct),
            QuizMode::InferForm => generate_infer_question(rows, constants, correct),
        };

        if question.is_some() {
            return question;
        }
    }

    None
}

fn generate_infer_question(
    rows: &[QuizRow],
    constants: &HashMap<i64, GrammarConstant>,
    correct: QuizRow,
) -> Option<Question> {
    let analysis = render_analysis(&correct, constants);
    let valid_forms = rows
        .iter()
        .filter(|row| row.lemma_id == correct.lemma_id)
        .filter_map(|row| {
            let row_analysis = render_analysis(row, constants);
            (row_analysis == analysis).then_some(row.form.clone())
        })
        .collect::<HashSet<_>>();

    if valid_forms.is_empty() {
        return None;
    }

    let mut clue_candidates = rows
        .iter()
        .filter(|row| row.lemma_id == correct.lemma_id)
        .filter(|row| row.form != correct.form)
        .filter(|row| render_analysis(row, constants) != analysis)
        .collect::<Vec<_>>();
    if clue_candidates.is_empty() {
        clue_candidates = rows
            .iter()
            .filter(|row| row.lemma_id == correct.lemma_id)
            .filter(|row| row.form != correct.form)
            .collect::<Vec<_>>();
    }
    clue_candidates.shuffle(&mut rand::rng());
    let clue = clue_candidates.first()?;

    let mut accepted_text_answers = valid_forms.into_iter().collect::<Vec<_>>();
    accepted_text_answers.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));

    Some(Question {
        lemma_id: correct.lemma_id,
        mode: QuizMode::InferForm,
        prompt: analysis,
        clue: Some(format!("Clue form: {}", clue.form)),
        options: Vec::new(),
        correct_option_ids: BTreeSet::new(),
        selected_option_id: None,
        accepted_text_answers,
        text_entry: String::new(),
        submitted_text: None,
    })
}

fn generate_parse_question(
    rows: &[QuizRow],
    constants: &HashMap<i64, GrammarConstant>,
    correct: QuizRow,
) -> Option<Question> {
    let valid_analyses = rows
        .iter()
        .filter(|row| row.lemma_id == correct.lemma_id && row.form == correct.form)
        .map(|row| render_analysis(row, constants))
        .collect::<HashSet<_>>();

    if valid_analyses.len() != 1 {
        return None;
    }

    let correct_label = valid_analyses.iter().next()?.clone();
    let correct_pos = correct.constant_ids.get("part_of_speech").copied();

    // Collect unique candidate labels with their tag-difference to the correct label.
    // ranked_similar_rows gives grammatically close rows first, so we visit them in a
    // good order before deduplication discards later duplicates.
    let mut seen_labels: HashSet<String> = HashSet::from([correct_label.clone()]);
    let mut all_candidates: Vec<(String, usize)> = Vec::new();

    for row in ranked_similar_rows(rows, &correct, correct_pos) {
        let label = render_analysis(&row, constants);
        if seen_labels.insert(label.clone()) {
            let diff = tag_diff_count(&correct_label, &label);
            all_candidates.push((label, diff));
        }
    }

    // Prefer labels that differ by 1–3 tags (confusable but distinct), fall back to the rest.
    let (mut close, mut far): (Vec<_>, Vec<_>) = all_candidates
        .into_iter()
        .partition(|(_, d)| *d >= 1 && *d <= 3);

    close.shuffle(&mut rand::rng());
    far.shuffle(&mut rand::rng());

    let mut options = vec![AnswerOption {
        id: format!("parse:{}", correct.inflection_id),
        label: correct_label.clone(),
    }];

    for (label, _) in close.into_iter().chain(far) {
        options.push(AnswerOption {
            id: format!("analysis:{}", options.len()),
            label,
        });
        if options.len() == 6 {
            break;
        }
    }

    if options.len() < 6 {
        return None;
    }

    options.shuffle(&mut rand::rng());

    Some(Question {
        lemma_id: correct.lemma_id,
        mode: QuizMode::ParseForm,
        prompt: correct.form.clone(),
        clue: None,
        correct_option_ids: BTreeSet::from([format!("parse:{}", correct.inflection_id)]),
        options,
        selected_option_id: None,
        accepted_text_answers: Vec::new(),
        text_entry: String::new(),
        submitted_text: None,
    })
}

fn generate_build_question(
    rows: &[QuizRow],
    constants: &HashMap<i64, GrammarConstant>,
    correct: QuizRow,
) -> Option<Question> {
    let analysis = render_analysis(&correct, constants);
    let valid_forms = rows
        .iter()
        .filter(|row| row.lemma_id == correct.lemma_id)
        .filter_map(|row| {
            let row_analysis = render_analysis(row, constants);
            (row_analysis == analysis).then_some(row.form.clone())
        })
        .collect::<HashSet<_>>();

    if valid_forms.len() != 1 {
        return None;
    }

    let correct_pos = correct.constant_ids.get("part_of_speech").copied();

    // Collect unique candidate forms with their edit distance to the correct form.
    // Visit same-lemma rows first (most inflectionally similar), then grammatically
    // similar rows from other lemmas via ranked_similar_rows.
    let mut seen = valid_forms.clone();
    let mut all_candidates: Vec<(String, usize)> = Vec::new();

    for row in rows.iter().filter(|row| row.lemma_id == correct.lemma_id) {
        if seen.insert(row.form.clone()) {
            let dist = levenshtein(&correct.form, &row.form);
            all_candidates.push((row.form.clone(), dist));
        }
    }
    for row in ranked_similar_rows(rows, &correct, correct_pos) {
        if seen.insert(row.form.clone()) {
            let dist = levenshtein(&correct.form, &row.form);
            all_candidates.push((row.form.clone(), dist));
        }
    }

    // Prefer forms that differ by 1–3 characters (look similar but aren't identical),
    // fall back to any other form if not enough close ones exist.
    let (mut close, mut far): (Vec<_>, Vec<_>) = all_candidates
        .into_iter()
        .partition(|(_, d)| *d >= 1 && *d <= 3);

    close.shuffle(&mut rand::rng());
    far.shuffle(&mut rand::rng());

    let mut options = vec![AnswerOption {
        id: format!("form:{}", correct.inflection_id),
        label: correct.form.clone(),
    }];

    for (form, _) in close.into_iter().chain(far) {
        options.push(AnswerOption {
            id: format!("form:{}", options.len()),
            label: form,
        });
        if options.len() == 6 {
            break;
        }
    }

    if options.len() < 6 {
        return None;
    }

    options.shuffle(&mut rand::rng());

    Some(Question {
        lemma_id: correct.lemma_id,
        mode: QuizMode::BuildForm,
        prompt: analysis.clone(),
        clue: None,
        correct_option_ids: BTreeSet::from([format!("form:{}", correct.inflection_id)]),
        options,
        selected_option_id: None,
        accepted_text_answers: Vec::new(),
        text_entry: String::new(),
        submitted_text: None,
    })
}

pub fn is_correct_text_answer(question: &Question, value: &str) -> bool {
    question
        .accepted_text_answers
        .iter()
        .any(|candidate| normalize_answer(candidate) == normalize_answer(value))
}

pub fn normalize_answer(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        let normalized = match ch {
            'ς' => 'σ',
            'Σ' => 'σ',
            'σ' => 'σ',
            _ => match deaccent_greek(ch) {
                Some(mapped) => mapped,
                None => continue,
            },
        };
        if !normalized.is_whitespace() {
            output.push(normalized);
        }
    }
    output
}

pub fn deaccent_greek_string(s: &str) -> String {
    s.chars().filter_map(deaccent_greek).collect()
}

pub fn deaccent_greek(ch: char) -> Option<char> {
    match ch {
        'α' | 'Α' => Some('α'),
        'ά' | 'ὰ' | 'ᾶ' | 'ἀ' | 'ἁ' | 'ἄ' | 'ἅ' | 'ἂ' | 'ἃ' | 'ἆ' | 'ἇ' | 'ᾱ' | 'ᾰ' | 'ᾳ'
        | 'ᾴ' | 'ᾲ' | 'ᾷ' | 'Ἀ' | 'Ἁ' | 'Ἄ' | 'Ἅ' | 'Ἂ' | 'Ἃ' | 'Ἆ' | 'Ἇ' | 'Ὰ' | 'Ά' => Some('α'),
        'ε' | 'Ε' => Some('ε'),
        'έ' | 'ὲ' | 'ἐ' | 'ἑ' | 'ἔ' | 'ἕ' | 'ἒ' | 'ἓ' | 'Ἐ' | 'Ἑ' | 'Ἔ' | 'Ἕ' | 'Ἒ' | 'Ἓ' | 'Ὲ' | 'Έ' => Some('ε'),
        'η' | 'Η' => Some('η'),
        'ή' | 'ὴ' | 'ῆ' | 'ἠ' | 'ἡ' | 'ἤ' | 'ἥ' | 'ἢ' | 'ἣ' | 'ἦ' | 'ἧ' | 'ῃ' | 'ῄ' | 'ῂ' | 'ῇ'
        | 'Ἠ' | 'Ἡ' | 'Ἤ' | 'Ἥ' | 'Ἢ' | 'Ἣ' | 'Ἦ' | 'Ἧ' | 'Ὴ' | 'Ή' => Some('η'),
        'ι' | 'Ι' => Some('ι'),
        'ί' | 'ὶ' | 'ῖ' | 'ἰ' | 'ἱ' | 'ἴ' | 'ἵ' | 'ἲ' | 'ἳ' | 'ἶ' | 'ἷ' | 'ϊ' | 'ΐ' | 'ῒ' | 'ῗ'
        | 'Ἰ' | 'Ἱ' | 'Ἴ' | 'Ἵ' | 'Ἲ' | 'Ἳ' | 'Ἶ' | 'Ἷ' | 'Ὶ' | 'Ί' | 'Ϊ' => Some('ι'),
        'ο' | 'Ο' => Some('ο'),
        'ό' | 'ὸ' | 'ὀ' | 'ὁ' | 'ὄ' | 'ὅ' | 'ὂ' | 'ὃ' | 'Ὀ' | 'Ὁ' | 'Ὄ' | 'Ὅ' | 'Ὂ' | 'Ὃ' | 'Ὸ' | 'Ό' => Some('ο'),
        'υ' | 'Υ' => Some('υ'),
        'ύ' | 'ὺ' | 'ῦ' | 'ὐ' | 'ὑ' | 'ὔ' | 'ὕ' | 'ὒ' | 'ὓ' | 'ὖ' | 'ὗ' | 'ϋ' | 'ΰ' | 'ῢ' | 'ῧ'
        | 'Ύ' | 'Ὺ' | 'Ϋ' => Some('υ'),
        'ω' | 'Ω' => Some('ω'),
        'ώ' | 'ὼ' | 'ῶ' | 'ὠ' | 'ὡ' | 'ὤ' | 'ὥ' | 'ὢ' | 'ὣ' | 'ὦ' | 'ὧ' | 'ῳ' | 'ῴ' | 'ῲ' | 'ῷ'
        | 'Ὠ' | 'Ὡ' | 'Ὤ' | 'Ὥ' | 'Ὢ' | 'Ὣ' | 'Ὦ' | 'Ὧ' | 'Ὼ' | 'Ώ' => Some('ω'),
        'β' | 'Β' => Some('β'),
        'γ' | 'Γ' => Some('γ'),
        'δ' | 'Δ' => Some('δ'),
        'ζ' | 'Ζ' => Some('ζ'),
        'θ' | 'Θ' => Some('θ'),
        'κ' | 'Κ' => Some('κ'),
        'λ' | 'Λ' => Some('λ'),
        'μ' | 'Μ' => Some('μ'),
        'ν' | 'Ν' => Some('ν'),
        'ξ' | 'Ξ' => Some('ξ'),
        'π' | 'Π' => Some('π'),
        'ρ' | 'ῤ' | 'ῥ' | 'Ρ' => Some('ρ'),
        'τ' | 'Τ' => Some('τ'),
        'φ' | 'Φ' => Some('φ'),
        'χ' | 'Χ' => Some('χ'),
        'ψ' | 'Ψ' => Some('ψ'),
        _ if ch.is_ascii_alphabetic() => Some(ch.to_ascii_lowercase()),
        _ => None,
    }
}

fn ranked_similar_rows(rows: &[QuizRow], correct: &QuizRow, correct_pos: Option<i64>) -> Vec<QuizRow> {
    let mut similar = rows
        .iter()
        .filter(|row| row.inflection_id != correct.inflection_id)
        .filter(|row| {
            correct_pos.is_none()
                || row.constant_ids.get("part_of_speech").copied() == correct_pos
                || row.lemma_id == correct.lemma_id
        })
        .cloned()
        .collect::<Vec<_>>();

    similar.sort_by_key(|row| std::cmp::Reverse(similarity_score(correct, row)));
    similar
}

fn similarity_score(left: &QuizRow, right: &QuizRow) -> usize {
    let mut score = 0;
    if left.lemma_id == right.lemma_id {
        score += 8;
    }
    for key in ["case", "number", "gender", "tense", "voice", "mood", "person"] {
        if left.constant_ids.get(key) == right.constant_ids.get(key) {
            score += 2;
        }
    }
    for key in ["declension", "grammatical_class", "dialect", "type"] {
        if left.lemma_attributes.by_category.get(key) == right.lemma_attributes.by_category.get(key) {
            score += 1;
        }
    }
    score
}

/// Levenshtein edit distance between two strings, operating on Unicode scalar values.
/// This correctly handles Greek polytonic characters.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 { return n; }
    if n == 0 { return m; }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Number of positional tag differences between two analysis label strings.
/// Both strings have the form "tag1 • tag2 • ... • tagN" with a fixed tag order,
/// so a positional comparison directly counts differing grammatical categories.
fn tag_diff_count(a: &str, b: &str) -> usize {
    let a_parts: Vec<&str> = a.split(" • ").collect();
    let b_parts: Vec<&str> = b.split(" • ").collect();
    let max_len = a_parts.len().max(b_parts.len());
    (0..max_len).filter(|&i| a_parts.get(i) != b_parts.get(i)).count()
}

pub fn render_analysis(row: &QuizRow, constants: &HashMap<i64, GrammarConstant>) -> String {
    render_tag_pairs(row, constants)
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>()
        .join(" • ")
}

pub fn render_tag_pairs(row: &QuizRow, constants: &HashMap<i64, GrammarConstant>) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    
    let mut person_str = None;
    let mut number_str = None;

    for category in DESCRIPTION_ORDER {
        if let Some(constant_id) = row.constant_ids.get(*category) {
            if let Some(constant) = constants.get(constant_id) {
                if *category == "person" {
                    person_str = Some(constant.display_label.clone());
                    continue;
                }
                if *category == "number" {
                    number_str = Some(constant.display_label.clone());
                    continue;
                }
                parts.push(((*category).to_string(), constant.display_label.clone()));
            }
            continue;
        }

        if let Some(values) = row.lemma_attributes.by_category.get(*category) {
            let joined = values
                .iter()
                .map(|value| value.display_label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if !joined.is_empty() {
                parts.push(((*category).to_string(), joined));
            }
        }
    }
    
    match (person_str, number_str) {
        (Some(person), Some(number)) => {
            let short_num = match number.as_str() {
                "Singular" => "s",
                "Plural" => "p",
                "Dual" => "d",
                _ => number.as_str(),
            };
            parts.push(("person_number".to_string(), format!("{}{}", person, short_num)));
        }
        (Some(person), None) => parts.push(("person".to_string(), person)),
        (None, Some(number)) => parts.push(("number".to_string(), number)),
        _ => {}
    }

    parts
}
