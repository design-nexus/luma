use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, MarketPane};
use crate::theme::Theme;
use crate::ui::{field_paragraph, panel};
use crate::util::{human_bytes, truncate};

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    if area.height < 6 || area.width < 10 {
        return;
    }
    let theme = app.theme.clone();
    if area.width < 80 {
        match app.hf_pane {
            MarketPane::Search => draw_search(frame, app, &theme, area),
            MarketPane::Results => draw_results(frame, app, &theme, area),
            MarketPane::Files => draw_files(frame, app, &theme, area),
        }
        return;
    }

    let [search_area, body] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(area);
    draw_search(frame, app, &theme, search_area);
    if app.hf_pane == MarketPane::Files {
        let [results, files] =
            Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)])
                .areas(body);
        draw_results(frame, app, &theme, results);
        draw_files(frame, app, &theme, files);
    } else {
        draw_results(frame, app, &theme, body);
    }
}

fn draw_search(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 0);
    let focused = app.hf_pane == MarketPane::Search;
    let title = format!("Search GGUF  ·  sort {}", app.hf_sort.label());
    let block = panel(&title, focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let style = Style::default().fg(theme.fg).bg(theme.bg);
    let paragraph = if focused {
        field_paragraph(&app.hf_query, inner.width as usize, 1, style)
    } else if app.hf_query.value.is_empty() {
        Paragraph::new(Span::styled(
            "Popular GGUF models. / to search. Ctrl+S changes sort.",
            Style::default().fg(theme.muted),
        ))
    } else {
        Paragraph::new(app.hf_query.value.clone())
    };
    frame.render_widget(paragraph, inner);
}

fn draw_results(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 1);
    let focused = app.hf_pane == MarketPane::Results;
    let title = if app.hf_loading && app.hf_pane != MarketPane::Files {
        "Results  ·  loading"
    } else {
        "Results"
    };
    let block = panel(title, focused, theme);
    let items: Vec<ListItem> = if app.hf_results.is_empty() {
        vec![ListItem::new(if app.hf_loading {
            "Searching…"
        } else {
            "No results"
        })]
    } else {
        app.hf_results
            .iter()
            .map(|model| {
                let downloads = compact(model.downloads);
                let likes = compact(model.likes);
                ListItem::new(format!(
                    "{}  ↓{}  ♥{}  {}",
                    truncate(&model.id, 36),
                    downloads,
                    likes,
                    model.last_modified
                ))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !app.hf_results.is_empty() {
        state.select(Some(app.hf_index));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(theme.selected()),
        area,
        &mut state,
    );
    app.navigation.rows(
        area.inner(ratatui::layout::Margin::new(1, 1)),
        1,
        state.offset(),
        1,
        app.hf_results.len().saturating_sub(state.offset()),
    );
}

fn draw_files(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 2);
    let block = panel(
        "Files · (Enter) pull",
        app.hf_pane == MarketPane::Files,
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [header, list] =
        Layout::vertical([Constraint::Length(2.min(inner.height)), Constraint::Min(0)])
            .areas(inner);
    frame.render_widget(
        Paragraph::new(format!(
            "{}\nSelect a GGUF file to pull through Ollama",
            app.hf_repo
        ))
        .style(Style::default().fg(theme.muted)),
        header,
    );
    let items = app
        .hf_files
        .iter()
        .map(|file| {
            ListItem::new(vec![
                Line::from(format!("{} {}", file.quant, human_bytes(file.size))),
                Line::from(file.path.clone()),
            ])
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(app.hf_file_index));
    frame.render_stateful_widget(
        List::new(items).highlight_style(theme.selected()),
        list,
        &mut state,
    );
    app.navigation.rows(
        list,
        2,
        state.offset(),
        2,
        app.hf_files.len().saturating_sub(state.offset()),
    );
}

fn compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
