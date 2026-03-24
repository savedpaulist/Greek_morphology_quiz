use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use dioxus::prelude::*;

use crate::db::Database;
use crate::models::{FilterSection, FilterState, GrammarConstant, LemmaOption, Question, QuizMode, QuizRow, SessionStats, StemView};
use crate::quiz;

const APP_CSS: &str = r#"
body {
    margin: 0;
    font-family: "Iowan Old Style", "Palatino Linotype", serif;
    background: radial-gradient(circle at top left, #eaf4f4 0%, #dceaea 55%, #cce0e0 100%);
    color: #0c2526;
}

#main {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    width: 100%;
    overflow-x: hidden;
}

.sidebar {
    position: fixed;
    top: 12px;
    right: 12px;
    bottom: 12px;
    width: min(380px, calc(100% - 24px));
    box-sizing: border-box;
    padding: 18px 16px 20px;
    background: rgba(228, 248, 248, 0.85);
    backdrop-filter: blur(10px);
    overflow: hidden;
    z-index: 30;
    box-shadow: -16px 0 40px rgba(23, 42, 10, 0.12);
    border-left: 1px solid rgba(26, 69, 71, 0.14);
    border-radius: 24px;
    transform: translateX(calc(100% + 24px));
    transition: transform 180ms ease;
    display: flex;
    flex-direction: column;
}

.sidebar.open {
    transform: translateX(0);
}

.content {
    flex: 1;
    padding: 16px 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    order: 1;
}

.content-top {
    display: grid;
    gap: 6px;
}

.header-row {
    display: grid;
    gap: 6px;
}

.header-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
}

.eyebrow {
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.15em;
    font-weight: 600;
    color: #3d7275;
    margin-bottom: 8px;
}

.title {
    font-size: clamp(22px, 6vw, 32px);
    line-height: 1;
    margin: 0 0 4px;
}

.subtle {
    color: #58522c;
    font-size: 13px;
    line-height: 1.45;
}

.stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 5px;
    width: 100%;
}

.stat-card,
.panel,
.question-card {
    background: rgba(236, 250, 250, 0.84);
    border: 1px solid rgba(26, 69, 71, 0.14);
    border-radius: 20px;
    box-shadow: 0 8px 32px rgba(26, 69, 71, 0.08);
    width: 100%;
    box-sizing: border-box;
}

.stat-card {
    padding: 8px 8px;
    min-width: 0;
    background: rgba(255, 247, 230, 0.96);
    display: flex;
    flex-direction: column;
    justify-content: center;
}

.stat-value {
    font-size: clamp(22px, 5vw, 32px);
    margin: 4px 0 0;
    font-weight: 500;
}

.panel {
    padding: 8px;
}

.actions-row,
.question-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
}

.mode-row,
.screen-row {
    display: flex;
    flex-wrap: nowrap;
    gap: 12px;
}

.top-controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-start;
    gap: 12px;
    padding-bottom: 4px;
    margin-bottom: 4px;
}

.control-block {
    flex: 0 0 auto;
}

.drawer-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
}

.drawer-fixed {
    flex: 0 0 auto;
    display: grid;
    gap: 8px;
    padding-bottom: 10px;
}

.drawer-scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
}

.drawer-close {
    min-width: 44px;
    min-height: 44px;
    font-size: 20px;
    line-height: 1;
}

.filter-launch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
}

.filter-trigger {
    min-height: 48px;
    padding: 12px 20px;
    font-size: 15px;
}

.filter-status {
    color: #6c5a40;
    font-size: 12px;
    line-height: 1.35;
}

.drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(32, 23, 11, 0.18);
    backdrop-filter: blur(3px);
    z-index: 20;
}

.mode-button,
.screen-button,
.action-button,
.answer-button {
    border: 1px solid rgba(90, 65, 28, 0.18);
    background: rgba(255, 247, 230, 0.96);
    color: #2f271b;
    border-radius: 999px;
    padding: 12px 18px;
    font-size: 15px;
    cursor: pointer;
    transition: transform 120ms ease, background 120ms ease, border-color 120ms ease;
}

.mode-button,
.screen-button {
    padding: 15px 28px;
    font-size: 16px;
    min-height: 52px;
    white-space: nowrap;
}

.mode-button:hover,
.screen-button:hover,
.action-button:hover,
.answer-button:hover {
    transform: translateY(-1px);
    border-color: rgba(90, 65, 28, 0.34);
}

.mode-button.active,
.screen-button.active {
    background: #47311a;
    color: #fbf3e6;
    border-color: #1a4547;
}

.select-input,
.dropdown-summary {
    width: 100%;
    box-sizing: border-box;
    padding: 12px 14px;
    border-radius: 14px;
    border: 1px solid rgba(90, 65, 28, 0.16);
    background: rgba(255, 252, 245, 0.92);
    color: #2f271b;
    margin-top: 8px;
    font: inherit;
}

.section + .section {
    margin-top: 18px;
}

.section-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 8px;
}

.dropdown {
    margin-top: 10px;
}

.filters-shell {
    margin-top: 8px;
}

.dropdown[open] .dropdown-summary {
    border-bottom-left-radius: 10px;
    border-bottom-right-radius: 10px;
}

.dropdown-summary {
    list-style: none;
    cursor: pointer;
}

.dropdown-summary::-webkit-details-marker {
    display: none;
}

.dropdown-summary.disabled,
.select-input:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}

.dropdown-body {
    margin-top: 6px;
    padding: 10px;
    border-radius: 14px;
    border: 1px solid rgba(90, 65, 28, 0.12);
    background: rgba(255, 252, 245, 0.9);
    display: grid;
    gap: 6px;
}

.dropdown-option {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 6px;
    border-radius: 10px;
}

.dropdown-option.active {
    background: rgba(71, 49, 26, 0.08);
}

.dropdown-option.disabled {
    opacity: 0.4;
}

.dropdown-checkbox {
    width: 16px;
    height: 16px;
}

.screen-button {
    min-width: 120px;
}

.question-card {
    flex: 1;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: stretch;
    justify-content: space-between;
}

.question-header {
    text-align: center;
    display: grid;
    gap: 4px;
}

.question-prompt {
    font-size: clamp(30px, 7vw, 64px);
    line-height: 1.05;
    margin: 0;
    text-align: center;
}

.answer-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(280px, 1fr));
    gap: 8px;
    width: 100%;
}

.question-clue {
    margin: 0;
    text-align: center;
    color: #5f5435;
    font-size: 15px;
}

.text-answer-panel {
    display: grid;
    gap: 8px;
    width: 100%;
}

.text-answer-input {
    width: 100%;
    box-sizing: border-box;
    padding: 12px 14px;
    border-radius: 18px;
    border: 1px solid rgba(90, 65, 28, 0.16);
    background: rgba(255, 252, 245, 0.96);
    color: #2f271b;
    font: inherit;
    font-size: 22px;
    text-align: center;
}

.answer-feedback {
    border-radius: 16px;
    padding: 14px 16px;
    font-size: 15px;
    line-height: 1.45;
}

.answer-feedback.correct {
    background: rgba(67, 124, 61, 0.12);
    border: 1px solid rgba(67, 124, 61, 0.24);
    color: #254d23;
}

.answer-feedback.wrong {
    background: rgba(141, 71, 43, 0.11);
    border: 1px solid rgba(141, 71, 43, 0.2);
    color: #6d2b1a;
}

.answer-button {
    border-radius: 16px;
    text-align: left;
    padding: 12px 16px;
    min-height: auto;
    font-size: 15px;
    line-height: 1.4;
    white-space: normal;
    display: flex;
    align-items: center;
}

.answer-button.correct {
    background: #3f5c3c;
    color: #f6f3ec;
    border-color: #3f5c3c;
}

.answer-button.wrong {
    background: #864636;
    color: #f8f2eb;
    border-color: #864636;
}

.empty {
    padding: 28px;
    text-align: center;
}

.stem-card {
    display: grid;
    gap: 18px;
    min-width: 0;
}

.stem-common {
    font-size: 18px;
    line-height: 1.5;
}

.stemtype-banner {
    font-size: 16px;
    line-height: 1.5;
    padding: 14px 16px;
    border-radius: 14px;
    background: rgba(255, 248, 236, 0.82);
    border: 1px solid rgba(82, 60, 27, 0.12);
}

.stem-table-list {
    display: grid;
    gap: 18px;
    min-width: 0;
}

.stem-table {
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    border-radius: 16px;
    background: rgba(255, 248, 236, 0.88);
    border: 1px solid rgba(82, 60, 27, 0.12);
    overflow-x: auto;
    max-width: 100%;
    box-sizing: border-box;
    -webkit-overflow-scrolling: touch;
}

.stem-table-heading {
    font-size: 18px;
    line-height: 1.4;
}

.paradigm-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: auto;
}

.paradigm-table th,
.paradigm-table td {
    border-bottom: 1px solid rgba(82, 60, 27, 0.12);
    padding: 12px 10px;
    text-align: center;
    vertical-align: middle;
    word-wrap: break-word;
}

.paradigm-table th:first-child,
.paradigm-table td:first-child {
    text-align: left;
    width: 1%;
    white-space: nowrap;
    padding-right: 12px;
}

.paradigm-table th {
    color: #6e5d43;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
}

.paradigm-table td {
    font-size: 22px;
    line-height: 1.25;
}

.question-actions {
    justify-content: space-between;
    align-items: center;
}

.next-button {
    padding: 10px 18px;
    width: min(50%, 320px);
    margin-left: auto;
    font-size: 15px;
    background: #47311a;
    color: #fbf3e6;
    border-color: #47311a;
}

@media (min-width: 1080px) {
    .answer-grid {
        grid-template-columns: repeat(2, minmax(280px, 1fr));
    }
}

@media (max-width: 640px) {
    #main {
        height: 100dvh;
        overflow: hidden;
    }

    .content {
        padding: clamp(12px, 3vh, 24px) 16px;
        gap: clamp(8px, 1.5vh, 12px);
        overflow-y: auto;
    }

    .sidebar {
        padding: 24px 16px;
    }

    .content-top {
        gap: clamp(6px, 1vh, 10px);
    }

    .header-row {
        gap: 2px;
    }

    /* hide the long subtitle on mobile to save vertical space */
    .header-row > .subtle {
        display: none;
    }

    .eyebrow {
        font-size: 10px;
        margin-bottom: 4px;
    }

    .title {
        font-size: 20px;
        margin: 0;
    }

    .header-topbar {
        gap: 6px;
    }

    .top-controls {
            gap: 6px;
            align-items: center;
            flex-wrap: nowrap;
            overflow-x: auto;
            -webkit-overflow-scrolling: touch;
            scrollbar-width: none;
        }

        .top-controls::-webkit-scrollbar {
            display: none;
        }

        .mode-row,
        .screen-row {
            gap: 4px;
        }

        .section-title {
            font-size: 12px;
            margin-bottom: 6px;
        }

        .mode-button,
        .screen-button {
            padding: clamp(6px, 1.5vh, 10px) 10px;
            font-size: clamp(12px, 3.5vw, 14px);
            min-height: clamp(36px, 6vh, 42px);
        }

        .screen-button {
            min-width: 70px;
        }

        .action-button {
            padding: clamp(6px, 1.5vh, 10px) 12px;
            font-size: 13px;
            white-space: nowrap;
        }

    .stats {
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 6px;
    }

    .stat-card {
        padding: 8px 6px;
    }

    .stat-card .eyebrow {
        font-size: 8px;
        letter-spacing: 0.05em;
        margin-bottom: 2px;
        text-align: center;
    }

    .stat-value {
        font-size: clamp(14px, 4vw, 18px);
        margin: 0;
        text-align: center;
    }

    .question-card {
        padding: clamp(8px, 1.5vh, 12px) 12px;
        gap: clamp(8px, 1.5vh, 12px);
    }

    .question-header {
        gap: clamp(2px, 0.5vh, 4px);
    }

    .question-prompt {
        font-size: clamp(22px, 15vw, 36px);
    }

    .answer-grid {
        grid-template-columns: 1fr;
        gap: clamp(4px, 1vh, 8px);
    }

    .answer-button {
        min-height: auto;
        padding: clamp(8px, 1.2vh, 12px) 14px;
        font-size: clamp(14px, 4vw, 16px);
    }

    .panel {
        padding: 10px 12px;
    }

    .question-actions {
        gap: 6px;
    }

    .next-button {
        width: auto;
        flex: 1;
        min-width: 0;
        padding: 10px 14px;
    }

    .action-button.next-button ~ .action-button,
    .question-actions .action-button:first-child {
        flex: 0 0 auto;
    }

    .section + .section {
        margin-top: 10px;
    }

        .stem-table {
            padding: 8px 4px;
            border-radius: 12px;
            overflow-x: hidden;
        }

        .paradigm-table {
            table-layout: fixed;
            width: 100%;
        }

        .paradigm-table th,
        .paradigm-table td {
            padding: 6px 2px;
            overflow: hidden;
        }

        .paradigm-table th:first-child,
        .paradigm-table td:first-child {
            padding-right: 4px;
            width: 25px; /* hardcode small width so it doesn't take 33% */
        }

        .paradigm-table td {
            font-size: clamp(12px, 3.2vw, 15px);
            word-break: break-all;
            overflow-wrap: anywhere;
            hyphens: auto;
        }
    }
"#;

pub fn app_root() -> Element {
    let app = use_signal(|| AppState::load().map_err(|error| error.to_string()));
    let snapshot = app.read().clone();

    let view = match snapshot {
        Ok(state) => render_app(app, state),
        Err(error) => rsx! {
            div { id: "main",
                aside { class: "sidebar",
                    div { class: "eyebrow", "Greek morphology" }
                    h1 { class: "title", "Startup error" }
                }
                main { class: "content",
                    div { class: "panel empty",
                        p { class: "subtle", "{error}" }
                    }
                }
            }
        },
    };

    rsx! {
        document::Title { "Ancient Greek Morpheme Test" }
        document::Style { "{APP_CSS}" }
        document::Link {
            rel: "icon",
            href: "/assets/icons/web/favicon.ico",
        }
        document::Link {
            rel: "icon",
            r#type: "image/png",
            sizes: "32x32",
            href: "/assets/icons/web/icon-32x32.png",
        }
        document::Link {
            rel: "icon",
            r#type: "image/png",
            sizes: "192x192",
            href: "/assets/icons/web/icon-192x192.png",
        }
        document::Link {
            rel: "apple-touch-icon",
            href: "/assets/icons/web/icon-192x192.png",
        }
        document::Link {
            rel: "apple-touch-icon",
            sizes: "512x512",
            href: "/assets/icons/web/icon-512x512.png",
        }
        {view}
    }
}

fn render_app(app: Signal<Result<AppState, String>>, state: AppState) -> Element {
    let sections = state.filter_sections.clone();
    let selected_filters = state.filters.clone();
    let question = state.current_question.clone();
    let stats = state.stats.clone();
    let selected_lemma_ids = state.filters.selected_lemma_ids.clone();
    let lemma_search_query = state.lemma_search_query.clone();
    let preset_name_input = state.preset_name_input.clone();
    let presets = state.presets.clone();
    let lemma_options = state.lemma_options.clone();
    let availability = state.availability.clone();
    let pool_size = state.pool.len();
    let mode = state.mode;
    let screen = state.screen;
    let stem_view = state.stem_view.clone();
    let filter_drawer_open = state.filter_drawer_open;
    let active_filters = state.filters.selected_constants.values().map(|set| set.len()).sum::<usize>();

    rsx! {
        div { id: "main",
            if filter_drawer_open {
                div {
                    class: "drawer-backdrop",
                    onclick: move |_| mutate_app(app, |state| state.set_filter_drawer(false)),
                }
            }

            main { class: "content",
                div { class: "content-top",
                    div { class: "top-controls",
                        button {
                            class: "action-button filter-trigger",
                            onclick: move |_| mutate_app(app, |state| state.set_filter_drawer(true)),
                            "Filters"
                        }
                        
                        button {
                            class: if screen == Screen::Quiz { "screen-button active" } else { "screen-button" },
                            onclick: move |_| mutate_app(app, |state| state.set_screen(Screen::Quiz)),
                            "Q"
                        }
                        
                        button {
                            class: if screen == Screen::Stems { "screen-button active" } else { "screen-button" },
                            onclick: move |_| mutate_app(app, |state| state.set_screen(Screen::Stems)),
                            "Stems"
                        }

                        button {
                            class: if mode == QuizMode::ParseForm || mode == QuizMode::BuildForm { "mode-button active" } else { "mode-button" },
                            onclick: move |_| {
                                let next_mode = if mode == QuizMode::ParseForm { QuizMode::BuildForm } else { QuizMode::ParseForm };
                                mutate_app(app, |state| state.set_mode(next_mode));
                            },
                            if mode == QuizMode::BuildForm { "A \u{2192} F" } else { "F \u{2192} A" }
                        }

                        button {
                            class: if mode == QuizMode::InferForm { "mode-button active" } else { "mode-button" },
                            onclick: move |_| mutate_app(app, |state| state.set_mode(QuizMode::InferForm)),
                            "Typed"
                        }

                        if active_filters > 0 || !selected_lemma_ids.is_empty() {
                            div { class: "filter-status",
                                if !selected_lemma_ids.is_empty() {
                                    {
                                        let count = selected_lemma_ids.len();
                                        let label_str = if count == 1 { "1 word".to_string() } else { format!("{} words", count) };
                                        rsx! {
                                            "{label_str}"
                                            if active_filters > 0 { " | " }
                                        }
                                    }
                                }
                                if active_filters > 0 {
                                    "{active_filters} active"
                                }
                            }
                        }
                    }
                }

                {
                    if screen == Screen::Quiz {
                        rsx! {
                            div { class: "stats",
                                {render_stat_card("Answered", stats.answered.to_string())}
                                {render_stat_card("Accuracy", format!("{}%", stats.accuracy()))}
                                {render_stat_card("Streak", stats.streak.to_string())}
                                {render_stat_card("Best", stats.best_streak.to_string())}
                            }

                            {
                                if let Some(question) = question {
                                    render_question_card(app, question)
                                } else {
                                    rsx! {
                                        div { class: "panel empty",
                                            h2 { "No question available" }
                                            p { class: "subtle", "The current filter combination does not produce a safe question yet." }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        render_stems_screen(stem_view)
                    }
                }
            }

            aside { class: if filter_drawer_open { "sidebar open" } else { "sidebar" },
                div { class: "drawer-fixed",
                    div { class: "drawer-actions",
                        div {
                            div { class: "eyebrow", "Settings" }
                            h2 { class: "title", "Filters" }
                        }
                        div { style: "display: flex; gap: 8px; align-items: center;",
                            button {
                                class: "action-button",
                                onclick: move |_| mutate_app(app, |state| state.clear_filters()),
                                "Clear"
                            }
                            button {
                                class: "action-button drawer-close",
                                onclick: move |_| mutate_app(app, |state| state.set_filter_drawer(false)),
                                "×"
                            }
                        }
                    }

                    details { class: "filters-shell dropdown", open: true,
                        summary { class: "dropdown-summary",
                            "Current selection"
                            if active_filters > 0 || !selected_lemma_ids.is_empty() {
                                " ("
                                if !selected_lemma_ids.is_empty() {
                                    {
                                        let count = selected_lemma_ids.len();
                                        let label_str = if count == 1 { "1 word".to_string() } else { format!("{} words", count) };
                                        rsx! {
                                            "{label_str}"
                                            if active_filters > 0 { ", " }
                                        }
                                    }
                                }
                                if active_filters > 0 {
                                    "{active_filters} active"
                                }
                                ")"
                            }
                        }
                    }
                }

                div { class: "drawer-scroll",
                    div { class: "section",
                        details { class: "filters-shell dropdown", open: false,
                            summary { class: "dropdown-summary", style: "font-size: 1.1em; font-weight: 500;", "Presets" }
                            div { class: "dropdown-body",
                                div { class: "search-box", style: "display: flex; gap: 8px;",
                                input {
                                    class: "select-input",
                                    r#type: "text",
                                    placeholder: "Preset name...",
                                    value: "{preset_name_input}",
                                    oninput: move |event| {
                                        mutate_app(app, |state| {
                                            state.preset_name_input = event.value().clone();
                                            Ok(())
                                        });
                                    }
                                }
                                button {
                                    class: "action-button",
                                    style: "padding: 8px 12px;",
                                    onclick: move |_| {
                                        mutate_app(app, |state| state.save_current_preset());
                                    },
                                    "Save"
                                }
                            }
                            if !presets.is_empty() {
                                div { class: "options-list", style: "margin-top: 8px; display: flex; flex-direction: column; gap: 4px;",
                                    for (name, _) in presets.clone() {
                                        div { class: "preset-row", style: "display: flex; justify-content: space-between; align-items: center;",
                                            button {
                                                style: "flex-grow: 1; text-align: left; padding: 6px 4px; background: none; border: none; cursor: pointer; color: var(--text-color); font-size: 1.1em;",
                                                onclick: {
                                                    let n = name.clone();
                                                    move |_| mutate_app(app, |state| state.load_preset(&n))
                                                },
                                                "{name}"
                                            }
                                            button {
                                                style: "padding: 6px 4px; background: none; border: none; cursor: pointer; color: #ff4444; font-size: 1.2em;",
                                                onclick: {
                                                    let n = name.clone();
                                                    move |_| mutate_app(app, |state| state.delete_preset(&n))
                                                },
                                                "✕" // cross
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "section",
                        div { class: "section-title", "Words" }
                            div { class: "search-box",
                                input {
                                    class: "select-input",
                                    r#type: "text",
                                    placeholder: "Search words...",
                                    value: "{lemma_search_query}",
                                    oninput: move |event| {
                                        mutate_app(app, |state| {
                                            state.lemma_search_query = event.value().clone();
                                            Ok(())
                                        });
                                    }
                                }
                            }
                            div { class: "options-list scrollable-list", style: "max-height: 300px; overflow-y: auto; margin-top: 8px;",
                                for lemma in lemma_options.iter().filter(|l| {
                                    if lemma_search_query.is_empty() { return true; }
                                    let q = crate::quiz::deaccent_greek_string(&lemma_search_query.to_lowercase());
                                    let t = crate::quiz::deaccent_greek_string(&l.label.to_lowercase());
                                    t.contains(&q)
                                }) {
                                    div {
                                        class: "checkbox-row",
                                        label { class: "checkbox-label",
                                            input {
                                                r#type: "checkbox",
                                                checked: selected_lemma_ids.contains(&lemma.id),
                                                onchange: {
                                                    let lemma_id = lemma.id;
                                                    move |_| {
                                                        mutate_app(app, |state| {
                                                            state.toggle_lemma(lemma_id)
                                                        });
                                                    }
                                                },
                                            }
                                            "{lemma.label}"
                                        }
                                    }
                                }
                            }
                        }

                        for section in sections {
                            {
                                let section_key = section.key.clone();
                                let section_title = section.title.clone();
                                let options = section.options.clone();
                                let available_ids = availability.get(&section_key).cloned().unwrap_or_default();
                                let selected_count = selected_filters
                                    .selected_constants
                                    .get(&section_key)
                                    .map(|values| values.len())
                                    .unwrap_or_default();

                                rsx! {
                                    div { class: "section", key: "{section_key}",
                                        div { class: "section-title", "{section_title}" }
                                        {
                                            if available_ids.is_empty() {
                                                rsx! {
                                                    div { class: "dropdown-summary disabled",
                                                        "Unavailable for current filter overlap"
                                                    }
                                                }
                                            } else {
                                                rsx! {
                                                    details { class: "dropdown",
                                                        summary { class: "dropdown-summary",
                                                            "{section_title}"
                                                            if selected_count > 0 {
                                                                " ({selected_count})"
                                                            }
                                                        }
                                                        div { class: "dropdown-body",
                                                            for option in options {
                                                                {render_filter_option(app, selected_filters.clone(), section_key.clone(), available_ids.clone(), option)}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        div { class: "section", style: "display: flex; 
                        font_size: 40px;flex-direction: column; gap: 15px;",
                            p { class: "subtle", "{active_filters} active grammar filters, {pool_size} candidate rows in current pool." }
                            button {
                                class: "btn secondary",
                                style: "background: #dfeaf0cd;color: black;
                                // font_size : 50px;
                                // padding: 5px 12px; width: 100%",
                                onclick: move |_| mutate_app(app, |state| state.reset_stats()),
                                "R E S E T  ___    P R O G R E S S"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_filter_option(
    app: Signal<Result<AppState, String>>,
    filters: FilterState,
    section_key: String,
    available_ids: BTreeSet<i64>,
    option: GrammarConstant,
) -> Element {
    let active = filters.is_selected(&section_key, option.id);
    let enabled = available_ids.contains(&option.id);
    let key = section_key;
    let id = option.id;

    rsx! {
        label { class: if enabled { if active { "dropdown-option active" } else { "dropdown-option" } } else { "dropdown-option disabled" },
            input {
                class: "dropdown-checkbox",
                r#type: "checkbox",
                checked: active,
                disabled: !enabled,
                onchange: move |_| mutate_app(app, |state| state.toggle_filter(&key, id))
            }
            span { "{option.display_label}" }
        }
    }
}

fn render_stat_card(label: &str, value: String) -> Element {
    rsx! {
        div { class: "stat-card",
            div { class: "eyebrow", "{label}" }
            p { class: "stat-value", "{value}" }
        }
    }
}

fn render_question_card(app: Signal<Result<AppState, String>>, question: Question) -> Element {
    let answered = question.is_answered();
    let uses_text_entry = question.uses_text_entry();
    let mode_label = question.mode.label().to_string();
    let prompt = question.prompt.clone();
    let clue = question.clue.clone();
    let selected_option_id = question.selected_option_id.clone();
    let correct_option_ids = question.correct_option_ids.clone();
    let options = question.options.clone();
    let text_entry = question.text_entry.clone();
    let submitted_text = question.submitted_text.clone();
    let accepted_text_answers = question.accepted_text_answers.clone();
    let text_is_correct = submitted_text
        .as_deref()
        .is_some_and(|value| quiz::is_correct_text_answer(&question, value));
    let accepted_answer_text = accepted_text_answers.join(" / ");
    let can_submit_text = !answered && !text_entry.trim().is_empty();

    rsx! {
        div { class: "question-card",
            div { class: "question-header",
                div { class: "eyebrow", "{mode_label}" }
                h2 { class: "question-prompt", "{prompt}" }
                if let Some(clue) = clue {
                    p { class: "question-clue", "{clue}" }
                }
            }

            if uses_text_entry {
                div { class: "text-answer-panel",
                    input {
                        class: "text-answer-input",
                        r#type: "text",
                        value: text_entry,
                        disabled: answered,
                        placeholder: "Type the requested Greek form",
                        oninput: move |event| mutate_app(app, |state| state.update_text_answer(event.value())),
                        onkeydown: move |event| {
                            if event.key() == dioxus::prelude::Key::Enter && can_submit_text {
                                mutate_app(app, |state| state.submit_text_answer());
                            }
                        },
                    }

                    button {
                        class: "action-button",
                        disabled: !can_submit_text,
                        onclick: move |_| mutate_app(app, |state| state.submit_text_answer()),
                        "Check answer"
                    }

                    if let Some(submitted_text) = submitted_text {
                        div { class: if text_is_correct { "answer-feedback correct" } else { "answer-feedback wrong" },
                            if text_is_correct {
                                "Correct: "
                                strong { "{submitted_text}" }
                            } else {
                                "Your answer: "
                                strong { "{submitted_text}" }
                                " | Expected: "
                                strong { "{accepted_answer_text}" }
                            }
                        }
                    }
                }
            } else {
                div { class: "answer-grid",
                    for option in options {
                        {
                            let option_id = option.id.clone();
                            let mut class_name = String::from("answer-button");
                            if answered {
                                if correct_option_ids.contains(&option_id) {
                                    class_name.push_str(" correct");
                                } else if selected_option_id.as_deref() == Some(option_id.as_str()) {
                                    class_name.push_str(" wrong");
                                }
                            }

                            rsx! {
                                button {
                                    class: "{class_name}",
                                    disabled: answered,
                                    onclick: move |_| {
                                        mutate_app(app, |state| state.answer(option_id.clone()));
                                    },
                                    "{option.label}"
                                }
                            }
                        }
                    }
                }
            }

            div { class: "question-actions",
                button {
                    class: "action-button",
                    onclick: move |_| {
                        mutate_app(app, |state| state.reload_questions());
                    },
                    "Rebuild candidate pool"
                }
                button {
                    class: "action-button next-button",
                    onclick: move |_| {
                        mutate_app(app, |state| state.next_question());
                    },
                    if answered { "Next question" } else { "Skip" }
                }
            }
        }
    }
}

fn render_stems_screen(stem_view: Option<StemView>) -> Element {
    if let Some(stem_view) = stem_view {
        let common_text = if stem_view.common_tags.is_empty() {
            None
        } else {
            Some(format!("Common: {}", stem_view.common_tags.join(" | ")))
        };
        let stemtype_text = if stem_view.stemtypes.is_empty() {
            None
        } else {
            Some(format!("Stemtype: {}", stem_view.stemtypes.join(", ")))
        };

        rsx! {
            div { class: "panel stem-card",
                div {
                    div { class: "eyebrow", "Word stems" }
                    h2 { class: "title", "{stem_view.lemma}" }
                    // p { class: "subtle", "Paradigm data is loaded from the shared exported snapshot." }
                }

                if let Some(stemtype_text) = stemtype_text {
                    div { class: "stemtype-banner", "{stemtype_text}" }
                }

                if let Some(common_text) = common_text {
                    div { class: "stem-common", "{common_text}" }
                }

                div { class: "stem-table-list",
                    for table in stem_view.tables {
                        {
                            let heading_text = table.heading.join(" | ");
                            rsx! {
                                div { class: "stem-table",
                                    if !table.heading.is_empty() {
                                        div { class: "stem-table-heading", "{heading_text}" }
                                    }
                                    table { class: "paradigm-table",
                                        thead {
                                            tr {
                                                th { "" }
                                                for column in table.columns {
                                                    th { "{column}" }
                                                }
                                            }
                                        }
                                        tbody {
                                            for row in table.rows {
                                                tr {
                                                    th { "{row.label}" }
                                                    for cell in row.cells {
                                                        td { "{cell}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {
            div { class: "panel empty",
                h2 { "No stem view available" }
                p { class: "subtle", "Pick a word or generate a question first so the app can load its stem profile." }
            }
        }
    }
}

fn mutate_app(mut app: Signal<Result<AppState, String>>, mutator: impl FnOnce(&mut AppState) -> Result<()>) {
    let mut guard = app.write();
    let result = match &mut *guard {
        Ok(state) => mutator(state).map_err(|error| error.to_string()),
        Err(error) => Err(error.clone()),
    };

    if let Err(error) = result {
        *guard = Err(error);
    }
}

#[derive(Clone, Debug)]
struct AppState {
    db: Database,
    constants: HashMap<i64, GrammarConstant>,
    filter_sections: Vec<FilterSection>,
    filters: FilterState,
    availability: HashMap<String, BTreeSet<i64>>,
    lemma_options: Vec<LemmaOption>,
    pool: Vec<QuizRow>,
    current_question: Option<Question>,
    mode: QuizMode,
    screen: Screen,
    filter_drawer_open: bool,
    stem_view: Option<StemView>,
    stats: SessionStats,
    lemma_search_query: String,
    preset_name_input: String,
    presets: std::collections::BTreeMap<String, FilterState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    Quiz,
    Stems,
}

impl AppState {
    fn load() -> Result<Self> {
        let db = Database::open_default()?;
        let constants = db.load_constants()?;
        let filter_sections = db.load_filter_sections()?;
        let mut state = Self {
            db,
            constants,
            filter_sections,
            filters: FilterState::default(),
            availability: HashMap::new(),
            lemma_options: Vec::new(),
            pool: Vec::new(),
            current_question: None,
            mode: QuizMode::ParseForm,
            screen: Screen::Quiz,
            filter_drawer_open: false,
            stem_view: None,
            stats: Self::load_stats_from_disk(),
            lemma_search_query: String::new(),
            preset_name_input: String::new(),
            presets: Self::load_presets_from_disk(),
        };
        state.reload_questions()?;
        Ok(state)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_stats_path() -> std::path::PathBuf {
        std::env::temp_dir().join("morph_app").join("stats.json")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_stats_from_disk() -> SessionStats {
        if let Ok(data) = std::fs::read_to_string(Self::get_stats_path()) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            SessionStats::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn load_stats_from_disk() -> SessionStats {
        Self::load_from_storage("morph_app.stats").unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_stats_to_disk(&self) {
        if let Ok(data) = serde_json::to_string(&self.stats) {
            let _ = std::fs::write(Self::get_stats_path(), data);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn save_stats_to_disk(&self) {
        Self::save_to_storage("morph_app.stats", &self.stats);
    }

    fn reset_stats(&mut self) -> Result<()> {
        self.stats = SessionStats::default();
        self.save_stats_to_disk();
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_presets_path() -> std::path::PathBuf {
        std::env::temp_dir().join("morph_app").join("presets.json")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_presets_from_disk() -> std::collections::BTreeMap<String, FilterState> {
        if let Ok(data) = std::fs::read_to_string(Self::get_presets_path()) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            std::collections::BTreeMap::new()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn load_presets_from_disk() -> std::collections::BTreeMap<String, FilterState> {
        Self::load_from_storage("morph_app.presets").unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_presets_to_disk(&self) {
        if let Ok(data) = serde_json::to_string(&self.presets) {
            let _ = std::fs::write(Self::get_presets_path(), data);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn save_presets_to_disk(&self) {
        Self::save_to_storage("morph_app.presets", &self.presets);
    }

    #[cfg(target_arch = "wasm32")]
    fn load_from_storage<T>(key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let storage = web_sys::window()?.local_storage().ok()??;
        let value = storage.get_item(key).ok()??;
        serde_json::from_str(&value).ok()
    }

    #[cfg(target_arch = "wasm32")]
    fn save_to_storage<T>(key: &str, value: &T)
    where
        T: serde::Serialize,
    {
        let Some(storage) = web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
        else {
            return;
        };

        let Ok(serialized) = serde_json::to_string(value) else {
            return;
        };

        let _ = storage.set_item(key, &serialized);
    }

    fn save_current_preset(&mut self) -> Result<()> {
        let name = self.preset_name_input.trim().to_string();
        if !name.is_empty() {
            self.presets.insert(name, self.filters.clone());
            self.save_presets_to_disk();
            self.preset_name_input.clear();
        }
        Ok(())
    }

    fn load_preset(&mut self, name: &str) -> Result<()> {
        if let Some(preset) = self.presets.get(name) {
            self.filters = preset.clone();
            self.reload_questions()?;
        }
        Ok(())
    }

    fn delete_preset(&mut self, name: &str) -> Result<()> {
        self.presets.remove(name);
        self.save_presets_to_disk();
        Ok(())
    }

    fn set_screen(&mut self, screen: Screen) -> Result<()> {
        self.screen = screen;
        self.refresh_stem_view()
    }

    fn set_filter_drawer(&mut self, open: bool) -> Result<()> {
        self.filter_drawer_open = open;
        Ok(())
    }

    fn set_mode(&mut self, mode: QuizMode) -> Result<()> {
        self.mode = mode;
        self.next_question()
    }

    fn toggle_lemma(&mut self, id: i64) -> Result<()> {
        self.filters.toggle_lemma(id);
        self.reload_questions()
    }

    fn toggle_filter(&mut self, category: &str, constant_id: i64) -> Result<()> {
        self.filters.toggle_constant(category, constant_id);
        self.reload_questions()
    }

    fn clear_filters(&mut self) -> Result<()> {
        self.filters = FilterState::default();
        self.reload_questions()
    }

    fn reload_questions(&mut self) -> Result<()> {
        self.sync_filter_state()?;
        self.pool = self.db.load_candidate_rows(&self.filters, 320)?;
        self.current_question = quiz::generate_question(&self.pool, &self.constants, self.mode);
        self.refresh_stem_view()?;
        Ok(())
    }

    fn next_question(&mut self) -> Result<()> {
        self.current_question = quiz::generate_question(&self.pool, &self.constants, self.mode);
        if self.current_question.is_none() {
            self.reload_questions()?;
        }
        self.refresh_stem_view()?;
        Ok(())
    }

    fn answer(&mut self, option_id: String) -> Result<()> {
        if let Some(question) = &mut self.current_question {
            if question.selected_option_id.is_none() {
                let was_correct = question.is_correct(&option_id);
                question.selected_option_id = Some(option_id);
                self.stats.register(was_correct);
                self.save_stats_to_disk();
            }
        }
        Ok(())
    }

    fn update_text_answer(&mut self, value: String) -> Result<()> {
        if let Some(question) = &mut self.current_question {
            if question.uses_text_entry() && question.submitted_text.is_none() {
                question.text_entry = value;
            }
        }
        Ok(())
    }

    fn submit_text_answer(&mut self) -> Result<()> {
        if let Some(question) = &mut self.current_question {
            if question.uses_text_entry() && question.submitted_text.is_none() {
                let submitted = question.text_entry.trim().to_string();
                if submitted.is_empty() {
                    return Ok(());
                }
                let was_correct = quiz::is_correct_text_answer(question, &submitted);
                question.submitted_text = Some(submitted);
                self.stats.register(was_correct);
                self.save_stats_to_disk();
            }
        }
        Ok(())
    }

    fn sync_filter_state(&mut self) -> Result<()> {
        for _ in 0..4 {
            let availability = self.db.load_filter_availability(&self.filters, &self.filter_sections)?;
            let lemma_options = self.db.load_lemma_options(&self.filters)?;
            let available_lemma_ids = lemma_options.iter().map(|lemma| lemma.id).collect::<BTreeSet<_>>();

            let mut changed = false;
            for section in &self.filter_sections {
                if let Some(selected) = self.filters.selected_constants.get_mut(&section.key) {
                    let allowed = availability.get(&section.key).cloned().unwrap_or_default();
                    let before = selected.len();
                    selected.retain(|value| allowed.contains(value));
                    if selected.len() != before {
                        changed = true;
                    }
                }
            }
            self.filters.selected_constants.retain(|_, values| !values.is_empty());

            let mut invalid_ids = Vec::new();
            for &lemma_id in &self.filters.selected_lemma_ids {
                if !available_lemma_ids.contains(&lemma_id) {
                    invalid_ids.push(lemma_id);
                }
            }
            if !invalid_ids.is_empty() {
                for id in invalid_ids {
                    self.filters.selected_lemma_ids.remove(&id);
                }
                changed = true;
            }

            if !changed {
                self.availability = availability;
                self.lemma_options = lemma_options;
                return Ok(());
            }
        }

        self.availability = self.db.load_filter_availability(&self.filters, &self.filter_sections)?;
        self.lemma_options = self.db.load_lemma_options(&self.filters)?;
        Ok(())
    }

    fn refresh_stem_view(&mut self) -> Result<()> {
        let active_lemma_id = self
            .filters
            .selected_lemma_ids
            .iter()
            .next()
            .copied()
            .or_else(|| self.current_question.as_ref().map(|question| question.lemma_id))
            .or_else(|| self.lemma_options.first().map(|lemma| lemma.id));

        if let Some(lemma_id) = active_lemma_id {
            self.stem_view = self.db.load_stem_view(lemma_id)?;
        } else {
            self.stem_view = None;
        }

        Ok(())
    }
}