use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use crate::app::{App, ChatPane};
use crate::store::Message;
use crate::theme::Theme;
use crate::ui::{self, field_paragraph, panel};
use crate::util::{hard_wrap, split_think, truncate, wrap};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 6 || area.width < 10 {
        return;
    }
    let area = crate::aurora::body(area);
    let theme = app.theme.clone();
    let content = if app.show_chats && (area.width >= 80 || app.chat_pane == ChatPane::Sidebar) {
        if area.width < 70 && app.chat_pane == ChatPane::Sidebar {
            draw_sidebar(frame, app, &theme, area);
            return;
        }
        let [side, _gutter, content] = Layout::horizontal([
            Constraint::Length(if area.width >= 100 { 29 } else { 24 }),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .areas(area);
        draw_sidebar(frame, app, &theme, side);
        content
    } else {
        area
    };
    let text_rows = composer_rows(
        &app.composer.value,
        content.width.saturating_sub(2) as usize,
    );
    let roomy = area.height >= 18;
    let composer_min = if roomy { 5 } else { 3 };
    let composer_h = (text_rows as u16 + if roomy { 4 } else { 2 })
        .max(composer_min)
        .min(content.height.saturating_sub(3))
        .max(3);
    let gap_h = u16::from(content.height >= 15);
    let desired_transcript = transcript_lines(app, &theme, content.width as usize).len() as u16 + 2;
    let [transcript, _gap, composer_area, _space] = Layout::vertical([
        Constraint::Length(
            desired_transcript
                .max(4)
                .min(content.height.saturating_sub(composer_h + gap_h)),
        ),
        Constraint::Length(gap_h),
        Constraint::Length(composer_h),
        Constraint::Min(0),
    ])
    .areas(content);
    draw_transcript(frame, app, &theme, transcript);
    draw_composer(frame, app, &theme, composer_area);
    if app.model_palette_open() {
        draw_model_palette(frame, app, &theme, transcript);
    } else if app.slash_palette_open() {
        draw_slash_palette(frame, app, &theme, transcript);
    }
}

fn draw_sidebar(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 0);
    let focused = app.chat_pane == ChatPane::Sidebar;
    let border = if focused { theme.accent } else { theme.border };
    let area = crate::aurora::sidebar(frame, area, Style::default().fg(border));
    let spaced = area.height >= 18;
    let list_h = (app.chats.len() as u16 * if spaced { 2 } else { 1 })
        .max(1)
        .min(area.height.saturating_sub(5));
    let [heading, list_area, _gap, note, _space] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(list_h),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new("Conversations").style(Style::default().fg(theme.accent)),
        heading,
    );
    let items: Vec<ListItem> = app
        .chats
        .iter()
        .enumerate()
        .map(|(index, chat)| {
            let marker = if app.streaming_chat.as_deref() == Some(chat.id.as_str()) {
                "● "
            } else if index == app.chat_index {
                "▸ "
            } else {
                "  "
            };
            let title = format!("{marker}{}", chat.title);
            let title = Line::from(truncate(&title, area.width.saturating_sub(1) as usize));
            ListItem::new(if spaced {
                vec![title, Line::from("")]
            } else {
                vec![title]
            })
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.chat_index));
    }
    frame.render_stateful_widget(
        List::new(items).highlight_style(theme.selected()),
        list_area,
        &mut state,
    );
    app.navigation.rows(
        list_area,
        0,
        state.offset(),
        if spaced { 2 } else { 1 },
        app.chats.len().saturating_sub(state.offset()),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("(Ctrl+N)", Style::default().fg(theme.accent)),
            Span::styled(" new conversation", Style::default().fg(theme.muted)),
        ])),
        note,
    );
}

fn draw_transcript(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 1);
    let focused = app.chat_pane == ChatPane::Transcript;
    let title = app
        .chat()
        .map(|c| truncate(&c.title, 40))
        .unwrap_or_else(|| "Chat".into());
    let model = app
        .chat()
        .map(|c| c.model.as_str())
        .filter(|m| !m.is_empty())
        .unwrap_or("no model");
    let [heading, messages] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);
    let header_inner = heading;
    if focused {
        frame.render_widget(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::LEFT)
                .border_style(Style::default().fg(theme.accent)),
            area,
        );
    }
    let [title_area, model_area] = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(model.chars().count().min(24) as u16 + 1),
    ])
    .areas(header_inner);
    frame.render_widget(
        Paragraph::new(Span::styled(title, Style::default().fg(theme.fg))),
        title_area,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(model, Style::default().fg(theme.muted)))
            .alignment(ratatui::layout::Alignment::Right),
        model_area,
    );
    let inner = messages;
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let lines = transcript_lines(app, theme, width);
    let view = inner.height as usize;
    app.transcript_view = view.max(1);
    app.transcript_len = lines.len();
    if app.stick_bottom {
        app.scroll = lines.len().saturating_sub(view);
    }
    let max_scroll = lines.len().saturating_sub(view);
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }
    let scroll = app.scroll;
    let total = lines.len();
    let visible: Vec<Line> = lines.into_iter().skip(scroll).take(view).collect();
    frame.render_widget(Paragraph::new(visible), inner);
    if total > view {
        let mut state = ScrollbarState::new(total.saturating_sub(view))
            .position(scroll)
            .viewport_content_length(view);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::default().fg(theme.muted)),
            area,
            &mut state,
        );
    }
}

fn transcript_lines(app: &App, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if app.connected == Some(false) {
        lines.push(Line::from(Span::styled(
            format!(
                "Ollama is not running at {}. Press s or Start.",
                app.config.host
            ),
            Style::default().fg(theme.red),
        )));
        lines.push(Line::from(""));
    }
    let Some(chat) = app.chat() else {
        return lines;
    };
    if !chat.system.trim().is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "system  {}",
                truncate(&chat.system.replace('\n', " "), width.saturating_sub(8))
            ),
            Style::default()
                .fg(theme.magenta)
                .add_modifier(Modifier::ITALIC),
        )));
        lines.push(Line::from(""));
    }
    if chat.messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "Ask something. Enter sends. Up or the wheel scrolls the reply. /help lists commands.",
            Style::default().fg(theme.muted),
        )));
        return lines;
    }
    for (index, message) in chat.messages.iter().enumerate() {
        let selected = app.chat_pane == ChatPane::Transcript && index == app.transcript_cursor;
        lines.extend(message_lines(
            message,
            theme,
            width,
            selected,
            app.config.show_token_stats,
            app.config.assistant_name.as_str(),
        ));
        lines.push(Line::from(""));
    }
    lines
}

fn message_lines(
    message: &Message,
    theme: &Theme,
    width: usize,
    selected: bool,
    show_stats: bool,
    assistant_name: &str,
) -> Vec<Line<'static>> {
    if message.role == "user" {
        return user_message_lines(message, theme, width, selected);
    }
    let mut lines = Vec::new();
    let name = assistant_name.trim();
    let name = if name.is_empty() { "Luma" } else { name };
    let marker = if selected { "▌ " } else { "" };
    lines.push(Line::from(Span::styled(
        format!("{marker}{name} · local response"),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));

    let (tagged, body) = split_think(&message.content);
    let mut thinking = message.thinking.clone();
    if !tagged.trim().is_empty() {
        if !thinking.is_empty() {
            thinking.push('\n');
        }
        thinking.push_str(&tagged);
    }
    let thinking = thinking.trim().to_string();
    if !thinking.is_empty() {
        let label = if message.thinking_open {
            "▾ thinking"
        } else {
            "▸ thinking"
        };
        lines.push(Line::from(Span::styled(
            format!("  {label}"),
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
        )));
        if message.thinking_open {
            for line in wrap(&thinking, width.saturating_sub(2)) {
                lines.push(Line::from(Span::styled(
                    format!("  {line}"),
                    Style::default()
                        .fg(theme.fg_dim)
                        .add_modifier(Modifier::ITALIC),
                )));
            }
        }
    }

    let mut in_code = false;
    let mut code = String::new();
    let flush_code = |code: &mut String, lines: &mut Vec<Line<'static>>| {
        if code.is_empty() {
            return;
        }
        for line in hard_wrap(code, width.saturating_sub(2)) {
            lines.push(Line::from(Span::styled(
                format!(" {line}"),
                Style::default().fg(theme.cyan).bg(theme.bg_raised),
            )));
        }
        code.clear();
    };
    for line in body.split('\n') {
        if line.trim_start().starts_with("```") {
            if in_code {
                flush_code(&mut code, &mut lines);
                in_code = false;
            } else {
                in_code = true;
            }
            continue;
        }
        if in_code {
            if !code.is_empty() {
                code.push('\n');
            }
            code.push_str(line);
        } else if line.is_empty() {
            lines.push(Line::from(""));
        } else {
            for wrapped in wrap(line, width.saturating_sub(2)) {
                lines.push(Line::from(Span::styled(
                    wrapped,
                    Style::default().fg(theme.fg),
                )));
            }
        }
    }
    if in_code {
        flush_code(&mut code, &mut lines);
    }
    if show_stats {
        if let Some(stats) = &message.stats {
            lines.push(Line::from(Span::styled(
                format!("  {:.1} tok/s · {} tok", stats.tokens_per_sec, stats.tokens),
                Style::default().fg(theme.muted),
            )));
        }
    }
    lines
}

fn user_message_lines(
    message: &Message,
    theme: &Theme,
    width: usize,
    selected: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        if selected { "▌ You" } else { "You" },
        Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD),
    ))];
    for paragraph in message.content.trim().split('\n') {
        for line in wrap(paragraph, width.max(1)) {
            lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(theme.fg),
            )));
        }
    }
    lines
}

fn composer_rows(value: &str, width: usize) -> usize {
    let width = width.max(1);
    let mut rows = 0usize;
    let parts: Vec<&str> = value.split('\n').collect();
    for (index, part) in parts.iter().enumerate() {
        let cols = crate::util::width_of(part);
        let cursor = usize::from(index + 1 == parts.len());
        rows += (cols + cursor).div_ceil(width).max(1);
    }
    rows.clamp(1, 8)
}

fn draw_model_palette(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let matches = app.model_matches();
    let shown = 8usize;
    let pick = if matches.is_empty() {
        0
    } else {
        app.slash_pick.min(matches.len() - 1)
    };
    let start = pick
        .saturating_add(1)
        .saturating_sub(shown)
        .min(matches.len().saturating_sub(shown));
    let window: Vec<_> = matches.iter().skip(start).take(shown).collect();
    let rows = window.len().max(1) as u16;
    let height = rows.saturating_add(2).min(area.height);
    let width = 52.min(area.width);
    let rect = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height),
        width,
        height,
    );
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = panel("Models", true, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 {
        return;
    }
    let mut lines = Vec::new();
    if window.is_empty() {
        lines.push(Line::from(Span::styled(
            "No installed models",
            Style::default().fg(theme.muted),
        )));
    }
    for (offset, name) in window.iter().enumerate() {
        let selected = start + offset == pick;
        let style = if selected {
            theme.selected()
        } else {
            Style::default().fg(theme.fg).bg(theme.bg)
        };
        lines.push(Line::from(Span::styled(
            truncate(name, inner.width as usize),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    app.navigation.rows(inner, 97, start, 1, window.len());
}

fn draw_slash_palette(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let matches = app.slash_matches();
    let shown = 8usize;
    let pick = if matches.is_empty() {
        0
    } else {
        app.slash_pick.min(matches.len() - 1)
    };
    let start = pick
        .saturating_add(1)
        .saturating_sub(shown)
        .min(matches.len().saturating_sub(shown));
    let window: Vec<_> = if matches.is_empty() {
        Vec::new()
    } else {
        matches.iter().skip(start).take(shown).collect()
    };
    let rows = window.len().max(1) as u16;
    let height = rows.saturating_add(2).min(area.height);
    let width = 52.min(area.width);
    let rect = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height),
        width,
        height,
    );
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = panel("Commands", true, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 {
        return;
    }
    let mut lines = Vec::new();
    if window.is_empty() {
        lines.push(Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(theme.muted),
        )));
    }
    for (offset, command) in window.iter().enumerate() {
        let selected = start + offset == pick;
        let style = if selected {
            theme.selected()
        } else {
            Style::default().fg(theme.fg).bg(theme.bg)
        };
        let name = format!("/{:<8}", command.name);
        let summary = truncate(command.summary, inner.width.saturating_sub(12) as usize);
        lines.push(Line::from(vec![
            Span::styled(
                name,
                if selected {
                    style
                } else {
                    Style::default().fg(theme.accent).bg(theme.bg)
                },
            ),
            Span::styled(summary, style),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    app.navigation.rows(inner, 96, start, 1, window.len());
}

fn draw_composer(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation.pane(area, 2);
    let focused = app.chat_pane == ChatPane::Composer
        && !app.model_picker
        && !app.renaming
        && !app.editing_system;
    let roomy = area.height >= 5;
    let block = ratatui::widgets::Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.border }))
        .padding(ratatui::widgets::Padding::horizontal(1))
        .style(theme.base());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = if roomy {
        let [heading, field] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
        let model = app
            .chat()
            .map(|chat| chat.model.as_str())
            .unwrap_or("no model");
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    if app.streaming {
                        "Generating"
                    } else {
                        "Message"
                    },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(format!(" · {model}"), Style::default().fg(theme.muted)),
            ])),
            heading,
        );
        field
    } else {
        inner
    };
    let [prompt, inner] =
        Layout::horizontal([Constraint::Length(2), Constraint::Min(0)]).areas(inner);
    frame.render_widget(
        Paragraph::new("› ").style(Style::default().fg(theme.cyan)),
        prompt,
    );
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let style = Style::default().fg(theme.fg).bg(theme.bg);
    let paragraph = if app.composer.value.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::styled("Ask a follow-up…", Style::default().fg(theme.muted)),
            Span::styled(
                if focused { " ▌" } else { "" },
                Style::default().fg(theme.accent),
            ),
        ]))
    } else if focused {
        field_paragraph(
            &app.composer,
            inner.width as usize,
            inner.height as usize,
            style,
        )
    } else {
        Paragraph::new(app.composer.value.clone())
    };
    frame.render_widget(paragraph, inner);
}

pub fn draw_model_picker(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rect = ui::centered(area, 56, 16);
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = panel("Models", true, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height < 3 {
        return;
    }
    let [filter_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let style = Style::default().fg(theme.fg).bg(theme.bg);
    frame.render_widget(
        field_paragraph(&app.model_filter, filter_area.width as usize, 1, style),
        filter_area,
    );
    let choices = app.model_choices();
    let items: Vec<ListItem> = if choices.is_empty() {
        vec![ListItem::new("No installed models")]
    } else {
        choices
            .into_iter()
            .map(|name| ListItem::new(name))
            .collect()
    };
    let mut state = ListState::default();
    if !app.model_choices().is_empty() {
        state.select(Some(app.model_pick.min(items.len().saturating_sub(1))));
    }
    frame.render_stateful_widget(
        List::new(items).highlight_style(theme.selected()),
        list_area,
        &mut state,
    );
    app.navigation.rows(
        list_area,
        98,
        state.offset(),
        1,
        app.model_choices().len().saturating_sub(state.offset()),
    );
}

pub fn draw_rename(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rect = ui::centered(area, 50, 5);
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = panel("Rename", true, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width > 0 && inner.height > 0 {
        let style = Style::default().fg(theme.fg).bg(theme.bg);
        frame.render_widget(
            field_paragraph(&app.rename_field, inner.width as usize, 1, style),
            inner,
        );
    }
}

pub fn draw_system(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rect = ui::centered(area, 64, 12);
    frame.render_widget(ratatui::widgets::Clear, rect);
    let block = panel("System prompt  ·  ^S save  esc cancel", true, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width > 0 && inner.height > 0 {
        let style = Style::default().fg(theme.fg).bg(theme.bg);
        frame.render_widget(
            field_paragraph(
                &app.system_field,
                inner.width as usize,
                inner.height as usize,
                style,
            ),
            inner,
        );
    }
}
