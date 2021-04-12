mod task;
mod ui;

use std::{
    io::{stdout, Stdout, Write},
    time::Duration,
};

use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    execute, queue,
    style::{self, Colorize, Print, Styler},
    terminal::{self, disable_raw_mode, enable_raw_mode},
    Result as CResult,
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
}

impl TaskList {
    fn selection(&self) -> Option<TaskId> {
        self.tasks.get(self.selection).copied()
    }

    fn show(&self, stdout: &mut Stdout, data: &AppData) -> CResult<()> {
        // ui::rectangle(stdout, 0, 0, 80, 20)?;
        let (w, h) = data.window_size;
        for (i, id) in self.tasks.iter().enumerate() {
            let task = data.store.get_task(*id);
            if i == self.selection {
                queue!(
                    stdout,
                    style::SetBackgroundColor(style::Color::DarkGrey),
                    cursor::MoveTo(1, i as u16 + 1),
                    Print(" ".repeat(w as usize - 2))
                )?;
            }
            queue!(stdout, cursor::MoveTo(2, i as u16 + 1))?;
            match task.status {
                task::Status::Todo => {
                    queue!(
                        stdout,
                        style::SetAttribute(style::Attribute::Bold),
                        Print("TODO "),
                        style::SetAttribute(style::Attribute::Reset)
                    )?;
                }
                task::Status::Done => {
                    queue!(
                        stdout,
                        style::SetAttribute(style::Attribute::Dim),
                        Print("DONE "),
                        style::SetAttribute(style::Attribute::Reset)
                    )?;
                }
            }
            if i == self.selection {
                queue!(stdout, style::SetBackgroundColor(style::Color::DarkGrey),)?;
            }
            queue!(stdout, Print(&task.title))?;
            if i == self.selection {
                queue!(stdout, style::ResetColor)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct TaskInput {
    title: String,
}

impl TaskInput {
    fn show(&self, stdout: &mut Stdout, data: &AppData) -> CResult<()> {
        queue!(
            stdout,
            cursor::MoveTo(0, data.window_size.1 - 1),
            Print("Title: "),
            Print(&self.title)
        )?;
        Ok(())
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

#[derive(Debug)]
enum UserInput {
    NewTask,
}

#[derive(Debug, Default)]
struct Tasker {
    tasklist: TaskList,
    state: AppState,
    pane: Pane,
    data: AppData,
}

impl Tasker {
    fn handle_key(&mut self, key: KeyEvent) -> CResult<()> {
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
                            if self.tasklist.selection < self.tasklist.tasks.len() - 1 {
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
                    self.state = AppState::Normal
                }
            }
        }

        Ok(())
    }

    fn show(&self, stdout: &mut Stdout) -> CResult<()> {
        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
        match self.pane {
            Pane::Main => {
                self.tasklist.show(stdout, &self.data)?;
            }
            Pane::OneTask(id) => {
                let list = TaskList {
                    tasks: vec![id],
                    selection: 1,
                };
                list.show(stdout, &self.data)?;
            }
        }

        if let AppState::Input(input) = &self.state {
            queue!(stdout, cursor::Show)?;
            input.show(stdout, &self.data)?;
        } else {
            queue!(stdout, cursor::Hide)?;
        }

        stdout.flush()?;

        Ok(())
    }
}

fn event_loop() -> CResult<()> {
    let mut tasker = Tasker::default();
    tasker.data.window_size = terminal::size()?;
    loop {
        tasker.show(&mut stdout())?;
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
                    tasker.handle_key(key)?;
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

    if let Err(e) = event_loop() {
        println!("Error: {:?}\r", e);
    }

    execute!(stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
    disable_raw_mode()
}
