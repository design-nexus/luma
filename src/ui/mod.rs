mod chat;
mod marketplace;
mod models;
mod settings;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::app::{App, Screen};
use crate::field::Field;
use crate::theme::Theme;
use crate::util::{human_bytes, truncate};

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.navigation.begin(frame.area());
    let area = frame.area();
    if area.width < 30 || area.height < 14 {
        frame.render_widget(
            Paragraph::new(
                "luma · needs 30×14
Resize terminal; Ctrl+C quits",
            ),
            area,
        );
        return;
    }
    let theme = app.theme.clone();
    frame.render_widget(Block::default().style(theme.base()), area);

    app.start_hit = None;
    let chrome = chrome(&theme);
    let area = chrome.shell(frame, area);
    let footer_h = if area.height >= 26 { 4 } else { 3 };
    let [status, tabs, body, footer] = Layout::vertical([
        Constraint::Length(crate::aurora::title_height(area.height)),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(footer_h),
    ])
    .areas(area);

    draw_status(frame, app, &theme, status);
    draw_tabs(frame, app, &theme, tabs);
    match app.screen {
        Screen::Chat => chat::draw(frame, app, body),
        Screen::Models => models::draw(frame, app, body),
        Screen::Marketplace => marketplace::draw(frame, app, body),
        Screen::Settings => settings::draw(frame, app, body),
    }
    let footer = chrome.footer(frame, footer);
    draw_footer(frame, app, &theme, footer);

    if app.model_picker {
        chat::draw_model_picker(frame, app, area);
    }
    if app.renaming {
        chat::draw_rename(frame, app, area);
    }
    if app.editing_system {
        chat::draw_system(frame, app, area);
    }
    if let Some(confirm) = app.confirm.as_ref() {
        draw_confirm(frame, &theme, area, confirm);
        app.navigation
            .dialog_buttons(frame, centered(area, 56, 7), app.navigation_skin());
    }
    if app.help {
        draw_help(frame, &theme, area);
    }
    app.commands.draw(
        frame,
        &app.command_actions(),
        app.theme.bg,
        app.theme.fg,
        app.theme.accent,
        app.theme.bg_raised,
    );
    app.draw_navigation(frame);
}

fn chrome(theme: &Theme) -> crate::aurora::Chrome {
    crate::aurora::Chrome {
        base: theme.base(),
        muted: Style::default().fg(theme.muted),
        accent: Style::default().fg(theme.accent),
        data: Style::default().fg(theme.cyan),
        border: Style::default().fg(theme.border),
        deep: theme.status(),
    }
}

fn draw_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let (connection, color) = connection(app, theme);
    let status = if app.status_is_error && !app.status.is_empty() {
        truncate(&app.status, 24)
    } else {
        format!("● Ollama {connection}")
    };
    chrome(theme).title(
        frame,
        area,
        "luma",
        "Local intelligence",
        &status,
        Style::default().fg(color),
    );
}

pub fn connection(app: &App, theme: &Theme) -> (String, ratatui::style::Color) {
    match app.connected {
        Some(true) => ("connected".into(), theme.green),
        Some(false) => ("offline".into(), theme.red),
        None => ("connecting".into(), theme.yellow),
    }
}

fn draw_tabs(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    app.navigation_bar(frame, area);
}

fn draw_footer(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let hints = app.footer_hints();
    let hint = hints.as_slice();
    if app.starting {
        let [gauge_area, hint_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
        let ratio = (app.pulse % 8) as f64 / 7.0;
        let gauge = Gauge::default()
            .ratio(ratio.clamp(0.05, 1.0))
            .label("Starting Ollama…")
            .gauge_style(Style::default().fg(theme.accent).bg(theme.bg_raised));
        frame.render_widget(gauge, gauge_area);
        draw_hint(frame, app, theme, hint_area, hint);
    } else if let Some(pull) = app.pull.as_ref() {
        let [gauge_area, hint_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
        let ratio = if pull.total == 0 {
            0.0
        } else {
            pull.completed as f64 / pull.total as f64
        };
        let label = if pull.total == 0 {
            format!("{}  {}", truncate(&pull.label, 24), pull.status)
        } else {
            format!(
                "{}  {}  {} / {}",
                truncate(&pull.label, 20),
                pull.status,
                human_bytes(pull.completed),
                human_bytes(pull.total)
            )
        };
        let gauge = Gauge::default()
            .ratio(ratio.clamp(0.0, 1.0))
            .label(label)
            .gauge_style(Style::default().fg(theme.accent).bg(theme.bg_raised));
        frame.render_widget(gauge, gauge_area);
        draw_hint(frame, app, theme, hint_area, hint);
    } else {
        draw_hint(frame, app, theme, area, hint);
    }
}

fn draw_hint(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect, hint: &[(&str, &str)]) {
    let mut spans = Vec::new();
    if app.can_start() {
        let button = " Start ";
        app.start_hit = Some((area.x, area.y, button.len() as u16, 1));
        app.navigation.hit(
            Rect::new(area.x, area.y, button.len() as u16, 1),
            crate::navigation::Intent::Command("Start Ollama"),
        );
        spans.push(Span::styled(
            button,
            theme.accent_style().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    spans.extend(
        crate::navigation::shortcuts(
            hint,
            area.width
                .saturating_sub(if app.can_start() { 8 } else { 0 }),
            Style::default().fg(theme.accent),
            Style::default().fg(theme.muted),
        )
        .spans,
    );
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme.status()),
        Rect::new(area.x, area.y, area.width, 1),
    );
    if app.screen == Screen::Marketplace && area.height > 1 {
        frame.render_widget(
            Paragraph::new("Gated models need HF_TOKEN").style(theme.status().fg(theme.muted)),
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
    }
}

fn draw_help(frame: &mut Frame, theme: &Theme, area: Rect) {
    let lines = [
        "Chat",
        "  enter send   alt+enter or ^J newline   ctrl+x stops a reply",
        "  wheel, pgup/pgdn, or up/down from the prompt scroll the reply",
        "  ^N new chat   ^M models   ^P system prompt",
        "",
        "Commands, typed in the prompt",
        "  /start  /stop  /restart     Ollama service",
        "  /model [name]               pick the chat model",
        "  /new  /clear  /cancel       new chat, wipe it, stop a reply",
        "  /system [prompt]  /theme [name]  /pull <name>",
        "  /model then a space filters installed models",
        "  ^B or /chats shows or hides the chat list",
        "  type / to filter commands, tab completes, enter runs",
        "  s or the Start button starts Ollama when it is offline",
        "  /help  /quit  /q",
        "",
        "Settings can start Ollama with the app, and stop it on quit.",
        "F2 Navigate  F6/shift+F6 panes  tab/shift+tab screens",
    ];
    overlay(frame, theme, area, " Help ", &lines, 68, 24);
}

fn draw_confirm(frame: &mut Frame, theme: &Theme, area: Rect, confirm: &crate::app::Confirm) {
    let question = match confirm {
        crate::app::Confirm::DeleteChat { title, .. } => format!("Delete chat \"{title}\"?"),
        crate::app::Confirm::DeleteModel(name) => format!("Delete model {name}?"),
    };
    let text = vec![
        question,
        String::new(),
        "y delete    n or esc cancel".into(),
    ];
    overlay_owned(frame, theme, area, " Confirm ", &text, 56, 7);
}

pub fn overlay(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    title: &str,
    lines: &[&str],
    width: u16,
    height: u16,
) {
    let owned: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    overlay_owned(frame, theme, area, title, &owned, width, height);
}

pub fn overlay_owned(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    title: &str,
    lines: &[String],
    width: u16,
    height: u16,
) {
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title(format!(" {title} "))
        .style(theme.base());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let text: Vec<Line> = lines
        .iter()
        .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(theme.fg))))
        .collect();
    frame.render_widget(Paragraph::new(text), inner);
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width).max(1);
    let height = height.min(area.height).max(1);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

pub fn panel(title: &str, focused: bool, theme: &Theme) -> Block<'static> {
    let border = if focused { theme.accent } else { theme.border };
    Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .title(format!(" {title} "))
        .style(theme.base())
}

pub fn field_paragraph(
    field: &Field,
    width: usize,
    max_lines: usize,
    style: Style,
) -> Paragraph<'static> {
    Paragraph::new(field_lines(field, width, max_lines, style))
}

pub fn field_lines(
    field: &Field,
    width: usize,
    max_lines: usize,
    style: Style,
) -> Vec<Line<'static>> {
    let width = width.max(1);
    let max_lines = max_lines.max(1);
    let mut rows: Vec<Vec<(char, bool)>> = vec![Vec::new()];
    let mut col = 0usize;
    let mut cursor_at = 0usize;
    for (byte, ch) in field.value.char_indices() {
        let cursor = byte == field.cursor;
        if ch == '\n' {
            if cursor {
                rows.last_mut().unwrap().push((' ', true));
                cursor_at = rows.len() - 1;
            }
            rows.push(Vec::new());
            col = 0;
            continue;
        }
        let cw = UnicodeWidthChar::width(ch).unwrap_or(1).max(1);
        if col + cw > width && !rows.last().unwrap().is_empty() {
            rows.push(Vec::new());
            col = 0;
        }
        if cursor {
            cursor_at = rows.len() - 1;
        }
        rows.last_mut().unwrap().push((ch, cursor));
        col += cw;
    }
    if field.cursor >= field.value.len() {
        if col >= width && !rows.last().unwrap().is_empty() {
            rows.push(Vec::new());
        }
        rows.last_mut().unwrap().push((' ', true));
        cursor_at = rows.len() - 1;
    }
    let start = cursor_at.saturating_add(1).saturating_sub(max_lines);
    let cursor_style = Style::default()
        .bg(style.fg.unwrap_or(ratatui::style::Color::White))
        .fg(style.bg.unwrap_or(ratatui::style::Color::Black));
    rows.into_iter()
        .skip(start)
        .take(max_lines)
        .map(|row| {
            let mut spans = Vec::new();
            for (ch, cursor) in row {
                let text = ch.to_string();
                spans.push(Span::styled(
                    text,
                    if cursor { cursor_style } else { style },
                ));
            }
            if spans.is_empty() {
                spans.push(Span::styled(" ", style));
            }
            Line::from(spans)
        })
        .collect()
}

pub fn swatch_line(theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for color in [
        theme.bg,
        theme.fg,
        theme.accent,
        theme.red,
        theme.green,
        theme.blue,
    ] {
        spans.push(Span::styled("  ", Style::default().bg(color)));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tokio::sync::mpsc::unbounded_channel;

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn four_screens_render() {
        let (tx, _rx) = unbounded_channel();
        let mut config = Config::default();
        config.theme = "dracula".into();
        let mut app = App::new(
            config,
            std::path::PathBuf::from("/tmp/luma-test-config.toml"),
            Vec::new(),
            None,
            String::new(),
            None,
            tx,
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let chat = buffer_text(&terminal);
        assert!(
            (0..8).any(|y| (0..100).any(|x| matches!(
                terminal.backend().buffer()[(x, y)].symbol(),
                "█" | "▀" | "▄"
            ))),
            "pixel title must be visible"
        );
        assert!(chat.contains("Chat"), "{chat}");
        assert!(chat.contains("Conversations"), "{chat}");

        app.screen = Screen::Models;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let models = buffer_text(&terminal);
        assert!(models.contains("Installed"), "{models}");

        app.screen = Screen::Marketplace;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let market = buffer_text(&terminal);
        assert!(market.contains("Search"), "{market}");

        app.screen = Screen::Settings;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let settings = buffer_text(&terminal);
        assert!(settings.contains("Theme"), "{settings}");
        assert!(settings.contains("Dracula"), "{settings}");
        assert!(settings.contains("Autostart"), "{settings}");
        assert!(!settings.contains("Start Ollama"), "{settings}");
        assert!(chat.contains("connecting"), "{chat}");
        assert!(!chat.contains("Dracula"), "{chat}");

        app.screen = Screen::Chat;
        app.connected = Some(false);
        app.config.host = "http://127.0.0.1:11434".into();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let offline = buffer_text(&terminal);
        assert!(offline.contains("Ollama is not running"), "{offline}");
        assert!(offline.contains("Start"), "{offline}");
    }
}

#[cfg(test)]
mod aurora_audit {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn commands() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)
    }
    fn fixture(path: std::path::PathBuf) -> App {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let mut chat = crate::store::Conversation::new("qwen3:8b".into());
        chat.title = "Rust notes".into();
        chat.messages.push(crate::store::Message::user(
            "Why does Rust say this value was moved? 你好 🦀".into(),
        ));
        let mut response = crate::store::Message::assistant();
        response.content = "Rust transfers ownership when you assign a value to another variable. The original binding can no longer use it.\n\n```rust\nlet first = String::from(\"hello\");\nlet second = first;\n```\n\nBorrow with &first to read it, or clone it for a separate owned copy.".into();
        chat.messages.push(response);
        let mut app = App::new(
            crate::config::Config::default(),
            path,
            vec![chat],
            None,
            String::new(),
            None,
            tx,
        );
        app.connected = Some(true);
        app.chat_pane = crate::app::ChatPane::Composer;
        app
    }

    #[tokio::test]
    async fn common_navigation_contract_preserves_focus_and_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        app.focus_pane(0);
        if app.navigation_editing() {
            let _ = app.on_key(key(KeyCode::Esc));
        }
        let screen = app.navigation_screen();
        let pane = app.focused_pane();
        let _ = app.on_key(key(KeyCode::Tab));
        assert_eq!(
            app.navigation_screen(),
            crate::navigation::step(
                screen,
                app.navigation_entries()
                    .iter()
                    .filter(|e| matches!(e.target, crate::navigation::Intent::Screen(_)))
                    .count(),
                1
            )
        );
        if app.navigation_editing() {
            let _ = app.on_key(key(KeyCode::Esc));
        }
        let _ = app.on_key(key(KeyCode::BackTab));
        assert_eq!(app.navigation_screen(), screen);
        let _ = app.on_key(key(KeyCode::F(6)));
        assert_eq!(
            app.focused_pane(),
            crate::navigation::step(pane, app.pane_names().len(), 1)
        );
        let _ = app.on_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::SHIFT));
        assert_eq!(app.focused_pane(), pane);
        let _ = app.on_key(key(KeyCode::F(2)));
        assert!(app.navigation.open);
        let _ = app.on_key(key(KeyCode::Char('1')));
        assert!(!app.navigation.open);
        assert_eq!(app.navigation_screen(), 0);
        app.select_screen(screen);
        let _ = app.on_key(key(KeyCode::F(2)));
        assert!(app.navigation.open);

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(crate::audit::text(terminal.backend().buffer()).contains("Navigate"));
        let _ = app.on_key(key(KeyCode::Esc));
        assert!(!app.navigation.open);
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut hit = None;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width.saturating_sub(3) {
                if buffer[(x, y)].symbol() == "("
                    && buffer[(x + 1, y)].symbol() == "F"
                    && buffer[(x + 2, y)].symbol() == "2"
                {
                    hit = Some((x, y));
                    break;
                }
            }
            if hit.is_some() {
                break;
            }
        }
        let (x, y) = hit.expect("visible clickable navigation");
        app.navigation_mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(app.navigation.open);
        let _ = app.on_key(key(KeyCode::Esc));

        let _ = app.on_key(commands());
        assert!(app.commands.open);
        let _ = app.on_key(key(KeyCode::Esc));
    }
    #[tokio::test]
    async fn all_screens_and_focus_targets_fit_the_navigation_matrix() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        let screens = app
            .navigation_entries()
            .iter()
            .filter(|e| matches!(e.target, crate::navigation::Intent::Screen(_)))
            .count();
        for screen in 0..screens {
            app.select_screen(screen);
            for pane in 0..app.pane_names().len() {
                app.focus_pane(pane);
                for &(w, h) in &crate::audit::SIZES[..4] {
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal.draw(|f| draw(f, &mut app)).unwrap();
                    let buffer = terminal.backend().buffer();
                    assert!(
                        crate::audit::text(buffer).contains("(F2)"),
                        "screen {screen} pane {pane} must expose navigation at {w}x{h}"
                    );
                    crate::audit::verify_footer(buffer);
                    crate::audit::capture(
                        "luma",
                        &format!("navigation-screen{screen}-pane{pane}"),
                        buffer,
                    );
                    let before = app.navigation_screen();
                    app.navigation_mouse(crossterm::event::MouseEvent {
                        kind: crossterm::event::MouseEventKind::Down(
                            crossterm::event::MouseButton::Left,
                        ),
                        column: w + 1,
                        row: h + 1,
                        modifiers: KeyModifiers::NONE,
                    });
                    assert_eq!(app.navigation_screen(), before);
                }
            }
        }
    }
    #[test]
    fn tab_completes_before_navigation_and_picker_owns_keys() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        app.composer = crate::field::Field::new("/he");
        app.on_key(key(KeyCode::Tab));
        assert!(app.screen == Screen::Chat);
        assert!(app.composer.value.starts_with("/help"));
        app.model_picker = true;
        app.on_key(key(KeyCode::Tab));
        assert!(app.screen == Screen::Chat);
        app.on_key(commands());
        assert!(!app.commands.open);
        app.model_picker = false;
        app.composer.clear();
        app.on_key(commands());
        assert!(app.commands.open);
        app.on_key(key(KeyCode::Tab));
        assert!(app.screen == Screen::Chat);
    }
    #[test]
    #[ignore = "interactive mock fixture for visual QA"]
    fn live_preview() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        match std::env::var("AURORA_NAV_PREVIEW").as_deref() {
            Ok("menu") => app.navigation.toggle(),
            Ok("actions") => app.commands.open = true,
            Ok("focused") => app.focus_pane(1),
            _ => {}
        }
        crate::audit::live_preview(
            &mut app,
            |f, a| draw(f, a),
            |a, k| {
                if let KeyCode::Char(c @ '1'..='4') = k.code {
                    a.screen = Screen::ALL[c as usize - '1' as usize];
                } else if matches!(
                    k.code,
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Down | KeyCode::Up | KeyCode::Esc
                ) {
                    a.on_key(k);
                }
            },
        );
    }
    #[test]
    fn responsive_render_matrix() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("config.toml"));
        // One terminal exercises shrink/grow transitions, rather than independent initial frames.
        let mut resizing = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for &(w, h) in &[
            (120, 40),
            (60, 20),
            (80, 24),
            (30, 10),
            (100, 30),
            (120, 40),
        ] {
            resizing.backend_mut().resize(w, h);
            resizing.draw(|f| draw(f, &mut app)).unwrap();
            assert_eq!(resizing.backend().buffer().area.width, w);
        }
        for &(w, h) in crate::audit::SIZES {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            let buffer = term.backend().buffer();
            if w >= 30 && h >= 14 {
                assert!(buffer
                    .content
                    .iter()
                    .any(|cell| matches!(cell.symbol(), "█" | "▀" | "▄")));
            }
            crate::audit::verify_footer(buffer);
            crate::audit::capture("luma", "main", buffer);
        }
        app.chat_pane = crate::app::ChatPane::Sidebar;
        for &(w, h) in &crate::audit::SIZES[..4] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            crate::audit::capture("luma", "focused", term.backend().buffer());
        }
        app.on_key(commands());
        for &(w, h) in &crate::audit::SIZES[..4] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            let buffer = term.backend().buffer();
            assert!(crate::audit::text(buffer).contains("Actions"));
            crate::audit::capture("luma", "actions", buffer);
        }
        app.commands.open = false;
        app.chats.clear();
        app.connected = Some(false);
        app.chat_pane = crate::app::ChatPane::Composer;
        for tab in Screen::ALL {
            app.screen = tab;
            for &(w, h) in &crate::audit::SIZES[..4] {
                let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                term.draw(|f| draw(f, &mut app)).unwrap();
                crate::audit::verify_footer(term.backend().buffer());
                crate::audit::capture("luma", "offline", term.backend().buffer());
            }
        }
    }
}
