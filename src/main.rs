mod automaton;
mod components;
mod state;
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
    Terminal,
};

use automaton::Machine;
use components::*;
use state::*;
use task::{Filter, TaskStore};

#[derive(Debug, Default)]
pub(crate) struct AppData {
    pub(crate) store: TaskStore,
    pub(crate) window_size: (u16, u16),
}

#[derive(Debug)]
struct Search {
    filter: Filter,
    list: TaskList,
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

pub(crate) enum Action {
    Key(KeyEvent),
}

#[derive(Debug, Default)]
pub(crate) struct Tasker {
    pub(crate) tasklist: TaskList,
    pub(crate) quick_input: Option<QuickInput>,
    pub(crate) quick_select: Option<QuickSelect>,
    pub(crate) search: Option<Search>,
    pub(crate) pane: Pane,
    pub(crate) data: AppData,
    pub(crate) filter: Filter,
}

impl Tasker {
    fn show(&mut self, terminal: &mut Terminal<impl Backend>) -> CResult<()> {
        terminal.draw(|f| {
            let constraints = if let Some(_search) = &mut self.search {
                vec![
                    Constraint::Min(2),
                    Constraint::Percentage(50),
                    Constraint::Length(1),
                ]
            } else {
                vec![Constraint::Min(2), Constraint::Length(1)]
            };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(f.size());

            match &mut self.pane {
                Pane::Main => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(2), Constraint::Length(5)])
                        .split(chunks[0]);
                    self.tasklist.show(&self.data, f, chunks[0]);
                    if let Some(id) = self.tasklist.selection() {
                        TaskView::new(id, &self.data, false).show(&self.data, f, chunks[1]);
                    }
                }
                Pane::OneTask(view) => {
                    view.show(&self.data, f, chunks[0]);
                }
            }

            if let Some(input) = &self.quick_input {
                let block = *chunks.last().unwrap();
                let (text, pos) = input.show(&self.data);
                f.render_widget(text, block);
                f.set_cursor(block.left() + pos, block.top());
            }

            if let Some(input) = &self.quick_select {
                let text = input.show(&self.data);
                f.render_widget(text, chunks[1]);
            }

            if let Some(search) = &mut self.search {
                search.list.show(&self.data, f, chunks[1]);
            }
        })?;

        Ok(())
    }
}

fn event_loop(mut terminal: Terminal<impl Backend>) -> CResult<()> {
    let mut tasker = Tasker::default();
    let mut machine = Machine::new(NormalState);
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
                    machine.act(&mut tasker, Action::Key(key));
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
