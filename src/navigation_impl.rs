const NAV_SCREENS: &[crate::navigation::Screen] = &[
    crate::navigation::Screen {
        name: "Chat",
        key: '1',
    },
    crate::navigation::Screen {
        name: "Models",
        key: '2',
    },
    crate::navigation::Screen {
        name: "Marketplace",
        key: '3',
    },
    crate::navigation::Screen {
        name: "Settings",
        key: '4',
    },
];
impl App {
    pub fn navigation_skin(&self) -> crate::navigation::Skin {
        let t = &self.theme;
        crate::navigation::Skin {
            base: t.base(),
            muted: ratatui::style::Style::default().fg(t.muted),
            accent: ratatui::style::Style::default().fg(t.accent),
            selected: t.selected(),
            border: ratatui::style::Style::default().fg(t.border),
        }
    }
    pub fn navigation_screen(&self) -> usize {
        self.screen.index()
    }
    pub fn pane_names(&self) -> &'static [&'static str] {
        match self.screen {
            Screen::Chat => &["Conversations", "Transcript", "Composer"],
            Screen::Models => &["Models", "Running models", "Pull input"],
            Screen::Marketplace => &["Search", "Results", "Files"],
            Screen::Settings => &["Settings"],
        }
    }
    pub fn focused_pane(&self) -> usize {
        match self.screen {
            Screen::Chat => match self.chat_pane {
                ChatPane::Sidebar => 0,
                ChatPane::Transcript => 1,
                ChatPane::Composer => 2,
            },
            Screen::Models => match self.models_pane {
                ModelsPane::List => 0,
                ModelsPane::Running => 1,
                ModelsPane::Pull => 2,
            },
            Screen::Marketplace => match self.hf_pane {
                MarketPane::Search => 0,
                MarketPane::Results => 1,
                MarketPane::Files => 2,
            },
            Screen::Settings => 0,
        }
    }
    pub fn navigation_editing(&self) -> bool {
        self.typing()
    }
    pub fn navigation_entries(&self) -> Vec<crate::navigation::Entry> {
        crate::navigation::entries(NAV_SCREENS, self.pane_names())
    }
    pub fn navigation_bar(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        self.navigation.bar(
            frame,
            area,
            NAV_SCREENS,
            self.navigation_screen(),
            self.pane_names(),
            self.focused_pane(),
            self.navigation_editing(),
            self.navigation_skin(),
        );
    }
    pub fn draw_navigation(&self, frame: &mut ratatui::Frame) {
        self.navigation
            .draw(frame, &self.navigation_entries(), self.navigation_skin());
    }
    pub fn focus_pane(&mut self, p: usize) {
        let p = p.min(self.pane_names().len().saturating_sub(1));
        let screen = self.navigation_screen();
        self.navigation.focus[screen] = p;
        match self.screen {
            Screen::Chat => {
                self.chat_pane = match p {
                    0 => ChatPane::Sidebar,
                    1 => ChatPane::Transcript,
                    _ => ChatPane::Composer,
                }
            }
            Screen::Models => {
                self.models_pane = match p {
                    0 => ModelsPane::List,
                    1 => ModelsPane::Running,
                    _ => ModelsPane::Pull,
                }
            }
            Screen::Marketplace => {
                self.hf_pane = match p {
                    0 => MarketPane::Search,
                    1 => MarketPane::Results,
                    _ => MarketPane::Files,
                }
            }
            Screen::Settings => {}
        }
    }
    pub fn select_screen(&mut self, screen: usize) {
        if screen >= NAV_SCREENS.len() {
            return;
        }
        let old = self.navigation_screen();
        if old == screen {
            return;
        }
        self.navigation.focus[old] = self.focused_pane();
        self.switch_screen(Screen::ALL[screen]);
        self.focus_pane(self.navigation.focus[screen]);
    }
    fn navigation_key(&mut self, key: KeyEvent) -> bool {
        if self.commands.open {
            return false;
        }
        if self.navigation.open {
            let entries = self.navigation_entries();
            if let Some(intent) = self.navigation.key(key, &entries) {
                self.dispatch_intent(intent);
            }
            return true;
        }
        match key.code {
            KeyCode::F(2) => {
                self.navigation.toggle();
                true
            }
            KeyCode::F(6) => {
                let direction = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    -1
                } else {
                    1
                };
                self.focus_pane(crate::navigation::step(
                    self.focused_pane(),
                    self.pane_names().len(),
                    direction,
                ));
                true
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if !self.navigation_editing() {
                    let d = if key.code == KeyCode::Tab {
                        -1i32 + 2
                    } else {
                        -1
                    };
                    self.select_screen(crate::navigation::step(
                        self.navigation_screen(),
                        NAV_SCREENS.len(),
                        d,
                    ));
                }
                true
            }
            KeyCode::Char(c)
                if !self.navigation_editing()
                    && key.modifiers.is_empty()
                    && NAV_SCREENS.iter().any(|s| s.key == c) =>
            {
                let i = NAV_SCREENS.iter().position(|s| s.key == c).unwrap();
                self.select_screen(i);
                true
            }
            KeyCode::Esc => {
                let p = if self.navigation_editing()
                    && self.focused_pane() == 0
                    && self.pane_names().len() > 1
                {
                    1
                } else {
                    self.focused_pane().saturating_sub(1)
                };
                self.focus_pane(p);
                true
            }
            KeyCode::Down | KeyCode::Up | KeyCode::Char('j' | 'k')
                if !self.navigation_editing()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let d = if matches!(key.code, KeyCode::Down | KeyCode::Char('j')) {
                    1
                } else {
                    -1
                };
                self.navigation_scroll(self.focused_pane(), d);
                true
            }
            KeyCode::PageDown | KeyCode::PageUp
                if !self.navigation_editing()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let d = if key.code == KeyCode::PageDown { 8 } else { -8 };
                self.navigation_scroll(self.focused_pane(), d);
                true
            }
            KeyCode::F(1) => {
                self.help = true;
                true
            }
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigation_cancel();
                true
            }
            _ => false,
        }
    }
    fn navigation_cancel(&mut self) {
        if self.streaming {
            self.stop_generation();
        } else if self.pull.is_some() {
            self.pull_job = self.pull_job.wrapping_add(1);
            if let Some(task) = self.pull_task.take() {
                task.abort();
            }
            self.pull = None;
            self.info("Download cancelled");
        } else {
            self.info("No cancellable operation is running");
        }
    }
    pub fn dispatch_intent(&mut self, intent: crate::navigation::Intent) {
        use crate::navigation::Intent;
        match intent {
            Intent::Screen(screen) => {
                self.navigation.open = false;
                self.select_screen(screen)
            }
            Intent::Pane(p) => {
                self.navigation.open = false;
                self.focus_pane(p)
            }
            Intent::Menu => self.navigation.toggle(),
            Intent::Details(p) => self.focus_pane(p),
            Intent::ScrollDetails(p, d) => {
                self.focus_pane(p);
                self.navigation_details_scroll(p, d)
            }
            Intent::Selector(i) => {
                self.navigation.open = false;
                self.navigation_selector(i)
            }
            Intent::Theme(id) => self.apply_aurora_theme(id),
            Intent::Command(id) => self.navigation_command(id),
            Intent::Row(p, i) => {
                self.focus_pane(p);
                self.navigation_row(p, i, false)
            }
            Intent::Activate(p, i) => {
                self.focus_pane(p);
                self.navigation_row(p, i, true)
            }
            Intent::Scroll(p, d) => {
                self.focus_pane(p);
                self.navigation_scroll(p, d)
            }
        }
    }
    pub fn command_actions(&self) -> Vec<crate::commands::Action> {
        let mut actions = NAV_SCREENS
            .iter()
            .enumerate()
            .map(|(i, s)| crate::commands::Action::screen(s.name, i, s.key))
            .collect::<Vec<_>>();
        actions.extend(self.pane_names().iter().enumerate().map(|(i, p)| {
            crate::commands::Action::function(p, 6, crate::navigation::Intent::Pane(i))
        }));
        actions.push(crate::commands::Action::function(
            "Navigate",
            2,
            crate::navigation::Intent::Menu,
        ));
        actions.extend(
            ACTIONS
                .iter()
                .filter(|a| !matches!(a.key, KeyCode::Char('0'..='9')))
                .cloned(),
        );
        actions
    }
}

impl App {
    fn navigation_command(&mut self, id: &str) {
        match id {
            "Confirm" => self.confirm_yes(),
            "Cancel" => self.confirm = None,
            "New conversation" => self.new_chat(),
            "Help" => self.help = true,
            "Start Ollama" => self.begin_start(),
            "Choose model" => self.open_model_picker(),
            "Delete conversation" => self.ask_delete_chat(),
            "Delete model" => self.ask_delete_model(),
            "Unload model" => self.unload_selected(),
            "Pull model" => self.focus_pane(2),
            "Search marketplace" => self.start_search(),
            _ => {}
        }
    }
    fn navigation_row(&mut self, pane: usize, index: usize, activate: bool) {
        if pane == 96 || pane == 97 {
            self.slash_pick = index;
            if activate {
                if pane == 97 {
                    self.accept_model_palette();
                } else {
                    self.complete_slash();
                }
            }
            return;
        }
        match self.screen {
            Screen::Chat => match pane {
                0 => {
                    self.move_chat(index as isize - self.chat_index as isize);
                    if activate {
                        self.focus_pane(2);
                    }
                }
                1 => {
                    self.transcript_cursor = index;
                    if activate {
                        self.toggle_thinking_or_compose();
                    }
                }
                _ => {}
            },
            Screen::Models => {
                if pane == 0 && index < self.models.len() {
                    self.model_index = index;
                    self.maybe_show();
                    if activate {
                        self.use_selected_model();
                    }
                }
            }
            Screen::Marketplace => match pane {
                1 => {
                    if index < self.hf_results.len() {
                        self.hf_index = index;
                        if activate {
                            self.open_repo();
                        }
                    }
                }
                2 => {
                    if index < self.hf_files.len() {
                        self.hf_file_index = index;
                        if activate {
                            self.pull_selected_file();
                        }
                    }
                }
                _ => {}
            },
            Screen::Settings => {
                self.settings_index = index.min(Setting::ALL.len().saturating_sub(1));
                if activate {
                    self.activate_setting();
                }
            }
        }
    }
    fn navigation_scroll(&mut self, pane: usize, d: i32) {
        match self.screen {
            Screen::Chat => {
                if pane == 0 {
                    self.move_chat(d as isize)
                } else {
                    self.scroll_lines(d as isize * 3)
                }
            }
            Screen::Models if pane == 1 => {
                self.navigation.scroll[1] =
                    (self.navigation.scroll[1] as i32 + d * 3).clamp(0, u16::MAX as i32) as u16;
            }
            Screen::Models => {
                self.model_index = (self.model_index as i32 + d)
                    .max(0)
                    .min(self.models.len().saturating_sub(1) as i32)
                    as usize;
                self.maybe_show();
            }
            Screen::Marketplace => match pane {
                1 => {
                    self.hf_index = (self.hf_index as i32 + d)
                        .max(0)
                        .min(self.hf_results.len().saturating_sub(1) as i32)
                        as usize
                }
                2 => {
                    self.hf_file_index = (self.hf_file_index as i32 + d)
                        .max(0)
                        .min(self.hf_files.len().saturating_sub(1) as i32)
                        as usize
                }
                _ => {}
            },
            Screen::Settings => {
                self.settings_index = (self.settings_index as i32 + d)
                    .clamp(0, Setting::ALL.len() as i32 - 1)
                    as usize
            }
        }
    }
    pub fn navigation_mouse(&mut self, event: crossterm::event::MouseEvent) {
        if self.confirm.is_some() {
            if let Some(intent) = self.navigation.mouse(event) {
                if matches!(
                    intent,
                    crate::navigation::Intent::Command("Confirm" | "Cancel")
                ) {
                    self.dispatch_intent(intent);
                }
            }
            return;
        }
        if self.model_picker {
            if let Some(intent) = self.navigation.mouse(event) {
                match intent {
                    crate::navigation::Intent::Row(98, i) => {
                        self.model_pick = i.min(self.model_choices().len().saturating_sub(1))
                    }
                    crate::navigation::Intent::Activate(98, i) => {
                        self.model_pick = i.min(self.model_choices().len().saturating_sub(1));
                        self.apply_model_pick();
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.renaming || self.editing_system || self.help || self.editing || self.picking_theme {
            return;
        }
        if self.navigation.open {
            let entries = self.navigation_entries();
            if let Some(intent) = self.navigation.menu_mouse(event, &entries) {
                self.dispatch_intent(intent);
            }
            return;
        }
        if self.commands.open {
            if let Some(action) = self.commands.mouse(event, &self.command_actions()) {
                self.dispatch_intent(action.target);
            }
            return;
        }
        if let Some(intent) = self.navigation.mouse(event) {
            self.dispatch_intent(intent);
        }
    }
}

impl App {
    fn navigation_details_scroll(&mut self, p: usize, d: i32) {
        self.navigation_scroll(p, d);
    }
    fn navigation_selector(&mut self, _i: usize) {}
}

impl App {
    pub fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.screen {
            Screen::Chat => match self.chat_pane {
                ChatPane::Composer => {
                    let mut v = vec![
                        ("Enter", "send"),
                        ("Ctrl+K", "actions"),
                        ("Alt+Enter", "newline"),
                        ("Esc", "leave input"),
                    ];
                    if self.streaming {
                        v.insert(0, ("Ctrl+X", "stop"));
                    }
                    v
                }
                ChatPane::Sidebar => vec![
                    ("Enter", "compose"),
                    ("Ctrl+K", "actions"),
                    ("Ctrl+N", "new chat"),
                    ("↑/↓", "select"),
                ],
                ChatPane::Transcript => vec![
                    ("Enter", "expand/compose"),
                    ("Ctrl+K", "actions"),
                    ("PgUp/PgDn", "scroll"),
                ],
            },
            Screen::Models => {
                if self.models_pane == ModelsPane::Running {
                    vec![
                        ("PgUp/PgDn", "scroll"),
                        ("Ctrl+K", "actions"),
                        ("F6", "pane"),
                    ]
                } else if self.models_pane == ModelsPane::Pull {
                    vec![
                        ("Enter", "pull"),
                        ("Ctrl+K", "actions"),
                        ("Esc", "leave input"),
                    ]
                } else {
                    vec![
                        ("Enter", "use model"),
                        ("Ctrl+K", "actions"),
                        ("u", "unload"),
                        ("d", "delete"),
                    ]
                }
            }
            Screen::Marketplace => match self.hf_pane {
                MarketPane::Search => vec![
                    ("Enter", "search"),
                    ("Ctrl+K", "actions"),
                    ("Esc", "leave input"),
                ],
                MarketPane::Results => vec![
                    ("Enter", "open files"),
                    ("Ctrl+K", "actions"),
                    ("Ctrl+S", "sort"),
                ],
                MarketPane::Files => vec![
                    ("Enter", "pull file"),
                    ("Ctrl+K", "actions"),
                    ("Esc", "results"),
                ],
            },
            Screen::Settings => vec![("Enter", "edit"), ("Ctrl+K", "actions"), ("←/→", "change")],
        }
    }
}
