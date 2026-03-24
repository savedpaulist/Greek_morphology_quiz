use std::borrow::Cow;

use anyhow::{Result, anyhow, bail};

const CASE_ORDER: &[(&str, &str)] = &[
    ("nom", "N"),
    ("gen", "G"),
    ("dat", "D"),
    ("acc", "A"),
    ("voc", "V"),
];

const NUMBER_COLUMNS: &[(&str, &str)] = &[
    ("sg", "Singular"),
    ("du", "Dual"),
    ("pl", "Plural"),
];

const GENDER_COLUMNS: &[(&str, &str)] = &[
    ("masc", "M"),
    ("fem", "F"),
    ("neut", "N"),
];

const COMMON_NEUTER_COLUMNS: &[(&str, &str)] = &[("common", "Common"), ("neut", "Neuter")];

const FINITE_SLOTS: &[VerbSlot] = &[
    VerbSlot::new("1sg", "1", "Singular"),
    VerbSlot::new("2sg", "2", "Singular"),
    VerbSlot::new("3sg", "3", "Singular"),
    VerbSlot::new("2du", "2", "Dual"),
    VerbSlot::new("3du", "3", "Dual"),
    VerbSlot::new("1pl", "1", "Plural"),
    VerbSlot::new("2pl", "2", "Plural"),
    VerbSlot::new("3pl", "3", "Plural"),
];

const IMPERATIVE_SLOTS: &[VerbSlot] = &[
    VerbSlot::new("2sg", "2", "Singular"),
    VerbSlot::new("3sg", "3", "Singular"),
    VerbSlot::new("2du", "2", "Dual"),
    VerbSlot::new("3du", "3", "Dual"),
    VerbSlot::new("2pl", "2", "Plural"),
    VerbSlot::new("3pl", "3", "Plural"),
];

const FINITE_COMPACT_SLOTS: &[VerbSlot] = &[
    VerbSlot::new("1sg", "1", "Singular"),
    VerbSlot::new("2sg", "2", "Singular"),
    VerbSlot::new("3sg", "3", "Singular"),
    VerbSlot::new("1pl", "1", "Plural"),
    VerbSlot::new("2pl", "2", "Plural"),
    VerbSlot::new("3pl", "3", "Plural"),
];

const IMPERATIVE_COMPACT_SLOTS: &[VerbSlot] = &[
    VerbSlot::new("2sg", "2", "Singular"),
    VerbSlot::new("3sg", "3", "Singular"),
    VerbSlot::new("2pl", "2", "Plural"),
    VerbSlot::new("3pl", "3", "Plural"),
];

const TENSE_MARKERS: &[&str] = &[
    "present",
    "imperfect",
    "future",
    "aorist",
    "perfect",
    "pluperfect",
    "future_perfect",
];

const VOICE_MARKERS: &[&str] = &["active", "middle", "passive"];
const MOOD_MARKERS: &[&str] = &[
    "indicative",
    "subjunctive",
    "optative",
    "imperative",
    "infinitive",
    "participle",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParadigmFamily {
    Nominal,
    GenderedNominal,
    Verb,
}

impl ParadigmFamily {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::GenderedNominal => "gendered_nominal",
            Self::Verb => "verb",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParadigmEntry {
    pub title: String,
    pub family: ParadigmFamily,
    pub raw_payload: String,
    pub tables: Vec<ParadigmTable>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParadigmTable {
    pub kind: String,
    pub heading: String,
    pub columns: Vec<ParadigmColumn>,
    pub rows: Vec<ParadigmRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParadigmColumn {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParadigmRow {
    pub key: String,
    pub label: String,
    pub cells: Vec<ParadigmCell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParadigmCell {
    pub column_key: String,
    pub column_label: String,
    pub value: String,
    pub alternatives: Vec<String>,
}

#[derive(Clone, Debug)]
struct RawBlock {
    markers: Vec<String>,
    values: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct VerbSlot {
    person: &'static str,
    number: &'static str,
}

impl VerbSlot {
    const fn new(_key: &'static str, person: &'static str, number: &'static str) -> Self {
        Self { person, number }
    }
}

pub fn parse_paradigm(title: &str, payload: &str) -> Result<ParadigmEntry> {
    let blocks = tokenize_payload(payload)?;
    let family = detect_family(&blocks)?;
    let tables = match family {
        ParadigmFamily::Nominal | ParadigmFamily::GenderedNominal => parse_nominal_like_tables(&blocks)?,
        ParadigmFamily::Verb => parse_verb_tables(&blocks)?,
    };

    Ok(ParadigmEntry {
        title: title.to_string(),
        family,
        raw_payload: payload.to_string(),
        tables,
    })
}

#[cfg(test)]
fn normalize_answer(value: &str) -> String {
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

fn tokenize_payload(payload: &str) -> Result<Vec<RawBlock>> {
    let mut pending_markers = Vec::<String>::new();
    let mut blocks = Vec::<RawBlock>::new();

    for segment in payload.split('#').filter(|segment| !segment.is_empty()) {
        if let Some((marker, remainder)) = segment.split_once('&') {
            let mut markers = pending_markers.clone();
            if let Some(marker) = normalize_marker(marker) {
                markers.push(marker);
            }
            pending_markers.clear();

            let values = remainder
                .split('&')
                .map(|value| value.trim().to_string())
                .collect::<Vec<_>>();
            blocks.push(RawBlock { markers, values });
        } else {
            if let Some(marker) = normalize_marker(segment) {
                pending_markers.push(marker);
            }
        }
    }

    if blocks.is_empty() {
        bail!("payload did not produce any blocks");
    }

    Ok(blocks)
}

fn normalize_marker(segment: &str) -> Option<String> {
    let normalized = segment.trim().trim_matches('/').trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn detect_family(blocks: &[RawBlock]) -> Result<ParadigmFamily> {
    if blocks
        .iter()
        .flat_map(|block| block.markers.iter())
        .any(|marker| TENSE_MARKERS.contains(&marker.as_str()) || MOOD_MARKERS.contains(&marker.as_str()))
    {
        return Ok(ParadigmFamily::Verb);
    }

    let case_widths = blocks
        .iter()
        .filter(|block| is_case_marker(block.markers.first().map(String::as_str).unwrap_or_default()))
        .map(|block| classify_case_width(block.values.len()))
        .collect::<Result<Vec<_>>>()?;
    if case_widths.is_empty() {
        bail!("no case block found in nominal payload");
    }
    if case_widths
        .iter()
        .any(|width| matches!(width, CaseWidth::GenderedSix | CaseWidth::GenderedNine))
    {
        Ok(ParadigmFamily::GenderedNominal)
    } else {
        Ok(ParadigmFamily::Nominal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaseWidth {
    Generic(usize),
    Nominal,
    GenderedSix,
    GenderedNine,
}

fn parse_nominal_like_tables(blocks: &[RawBlock]) -> Result<Vec<ParadigmTable>> {
    let mut tables = Vec::new();
    let mut index = 0;
    let mut pending_label: Option<String> = None;

    while index < blocks.len() {
        let block = &blocks[index];
        let marker = block.markers[0].trim();

        if marker == "der" {
            tables.push(make_derivatives_table_with_heading(&block.values, "Derived forms"));
            index += 1;
            continue;
        }

        if is_case_marker(marker) {
            let width = classify_case_width(block.values.len())?;
            let start = index;
            index += 1;
            while index < blocks.len() {
                let next_marker = blocks[index].markers[0].trim();
                if !is_case_marker(next_marker) {
                    break;
                }
                if classify_case_width(blocks[index].values.len())? != width {
                    break;
                }
                index += 1;
            }

            let heading_prefix = pending_label.take();
            let mut group_tables = parse_case_group(&blocks[start..index], width, heading_prefix.as_deref())?;
            if index < blocks.len() && blocks[index].markers[0].trim() == "der" {
                let derivative_heading = heading_prefix
                    .as_deref()
                    .map(|label| format!("{} derived forms", label))
                    .unwrap_or_else(|| "Derived forms".to_string());
                group_tables.push(make_derivatives_table_with_heading(&blocks[index].values, &derivative_heading));
                index += 1;
            }
            tables.extend(group_tables);
            continue;
        }

        pending_label = Some(build_variant_label(block));
        index += 1;
    }

    Ok(tables)
}

fn parse_case_group(blocks: &[RawBlock], width: CaseWidth, heading_prefix: Option<&str>) -> Result<Vec<ParadigmTable>> {
    match width {
        CaseWidth::Generic(width) => parse_generic_case_group(blocks, width, heading_prefix),
        CaseWidth::Nominal => parse_nominal_group(blocks, heading_prefix),
        CaseWidth::GenderedSix => parse_gendered_group(blocks, 6, heading_prefix),
        CaseWidth::GenderedNine => parse_gendered_group(blocks, 9, heading_prefix),
    }
}

fn parse_generic_case_group(blocks: &[RawBlock], width: usize, heading_prefix: Option<&str>) -> Result<Vec<ParadigmTable>> {
    let columns = (0..width)
        .map(|index| {
            if width == 1 {
                ParadigmColumn {
                    key: "form".to_string(),
                    label: "Form".to_string(),
                }
            } else {
                ParadigmColumn {
                    key: format!("form_{}", index + 1),
                    label: format!("Form {}", index + 1),
                }
            }
        })
        .collect::<Vec<_>>();
    let column_defs = columns
        .iter()
        .map(|column| (column.key.as_str(), column.label.as_str()))
        .collect::<Vec<_>>();
    let rows = blocks
        .iter()
        .map(|block| make_case_row_from_values(block.markers[0].as_str(), block.values.clone(), &column_defs))
        .collect::<Result<Vec<_>>>()?;

    Ok(vec![ParadigmTable {
        kind: "nominal_irregular".to_string(),
        heading: make_heading(heading_prefix, "Nominal paradigm"),
        columns,
        rows,
    }])
}

fn parse_nominal_group(blocks: &[RawBlock], heading_prefix: Option<&str>) -> Result<Vec<ParadigmTable>> {
    let mut trailing_derivatives = Vec::new();
    let rows = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let mut values = block.values.clone();
            if index + 1 == blocks.len() && values.len() > NUMBER_COLUMNS.len() {
                trailing_derivatives = values.split_off(NUMBER_COLUMNS.len());
            }
            make_case_row_from_values(block.markers[0].as_str(), values, NUMBER_COLUMNS)
        })
        .collect::<Result<Vec<_>>>()?;
    let heading = make_heading(heading_prefix, "Nominal paradigm");
    let mut tables = vec![ParadigmTable {
        kind: "nominal".to_string(),
        heading,
        columns: NUMBER_COLUMNS.iter().map(make_column).collect(),
        rows,
    }];
    if !trailing_derivatives.is_empty() {
        let heading = make_heading(heading_prefix, "Derived forms");
        tables.push(make_derivatives_table_with_heading(&trailing_derivatives, &heading));
    }
    Ok(tables)
}

fn parse_gendered_group(blocks: &[RawBlock], expected_width: usize, heading_prefix: Option<&str>) -> Result<Vec<ParadigmTable>> {
    let mut tables = Vec::new();
    let mut trailing_derivatives = Vec::new();
    match expected_width {
        9 => {
            for (_, number_label) in NUMBER_COLUMNS {
                let number_index = NUMBER_COLUMNS
                    .iter()
                    .position(|(_, candidate)| candidate == number_label)
                    .ok_or_else(|| anyhow!("missing number column"))?;
                let rows = blocks
                    .iter()
                    .enumerate()
                    .map(|(index, block)| {
                        let values = trim_case_values(blocks.len(), index, &block.values, expected_width, &mut trailing_derivatives);
                        let values = values[number_index * 3..number_index * 3 + 3].to_vec();
                        make_case_row_from_values(block.markers[0].as_str(), values, GENDER_COLUMNS)
                    })
                    .collect::<Result<Vec<_>>>()?;
                tables.push(ParadigmTable {
                    kind: "gendered_nominal".to_string(),
                    heading: make_heading(heading_prefix, &format!("{} forms", number_label)),
                    columns: GENDER_COLUMNS.iter().map(make_column).collect(),
                    rows,
                });
            }
        }
        6 => {
            for (_, number_label) in NUMBER_COLUMNS {
                let number_index = NUMBER_COLUMNS
                    .iter()
                    .position(|(_, candidate)| candidate == number_label)
                    .ok_or_else(|| anyhow!("missing number column"))?;
                let rows = blocks
                    .iter()
                    .enumerate()
                    .map(|(index, block)| {
                        let values = trim_case_values(blocks.len(), index, &block.values, expected_width, &mut trailing_derivatives);
                        let values = values[number_index * 2..number_index * 2 + 2].to_vec();
                        make_case_row_from_values(block.markers[0].as_str(), values, COMMON_NEUTER_COLUMNS)
                    })
                    .collect::<Result<Vec<_>>>()?;
                tables.push(ParadigmTable {
                    kind: "gendered_nominal".to_string(),
                    heading: make_heading(heading_prefix, &format!("{} forms", number_label)),
                    columns: COMMON_NEUTER_COLUMNS.iter().map(make_column).collect(),
                    rows,
                });
            }
        }
        other => bail!("unsupported gendered nominal width: {other}"),
    }
    if !trailing_derivatives.is_empty() {
        let heading = make_heading(heading_prefix, "Derived forms");
        tables.push(make_derivatives_table_with_heading(&trailing_derivatives, &heading));
    }
    Ok(tables)
}

fn trim_case_values<'a>(block_count: usize, index: usize, values: &'a [String], expected_width: usize, trailing_derivatives: &mut Vec<String>) -> &'a [String] {
    if index + 1 == block_count && values.len() > expected_width {
        if trailing_derivatives.is_empty() {
            *trailing_derivatives = values[expected_width..].to_vec();
        }
        &values[..expected_width]
    } else {
        values
    }
}

fn classify_case_width(width: usize) -> Result<CaseWidth> {
    match width {
        1 | 2 => Ok(CaseWidth::Generic(width)),
        3 | 4 | 5 => Ok(CaseWidth::Nominal),
        6 | 7 | 8 => Ok(CaseWidth::GenderedSix),
        9 | 10 | 11 | 12 => Ok(CaseWidth::GenderedNine),
        other => Err(anyhow!("unsupported nominal cell count: {other}")),
    }
}

fn build_variant_label(block: &RawBlock) -> String {
    let marker = title_case(block.markers[0].trim());
    if block.values.is_empty() {
        marker
    } else {
        format!("{} {}", marker, block.values.join(" / ").trim())
    }
}

fn make_heading(prefix: Option<&str>, base: &str) -> String {
    match prefix {
        Some(prefix) => format!("{} | {}", prefix, base),
        None => base.to_string(),
    }
}

fn make_derivatives_table_with_heading(values: &[String], heading: &str) -> ParadigmTable {
    let derivative_columns = [
        ("adverb", "Adverb"),
        ("comparative", "Comparative"),
        ("superlative", "Superlative"),
    ];
    let row = ParadigmRow {
        key: "der".to_string(),
        label: "Derived".to_string(),
        cells: derivative_columns
            .iter()
            .zip(values.iter())
            .map(|((key, label), value)| make_cell(key, label, value))
            .collect(),
    };
    ParadigmTable {
        kind: "derivatives".to_string(),
        heading: heading.to_string(),
        columns: derivative_columns.iter().map(make_column).collect(),
        rows: vec![row],
    }
}

fn parse_verb_tables(blocks: &[RawBlock]) -> Result<Vec<ParadigmTable>> {
    let mut tables = Vec::new();
    let mut current_tense: Option<String> = None;
    let mut current_voices = Vec::<String>::new();

    for block in blocks {
        let markers = block.markers.iter().map(|marker| marker.as_str()).collect::<Vec<_>>();
        let tenses = markers
            .iter()
            .copied()
            .filter(|marker| TENSE_MARKERS.contains(marker))
            .collect::<Vec<_>>();
        if let Some(last_tense) = tenses.last() {
            current_tense = Some((*last_tense).to_string());
        }

        let voices = markers
            .iter()
            .copied()
            .filter(|marker| VOICE_MARKERS.contains(marker))
            .collect::<Vec<_>>();
        if !voices.is_empty() {
            current_voices = voices.iter().map(|voice| (*voice).to_string()).collect();
        }
        let moods = markers
            .iter()
            .copied()
            .filter(|marker| MOOD_MARKERS.contains(marker))
            .collect::<Vec<_>>();
        let variants = markers
            .iter()
            .copied()
            .filter(|marker| {
                !TENSE_MARKERS.contains(marker) && !VOICE_MARKERS.contains(marker) && !MOOD_MARKERS.contains(marker)
            })
            .collect::<Vec<_>>();
        let mood = moods
            .last()
            .copied()
            .ok_or_else(|| anyhow!("verb block missing mood marker: {}", markers.join("#")))?;
        let effective_voices = if voices.is_empty() {
            current_voices.iter().map(|voice| voice.as_str()).collect::<Vec<_>>()
        } else {
            voices.clone()
        };

        let heading = build_verb_heading(current_tense.as_deref(), &variants, &effective_voices, mood);
        match mood {
            "indicative" | "subjunctive" | "optative" => {
                tables.push(make_finite_table(&heading, &block.values, FINITE_SLOTS)?);
            }
            "imperative" => {
                tables.push(make_finite_table(&heading, &block.values, IMPERATIVE_SLOTS)?);
            }
            "infinitive" => {
                let nonfinite_voices = infer_nonfinite_voices(&effective_voices, block.values.len(), 1)?;
                let heading = build_verb_heading(current_tense.as_deref(), &variants, &nonfinite_voices, mood);
                tables.push(make_infinitive_table(&heading, &nonfinite_voices, &block.values)?);
            }
            "participle" => {
                let nonfinite_voices = infer_nonfinite_voices(&effective_voices, block.values.len(), 3)?;
                let heading = build_verb_heading(current_tense.as_deref(), &variants, &nonfinite_voices, mood);
                tables.push(make_participle_table(&heading, &nonfinite_voices, &block.values)?);
            }
            _ => bail!("unsupported mood marker: {mood}"),
        }
    }

    Ok(tables)
}

fn make_finite_table(heading: &str, values: &[String], slots: &[VerbSlot]) -> Result<ParadigmTable> {
    let effective_slots = if slots.as_ptr() == IMPERATIVE_SLOTS.as_ptr() {
        match values.len() {
            6 => IMPERATIVE_SLOTS,
            4 => IMPERATIVE_COMPACT_SLOTS,
            _ => return Ok(make_sequence_table("verb_irregular", heading, "form", "Form", values)),
        }
    } else {
        match values.len() {
            8 => FINITE_SLOTS,
            6 => FINITE_COMPACT_SLOTS,
            _ => return Ok(make_sequence_table("verb_irregular", heading, "form", "Form", values)),
        }
    };

    if values.len() != effective_slots.len() {
        bail!(
            "finite verb block {} expected {} values, got {}",
            heading,
            effective_slots.len(),
            values.len()
        );
    }

    let columns = NUMBER_COLUMNS.iter().map(make_column).collect::<Vec<_>>();
    let row_labels = if slots.as_ptr() == IMPERATIVE_SLOTS.as_ptr() {
        vec![("2", "2"), ("3", "3")]
    } else {
        vec![("1", "1"), ("2", "2"), ("3", "3")]
    };

    let mut rows = Vec::new();
    for (row_key, row_label) in row_labels {
        let mut cells = Vec::new();
        for (column_key, column_label) in NUMBER_COLUMNS {
            let value = effective_slots
                .iter()
                .zip(values.iter())
                .find(|(slot, _)| slot.person == row_key && slot.number == *column_label)
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "-".to_string());
            cells.push(make_cell(column_key, column_label, &value));
        }
        rows.push(ParadigmRow {
            key: row_key.to_string(),
            label: row_label.to_string(),
            cells,
        });
    }

    Ok(ParadigmTable {
        kind: "verb".to_string(),
        heading: heading.to_string(),
        columns,
        rows,
    })
}

fn make_sequence_table(kind: &str, heading: &str, column_key: &str, column_label: &str, values: &[String]) -> ParadigmTable {
    let column = ParadigmColumn {
        key: column_key.to_string(),
        label: column_label.to_string(),
    };
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| ParadigmRow {
            key: format!("slot_{}", index + 1),
            label: format!("Slot {}", index + 1),
            cells: vec![make_cell(column_key, column_label, value)],
        })
        .collect();

    ParadigmTable {
        kind: kind.to_string(),
        heading: heading.to_string(),
        columns: vec![column],
        rows,
    }
}

fn make_infinitive_table(heading: &str, voices: &[&str], values: &[String]) -> Result<ParadigmTable> {
    let effective_voices = if voices.is_empty() {
        vec!["form"]
    } else {
        voices.to_vec()
    };
    if values.len() != effective_voices.len() {
        bail!(
            "infinitive block {} expected {} values, got {}",
            heading,
            effective_voices.len(),
            values.len()
        );
    }

    let columns = effective_voices
        .iter()
        .map(|voice| (voice.to_string(), title_case(voice)))
        .map(|(key, label)| ParadigmColumn { key, label })
        .collect::<Vec<_>>();
    let row = ParadigmRow {
        key: "infinitive".to_string(),
        label: "Inf".to_string(),
        cells: effective_voices
            .iter()
            .zip(values.iter())
            .map(|(voice, value)| make_cell(voice, &title_case(voice), value))
            .collect(),
    };

    Ok(ParadigmTable {
        kind: "infinitive".to_string(),
        heading: heading.to_string(),
        columns,
        rows: vec![row],
    })
}

fn make_participle_table(heading: &str, voices: &[&str], values: &[String]) -> Result<ParadigmTable> {
    let effective_voices = if voices.is_empty() {
        vec!["form"]
    } else {
        voices.to_vec()
    };
    if values.len() != effective_voices.len() * 3 {
        bail!(
            "participle block {} expected {} values, got {}",
            heading,
            effective_voices.len() * 3,
            values.len()
        );
    }

    let columns = effective_voices
        .iter()
        .map(|voice| (voice.to_string(), title_case(voice)))
        .map(|(key, label)| ParadigmColumn { key, label })
        .collect::<Vec<_>>();

    let mut rows = Vec::new();
    for (gender_index, (gender_key, gender_label)) in GENDER_COLUMNS.iter().enumerate() {
        let mut cells = Vec::new();
        for (voice_index, voice) in effective_voices.iter().enumerate() {
            let index = gender_index * effective_voices.len() + voice_index;
            cells.push(make_cell(voice, &title_case(voice), &values[index]));
        }
        rows.push(ParadigmRow {
            key: gender_key.to_string(),
            label: gender_label.to_string(),
            cells,
        });
    }

    Ok(ParadigmTable {
        kind: "participle".to_string(),
        heading: heading.to_string(),
        columns,
        rows,
    })
}

fn infer_nonfinite_voices<'a>(voices: &[&'a str], value_count: usize, values_per_voice: usize) -> Result<Vec<&'a str>> {
    let inferred_voice_count = value_count / values_per_voice;
    if inferred_voice_count * values_per_voice != value_count {
        bail!(
            "non-finite block with {} values is not divisible by {}",
            value_count,
            values_per_voice
        );
    }

    if voices.is_empty() {
        if inferred_voice_count == 1 {
            return Ok(vec!["form"]);
        }
        bail!(
            "non-finite block has {} voice groups but no explicit voice markers",
            inferred_voice_count
        );
    }

    if inferred_voice_count == voices.len() {
        return Ok(voices.to_vec());
    }

    if inferred_voice_count == 1 {
        if voices == ["middle", "passive"] {
            return Ok(vec!["middle/passive"]);
        }
        if voices == ["active", "middle"] {
            return Ok(vec!["active/middle"]);
        }
        return Ok(vec!["form"]);
    }

    if voices == ["active", "middle", "passive"] && inferred_voice_count == 2 {
        return Ok(vec!["active", "middle/passive"]);
    }

    if voices == ["active", "middle"] && inferred_voice_count == 3 {
        return Ok(vec!["active", "middle", "passive"]);
    }

    if voices == ["middle"] && inferred_voice_count == 2 {
        return Ok(vec!["active", "middle"]);
    }

    bail!(
        "non-finite block expected {} voice groups from markers, got {}",
        voices.len(),
        inferred_voice_count
    )
}

fn make_case_row_from_values(case_marker: &str, values: Vec<String>, columns: &[(&str, &str)]) -> Result<ParadigmRow> {
    if values.len() != columns.len() {
        bail!(
            "case block {} expected {} values, got {}",
            case_marker,
            columns.len(),
            values.len()
        );
    }
    let (row_key, row_label) = case_label(case_marker)?;
    let cells = columns
        .iter()
        .zip(values.iter())
        .map(|((key, label), value)| make_cell(key, label, value))
        .collect();
    Ok(ParadigmRow {
        key: row_key.to_string(),
        label: row_label.to_string(),
        cells,
    })
}

fn make_column((key, label): &(&str, &str)) -> ParadigmColumn {
    ParadigmColumn {
        key: (*key).to_string(),
        label: (*label).to_string(),
    }
}

fn make_cell(key: &str, label: &str, value: &str) -> ParadigmCell {
    let alternatives = split_alternatives(value);
    ParadigmCell {
        column_key: key.to_string(),
        column_label: label.to_string(),
        value: value.to_string(),
        alternatives,
    }
}

fn split_alternatives(value: &str) -> Vec<String> {
    let delimiter = if value.contains(',') { ',' } else { '/' };
    value
        .split(delimiter)
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(|candidate| candidate.to_string())
        .collect()
}

fn build_verb_heading(tense: Option<&str>, variants: &[&str], voices: &[&str], mood: &str) -> String {
    let mut parts = Vec::<Cow<'_, str>>::new();
    if let Some(tense) = tense {
        parts.push(Cow::Owned(title_case(tense)));
    }
    for variant in variants {
        parts.push(Cow::Owned(title_case(variant)));
    }
    if !voices.is_empty() {
        parts.push(Cow::Owned(
            voices.iter().map(|voice| title_case(voice)).collect::<Vec<_>>().join(" / "),
        ));
    }
    parts.push(Cow::Owned(title_case(mood)));
    parts.join(" | ")
}

fn is_case_marker(marker: &str) -> bool {
    CASE_ORDER.iter().any(|(key, _)| *key == marker)
}

fn case_label(marker: &str) -> Result<(&'static str, &'static str)> {
    CASE_ORDER
        .iter()
        .find(|(key, _)| *key == marker)
        .copied()
        .ok_or_else(|| anyhow!("unknown case marker: {marker}"))
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
fn deaccent_greek(ch: char) -> Option<char> {
    match ch {
        'α' | 'Α' => Some('α'),
        'ά' | 'ὰ' | 'ᾶ' | 'ἀ' | 'ἁ' | 'ἄ' | 'ἅ' | 'ἂ' | 'ἃ' | 'ἆ' | 'ἇ' | 'ᾱ' | 'ᾰ' | 'ᾳ'
        | 'ᾴ' | 'ᾲ' | 'ᾷ' | 'Ἀ' | 'Ἁ' | 'Ἄ' | 'Ἅ' | 'Ἂ' | 'Ἃ' | 'Ἆ' | 'Ἇ' | 'Ὰ' | 'Ά' => Some('α'),
        'ε' | 'Ε' => Some('ε'),
        'έ' | 'ὲ' | 'ἐ' | 'ἑ' | 'ἔ' | 'ἕ' | 'ἒ' | 'ἓ' | 'Έ' | 'Ἐ' | 'Ἑ' | 'Ἔ' | 'Ἕ' | 'Ἒ' | 'Ἓ' => Some('ε'),
        'η' | 'Η' => Some('η'),
        'ή' | 'ὴ' | 'ῆ' | 'ἠ' | 'ἡ' | 'ἤ' | 'ἥ' | 'ἢ' | 'ἣ' | 'ἦ' | 'ἧ' | 'ῃ' | 'ῄ' | 'ῂ' | 'ῇ'
        | 'Ή' | 'Ἠ' | 'Ἡ' | 'Ἤ' | 'Ἥ' | 'Ἢ' | 'Ἣ' | 'Ἦ' | 'Ἧ' => Some('η'),
        'ι' | 'Ι' => Some('ι'),
        'ί' | 'ὶ' | 'ῖ' | 'ἰ' | 'ἱ' | 'ἴ' | 'ἵ' | 'ἲ' | 'ἳ' | 'ἶ' | 'ἷ' | 'ϊ' | 'ΐ' | 'ῒ' | 'ῗ'
        | 'Ί' | 'Ἰ' | 'Ἱ' | 'Ἴ' | 'Ἵ' | 'Ἲ' | 'Ἳ' | 'Ἶ' | 'Ἷ' => Some('ι'),
        'ο' | 'Ο' => Some('ο'),
        'ό' | 'ὸ' | 'ὀ' | 'ὁ' | 'ὄ' | 'ὅ' | 'ὂ' | 'ὃ' | 'Ό' | 'Ὀ' | 'Ὁ' | 'Ὄ' | 'Ὅ' | 'Ὂ' | 'Ὃ' => Some('ο'),
        'υ' | 'Υ' => Some('υ'),
        'ύ' | 'ὺ' | 'ῦ' | 'ὐ' | 'ὑ' | 'ὔ' | 'ὕ' | 'ὒ' | 'ὓ' | 'ὖ' | 'ὗ' | 'ϋ' | 'ΰ' | 'ῢ' | 'ῧ'
        | 'Ύ' | 'Ὑ' | 'Ὕ' | 'Ὓ' | 'Ὗ' => Some('υ'),
        'ω' | 'Ω' => Some('ω'),
        'ώ' | 'ὼ' | 'ῶ' | 'ὠ' | 'ὡ' | 'ὤ' | 'ὥ' | 'ὢ' | 'ὣ' | 'ὦ' | 'ὧ' | 'ῳ' | 'ῴ' | 'ῲ' | 'ῷ'
        | 'Ώ' | 'Ὠ' | 'Ὡ' | 'Ὤ' | 'Ὥ' | 'Ὢ' | 'Ὣ' | 'Ὦ' | 'Ὧ' => Some('ω'),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nominal_table() {
        let payload = "#nom&ὁ ᾰ̓γών&τὼ ᾰ̓γῶνε&οἱ ᾰ̓γῶνες#gen&τοῦ ᾰ̓γῶνος&τοῖν ᾰ̓γώνοιν&τῶν ᾰ̓γώνων#dat&τῷ ᾰ̓γῶνῐ&τοῖν ᾰ̓γώνοιν&τοῖς ᾰ̓γῶσῐ(ν)#acc&τὸν ᾰ̓γῶνᾰ&τὼ ᾰ̓γῶνε&τοὺς ᾰ̓γῶνᾰς#voc&ᾰ̓γών&ᾰ̓γῶνε&ᾰ̓γῶνες";
        let entry = parse_paradigm("ἀγών", payload).unwrap();
        assert_eq!(entry.family, ParadigmFamily::Nominal);
        assert_eq!(entry.tables.len(), 1);
        assert_eq!(entry.tables[0].columns.len(), 3);
        assert_eq!(entry.tables[0].rows.len(), 5);
    }

    #[test]
    fn parses_gendered_nominal_and_derivatives() {
        let payload = "#nom&ᾰ̓γᾰθός&ᾰ̓γᾰθή&ᾰ̓γᾰθόν&ᾰ̓γᾰθώ&ᾰ̓γᾰθᾱ́&ᾰ̓γᾰθώ&ᾰ̓γᾰθοί&ᾰ̓γᾰθαί&ᾰ̓γᾰθᾰ́#gen&ᾰ̓γᾰθοῦ&ᾰ̓γᾰθῆς&ᾰ̓γᾰθοῦ&ᾰ̓γᾰθοῖν&ᾰ̓γᾰθαῖν&ᾰ̓γᾰθοῖν&ᾰ̓γᾰθῶν&ᾰ̓γᾰθῶν&ᾰ̓γᾰθῶν#dat&ᾰ̓γᾰθῷ&ᾰ̓γᾰθῇ&ᾰ̓γᾰθῷ&ᾰ̓γᾰθοῖν&ᾰ̓γᾰθαῖν&ᾰ̓γᾰθοῖν&ᾰ̓γᾰθοῖς&ᾰ̓γᾰθαῖς&ᾰ̓γᾰθοῖς#acc&ᾰ̓γᾰθόν&ᾰ̓γᾰθήν&ᾰ̓γᾰθόν&ᾰ̓γᾰθώ&ᾰ̓γᾰθᾱ́&ᾰ̓γᾰθώ&ᾰ̓γᾰθούς&ᾰ̓γᾰθᾱ́ς&ᾰ̓γᾰθᾰ́#voc&ᾰ̓γᾰθέ&ᾰ̓γᾰθή&ᾰ̓γᾰθόν&ᾰ̓γᾰθώ&ᾰ̓γᾰθᾱ́&ᾰ̓γᾰθώ&ᾰ̓γᾰθοί&ᾰ̓γᾰθαί&ᾰ̓γᾰθᾰ́#der&ᾰ̓γᾰθῶς&ᾰ̓γᾰθώτερος/ἀμείνων&ᾰ̓γᾰθώτᾰτος/ἄριστος";
        let entry = parse_paradigm("ἀγαθός", payload).unwrap();
        assert_eq!(entry.family, ParadigmFamily::GenderedNominal);
        assert_eq!(entry.tables.len(), 4);
        assert_eq!(entry.tables[0].columns.len(), 3);
        assert_eq!(entry.tables[3].kind, "derivatives");
        assert_eq!(entry.tables[3].rows[0].cells[1].alternatives.len(), 2);
    }

    #[test]
    fn parses_verb_tables() {
        let payload = "#present#active#indicative&ἀγγέλλω&ἀγγέλλεις&ἀγγέλλει&ἀγγέλλετον&ἀγγέλλετον&ἀγγέλλομεν&ἀγγέλλετε&ἀγγέλλουσῐ(ν)#imperative&ἄγγελλε&ἀγγελλέτω&ἀγγέλλετον&ἀγγελλέτων&ἀγγέλλετε&ἀγγελλόντων#active#middle#infinitive&ἀγγέλλειν&ἀγγέλλεσθαι#participle&ἀγγέλλων&ἀγγελλόμενος&ἀγγέλλουσᾰ&ἀγγελλομένη&ἀγγέλλον&ἀγγελλόμενον";
        let entry = parse_paradigm("ἀγγέλλω", payload).unwrap();
        assert_eq!(entry.family, ParadigmFamily::Verb);
        assert_eq!(entry.tables.len(), 4);
        assert_eq!(entry.tables[0].rows.len(), 3);
        assert_eq!(entry.tables[1].rows.len(), 2);
        assert_eq!(entry.tables[2].kind, "infinitive");
        assert_eq!(entry.tables[3].kind, "participle");
    }

    #[test]
    fn normalizes_diacritics_and_final_sigma() {
        assert_eq!(normalize_answer("ἄγγελλε"), "αγγελλε");
        assert_eq!(normalize_answer("λόγος"), "λογοσ");
        assert_eq!(normalize_answer("ΑΥΤΌΣ"), "αυτοσ");
    }

    #[test]
    fn normalizes_slashy_markers_and_keeps_variant_headings() {
        let payload = "#present#epic#active#indicative&εἰμί&ἐσσί&ἐστί&ἐστόν&ἐστόν&εἰμέν&ἐστέ&εἰσί(ν)#imperfect/#epic#active#indicative&ἦα&ἔησθα&ἦεν&ἤστον&ἤστην&ἦμεν&ἦτε&ἦσαν";
        let entry = parse_paradigm("εἰμί", payload).unwrap();
        assert_eq!(entry.tables[0].heading, "Present | Epic | Active | Indicative");
        assert_eq!(entry.tables[1].heading, "Imperfect | Epic | Active | Indicative");
    }
}