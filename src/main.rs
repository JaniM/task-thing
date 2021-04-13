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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

use task::{TaskId, TaskStore};

#[derive(Debug, Default)]
struct AppData {
    store: TaskStore,
    window_size: (u16, u16),
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

    fn show(&mut self, data: &AppData) -> (List, &mut ListState) {
        // ui::rectangle(stdout, 0, 0, 80, 20)?;
        let mut items = vec![];
        for id in &self.tasks {
            let mut spans = vec![];
            let task = data.store.get_task(*id);
            match task.status {
                task::Status::Todo => {
                    spans.push(Span::styled(
                        "TODO ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
                task::Status::Done => {
                    spans.push(Span::styled(
                        "DONE ",
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
            }
            spans.push(Span::raw(task.title.clone()));
            items.push(ListItem::new(vec![Spans::from(spans)]));
        }
        let list = List::new(items)
            .block(Block::default().borders(Borders::all()).title("Tasks"))
            .highlight_style(Style::default().bg(Color::DarkGray));
        self.list_state.select(Some(self.selection));
        (list, &mut self.list_state)
    }
}

#[derive(Debug, Default)]
struct TaskInput {
    title: String,
}

impl TaskInput {
    fn show(&self, _data: &AppData) -> (Paragraph, u16) {
        let text = Paragraph::new(vec![Spans::from(vec![
            Span::from("Title: "),
            Span::from(self.title.as_str()),
        ])]);
        (text, self.title.len() as u16 + 7)
    }
}

#[derive(Debug)]
enum AppState {
    Normal,
    Input(TaskInput),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Normal
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Pane {
    Main,
    OneTask(TaskId),
}

impl Default for Pane {
    fn default() -> Self {
        Pane::Main
    }
}

#[derive(Debug, Default)]
struct Tasker {
    tasklist: TaskList,
    state: AppState,
    pane: Pane,
    data: AppData,
}

impl Tasker {
    fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.state {
            state @ AppState::Normal => {
                if self.pane == Pane::Main {
                    match key.code {
                        KeyCode::Char('n') => {
                            *state = AppState::Input(TaskInput::default());
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
                                self.pane = Pane::OneTask(id);
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
                        KeyCode::Char('n') => {
                            *state = AppState::Input(TaskInput::default());
                        }
                        KeyCode::Enter => {
                            self.pane = Pane::Main;
                        }
                        _ => {}
                    }
                }
            }
            AppState::Input(input) => {
                if let KeyCode::Char(c) = key.code {
                    input.title.push(c);
                }
                if key.code == KeyCode::Enter {
                    let task = self.data.store.new_task();
                    task.title = input.title.clone();
                    self.tasklist.tasks.push(task.id);
                    self.state = AppState::Normal;
                    self.tasklist.selection = self.tasklist.tasks.len() - 1;
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

            let (list, state) = self.tasklist.show(&self.data);
            f.render_stateful_widget(list, chunks[0], state);

            if let AppState::Input(input) = &self.state {
                let (text, pos) = input.show(&self.data);
                f.render_widget(text, chunks[1]);
                f.set_cursor(chunks[1].left() + pos, chunks[1].top());
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
                Event::Key(k) if k == KeyCode::Esc.into() => {
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
