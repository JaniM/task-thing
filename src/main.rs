mod task;

use std::{io::stdout, time::Duration};

use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    execute,
    terminal::{self, disable_raw_mode, enable_raw_mode},
    Result as CResult,
};

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use unicode_segmentation::UnicodeSegmentation;

use task::{Filter, TaskId, TaskStore};

#[derive(Debug, Default)]
struct AppData {
    store: TaskStore,
    window_size: (u16, u16),
}

fn status_to_span(status: task::Status) -> Span<'static> {
    match status {
        task::Status::Todo => Span::styled("TODO", Style::default().add_modifier(Modifier::BOLD)),
        task::Status::Done => Span::styled("DONE", Style::default().add_modifier(Modifier::DIM)),
    }
}

#[derive(Debug, Default)]
struct TaskList {
    tasks: Vec<TaskId>,
    selection: usize,
    list_state: ListState,
}

impl TaskList {
    fn selection(&self) -> Option<TaskId> {
        self.tasks.get(self.selection).copied()
    }

    fn show<'a>(&mut self, data: &'a AppData, frame: &mut Frame<impl Backend>, size: Rect) {
        // ui::rectangle(stdout, 0, 0, 80, 20)?;
        let mut items = vec![];
        for id in &self.tasks {
            let mut spans = vec![];
            let task = data.store.get_task(*id);
            spans.push(status_to_span(task.status));
            spans.push(Span::raw(" "));
            spans.push(Span::raw(&task.title));
            items.push(ListItem::new(vec![Spans::from(spans)]));
        }
        let block = Block::default().borders(Borders::TOP).title(" Tasks ");
        let inner = block.inner(size);
        frame.render_widget(block, size);
        let chunks = Layout::default()
            .horizontal_margin(1)
            .constraints([Constraint::Min(0)])
            .split(inner);
        let list = List::new(items).highlight_style(Style::default().bg(Color::DarkGray));
        self.list_state.select(Some(self.selection));
        frame.render_stateful_widget(list, chunks[0], &mut self.list_state);
    }
}

#[derive(Debug)]
struct TaskView {
    task_id: TaskId,
}

impl TaskView {
    fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }

    fn show<'a>(&'a self, data: &'a AppData, frame: &mut Frame<impl Backend>, size: Rect) {
        let task = data.store.get_task(self.task_id);
        let block = Block::default()
            .borders(Borders::TOP)
            .title(Spans::from(vec![
                Span::from(" "),
                Span::from(task.title.as_str()),
                Span::from(" "),
            ]));
        frame.render_widget(block, size);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(size);

        let text = vec![Spans::from(vec![
            Span::from("Status: "),
            status_to_span(task.status),
        ])];
        let text = Paragraph::new(text);
        frame.render_widget(text, chunks[0]);

        let description = Text::raw(task.description.as_str());
        let paragraph = Paragraph::new(description).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, chunks[1]);
    }
}

#[derive(Debug, Default)]
struct QuickInput {
    title: String,
    text: String,
    continuous: bool,
}

impl QuickInput {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            text: String::new(),
            continuous: false,
        }
    }

    fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    fn continuous(mut self) -> Self {
        self.continuous = true;
        self
    }

    fn show(&self, _data: &AppData) -> (Paragraph, u16) {
        let text = Paragraph::new(vec![Spans::from(vec![
            Span::from(self.title.as_str()),
            Span::from(": "),
            Span::from(self.text.as_str()),
        ])]);
        (
            text,
            self.text.graphemes(true).count() as u16 + self.title.len() as u16 + 2,
        )
    }
}

#[derive(Debug, Default)]
struct QuickSelect {
    title: String,
    choices: Vec<(char, String)>,
}

impl QuickSelect {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            choices: Vec::new(),
        }
    }

    fn choices(mut self, choices: impl IntoIterator<Item = (char, impl Into<String>)>) -> Self {
        self.choices
            .extend(choices.into_iter().map(|x| (x.0, x.1.into())));
        self
    }

    fn show(&self, _data: &AppData) -> Paragraph {
        let mut spans = vec![Span::from(self.title.as_str()), Span::from(": ")];
        for (key, text) in &self.choices {
            spans.push(Span::raw(format!("[{}] {} ", key, text)));
        }
        let text = Paragraph::new(vec![Spans::from(spans)]);
        text
    }
}

#[derive(Debug)]
enum AppState {
    Normal,
    Input(QuickInput),
    Select(QuickSelect),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Normal
    }
}

#[derive(Debug)]
enum Pane {
    Main,
    OneTask(TaskView),
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Main
    }
}

#[derive(Debug)]
enum Command {
    QuickNew,
    SetFilter,
    SelectFilter,
    SetDescription(TaskId),
    Text(String),
}

#[derive(Debug, Default)]
struct Tasker {
    tasklist: TaskList,
    state: AppState,
    pane: Pane,
    data: AppData,
    filter: Filter,
    commands: Vec<Command>,
}

impl Tasker {
    fn execute_command(&mut self, done: bool) {
        match self.commands.get(0) {
            Some(Command::QuickNew) => {
                if let Some(Command::Text(text)) = self.commands.get(1) {
                    let task = self.data.store.new_task();
                    task.title = text.clone();
                    self.tasklist.tasks.push(task.id);
                    self.tasklist.selection = self.tasklist.tasks.len() - 1;
                }
            }
            Some(Command::SetDescription(id)) => {
                if let Some(Command::Text(text)) = self.commands.get(1) {
                    let task = self.data.store.get_task_mut(*id);
                    task.description = text.clone();
                }
            }
            Some(Command::SetFilter) => {
                if let Some(Command::Text(text)) = self.commands.get(1) {
                    self.filter.title = text.clone();
                    self.tasklist.tasks = self.filter.apply(&self.data.store);
                    self.tasklist.selection = 0;
                    self.tasklist.list_state = Default::default();
                }
            }
            Some(Command::SelectFilter) => {
                if let Some(Command::Text(text)) = self.commands.get(1) {
                    if text == "Title" {
                        self.commands.clear();
                        self.commands.push(Command::SetFilter);
                        self.state = AppState::Input(
                            QuickInput::new("Filter [Title]")
                                .text(self.filter.title.clone())
                                .continuous(),
                        );
                        return;
                    }
                    if text == "Todo" {
                        self.filter.status = Some(task::Status::Todo);
                    }
                    if text == "Done" {
                        self.filter.status = Some(task::Status::Done);
                    }
                    if text == "Clear" {
                        self.filter = Filter::default();
                    }
                    self.tasklist.tasks = self.filter.apply(&self.data.store);
                    self.tasklist.selection = 0;
                    self.tasklist.list_state = Default::default();
                    self.state = AppState::Normal;
                }
            }
            _ => {}
        }
        if done {
            self.commands.clear();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.state {
            state @ AppState::Normal => {
                match key.code {
                    KeyCode::Char('n') => {
                        self.commands.push(Command::QuickNew);
                        *state = AppState::Input(QuickInput::new("Title"));
                    }
                    KeyCode::Char('e') => {
                        if let Some(id) = self.tasklist.selection() {
                            let task = self.data.store.get_task(id);
                            self.commands.push(Command::SetDescription(id));
                            *state = AppState::Input(
                                QuickInput::new("Description").text(task.description.clone()),
                            );
                        }
                    }
                    _ => {}
                }
                if matches!(self.pane, Pane::Main) {
                    match key.code {
                        KeyCode::Char('f') => {
                            self.commands.push(Command::SelectFilter);
                            *state = AppState::Select(QuickSelect::new("Filter").choices(vec![
                                ('t', "Title"),
                                ('d', "Todo"),
                                ('D', "Done"),
                                ('c', "Clear"),
                            ]));
                        }
                        KeyCode::Up => {
                            self.tasklist.selection = self.tasklist.selection.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            if self.tasklist.selection + 1 < self.tasklist.tasks.len() {
                                self.tasklist.selection += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(id) = self.tasklist.selection() {
                                self.pane = Pane::OneTask(TaskView::new(id));
                            }
                        }
                        KeyCode::Char(' ') => {
                            if let Some(id) = self.tasklist.selection() {
                                let task = self.data.store.get_task_mut(id);
                                task.toggle_status();
                            }
                        }
                        KeyCode::Char('m') => {
                            let task = self.data.store.new_task();
                            task.title = task.id.id().to_string();
                            self.tasklist.tasks.push(task.id);
                            self.tasklist.selection = self.tasklist.tasks.len() - 1;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Enter => {
                            self.pane = Pane::Main;
                        }
                        _ => {}
                    }
                }
            }
            AppState::Input(input) => {
                let mut send = false;
                if let KeyCode::Char(c) = key.code {
                    input.text.push(c);
                    send = true;
                }
                if key.code == KeyCode::Backspace {
                    input.text.pop();
                    send = true;
                }
                if send && input.continuous {
                    if let Some(Command::Text(_)) = self.commands.last() {
                        self.commands.pop();
                    }
                    self.commands.push(Command::Text(input.text.clone()));
                    self.execute_command(false);
                } else if key.code == KeyCode::Enter {
                    self.commands.push(Command::Text(input.text.clone()));
                    self.execute_command(true);
                    self.state = AppState::Normal;
                }

                if key.code == KeyCode::Esc {
                    self.commands.clear();
                    self.state = AppState::Normal;
                }
            }
            AppState::Select(input) => {
                if let KeyCode::Char(c) = key.code {
                    if let Some(choice) = input.choices.iter().find(|x| x.0 == c) {
                        self.commands.push(Command::Text(choice.1.clone()));
                        self.execute_command(true);
                    }
                }
                if key.code == KeyCode::Esc {
                    self.commands.clear();
                    self.state = AppState::Normal;
                }
            }
        }
    }

    fn show(&mut self, terminal: &mut Terminal<impl Backend>) -> CResult<()> {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(2), Constraint::Length(1)])
                .split(f.size());

            match &self.pane {
                Pane::Main => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(2), Constraint::Length(5)])
                        .split(chunks[0]);
                    self.tasklist.show(&self.data, f, chunks[0]);
                    if let Some(id) = self.tasklist.selection() {
                        TaskView::new(id).show(&self.data, f, chunks[1]);
                    }
                }
                Pane::OneTask(view) => {
                    view.show(&self.data, f, chunks[0]);
                }
            }

            if let AppState::Input(input) = &self.state {
                let (text, pos) = input.show(&self.data);
                f.render_widget(text, chunks[1]);
                f.set_cursor(chunks[1].left() + pos, chunks[1].top());
            }

            if let AppState::Select(input) = &self.state {
                let text = input.show(&self.data);
                f.render_widget(text, chunks[1]);
            }
        })?;

        Ok(())
    }
}

fn event_loop(mut terminal: Terminal<impl Backend>) -> CResult<()> {
    let mut tasker = Tasker::default();
    tasker.data.window_size = terminal::size()?;
    loop {
        tasker.show(&mut terminal)?;
        // Wait up to 1s for another event
        if poll(Duration::from_millis(1_000))? {
            // It's guaranteed that read() wont block if `poll` returns `Ok(true)`
            let event = read()?;

            match event {
                Event::Resize(w, h) => {
                    tasker.data.window_size = (w, h);
                }
                Event::Key(k)
                    if k.code == KeyCode::Char('c')
                        && k.modifiers == crossterm::event::KeyModifiers::CONTROL =>
                {
                    break;
                }
                Event::Key(key) => {
                    tasker.handle_key(key);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn main() -> CResult<()> {
    enable_raw_mode()?;
    execute!(stdout(), terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;

    if let Err(e) = event_loop(terminal) {
        println!("Error: {:?}\r", e);
    }

    execute!(stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
    disable_raw_mode()
}
