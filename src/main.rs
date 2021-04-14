mod automaton;
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

use automaton::*;
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
    title: Option<String>,
}

impl TaskList {
    fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn selection(&self) -> Option<TaskId> {
        self.tasks.get(self.selection).copied()
    }

    fn apply_filter(&mut self, data: &AppData, filter: &Filter) {
        self.tasks = filter.apply(&data.store);
        self.selection = 0;
        self.list_state = Default::default();
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
        let block = Block::default()
            .borders(Borders::TOP)
            .title(format!(" {} ", self.title.as_deref().unwrap_or("Tasks")));
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
    link_list: TaskList,
    show_full: bool,
}

impl TaskView {
    fn new(task_id: TaskId, data: &AppData, show_full: bool) -> Self {
        let task = data.store.get_task(task_id);
        let mut link_list = TaskList::default().title("Linked tasks");
        link_list.tasks = task.links.clone();
        Self {
            task_id,
            link_list,
            show_full,
        }
    }

    fn show(&mut self, data: &AppData, frame: &mut Frame<impl Backend>, size: Rect) {
        let task = data.store.get_task(self.task_id);
        let block = Block::default()
            .borders(Borders::TOP)
            .title(Spans::from(vec![
                Span::from(" "),
                Span::from(task.title.as_str()),
                Span::from(" "),
            ]));
        frame.render_widget(block, size);

        let h_constraints = if self.show_full {
            vec![Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]
        } else {
            vec![Constraint::Min(0)]
        };

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(h_constraints)
            .split(size);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(horizontal[0]);

        let text = vec![Spans::from(vec![
            Span::from("Status: "),
            status_to_span(task.status),
        ])];
        let text = Paragraph::new(text);
        frame.render_widget(text, chunks[0]);

        let description = Text::raw(task.description.as_str());
        let paragraph = Paragraph::new(description).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, chunks[1]);

        if self.show_full {
            self.link_list.show(data, frame, horizontal[1]);
        }
    }
}

#[derive(Debug, Default)]
struct QuickInput {
    title: String,
    text: String,
}

impl QuickInput {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            text: String::new(),
        }
    }

    fn text(mut self, text: String) -> Self {
        self.text = text;
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
        Paragraph::new(vec![Spans::from(spans)])
    }
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

enum Action {
    Key(KeyEvent),
}

struct NormalState;

impl State for NormalState {
    type Action = Action;
    type Data = Tasker;
    type Input = ();
    type Return = ();

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        let Action::Key(key) = action;
        match key.code {
            KeyCode::Char('n') => {
                return self.push(QuickCreateState);
            }
            KeyCode::Char('f') => {
                return self.push(SetFilterState);
            }
            KeyCode::Up => {
                data.tasklist.selection = data.tasklist.selection.saturating_sub(1);
            }
            KeyCode::Down => {
                if data.tasklist.selection + 1 < data.tasklist.tasks.len() {
                    data.tasklist.selection += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(id) = data.tasklist.selection() {
                    return self.transition(OneTaskState(id));
                }
            }
            KeyCode::Char(' ') => {
                if let Some(id) = data.tasklist.selection() {
                    let task = data.data.store.get_task_mut(id);
                    task.toggle_status();
                }
            }
            KeyCode::Char('m') => {
                let task = data.data.store.new_task();
                task.title = task.id.id().to_string();
                data.tasklist.tasks.push(task.id);
                data.tasklist.selection = data.tasklist.tasks.len() - 1;
            }
            KeyCode::Char('e') => {
                if let Some(id) = data.tasklist.selection() {
                    return self.push(SetDescriptionState(id));
                }
            }
            _ => {}
        }
        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        data.pane = Pane::Main;
        ActResult::Nothing
    }
}

struct OneTaskState(TaskId);

impl State for OneTaskState {
    type Action = Action;
    type Data = Tasker;
    type Input = ();
    type Return = ();

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        let view = match &mut data.pane {
            Pane::OneTask(view) => view,
            _ => panic!("Wrong pane"),
        };

        let Action::Key(key) = action;

        match key.code {
            KeyCode::Esc => {
                return self.transition(NormalState);
            }
            KeyCode::Char('n') => {
                return self.push(QuickCreateState);
            }
            KeyCode::Up => {
                view.link_list.selection = view.link_list.selection.saturating_sub(1);
            }
            KeyCode::Down => {
                if view.link_list.selection + 1 < view.link_list.tasks.len() {
                    view.link_list.selection += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(id) = view.link_list.selection() {
                    return self.transition(OneTaskState(id));
                }
            }
            KeyCode::Char(' ') => {
                let task = data.data.store.get_task_mut(view.task_id);
                task.toggle_status();
            }
            KeyCode::Char('l') => {
                return self.push(AddLinkState(self.0));
            }
            KeyCode::Char('e') => {
                return self.push(SetDescriptionState(self.0));
            }
            _ => {}
        }

        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        data.pane = Pane::OneTask(TaskView::new(self.0, &data.data, true));
        ActResult::Nothing
    }
}

struct AddLinkState(TaskId);

impl State for AddLinkState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<TaskId>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("AddLinkState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        if let Some(oid) = value {
            let id = self.0;
            let task = data.data.store.get_task_mut(id);
            task.links.push(oid);
            let other_task = data.data.store.get_task_mut(oid);
            other_task.links.push(id);

            let view = match &mut data.pane {
                Pane::OneTask(view) => view,
                _ => panic!("Wrong pane"),
            };
            view.link_list.tasks.push(oid);
        }

        self.pop(())
    }

    fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        self.push(SearchTaskState {
            title: "Link a task".to_owned(),
        })
    }
}

struct SearchTaskState {
    title: String,
}

impl State for SearchTaskState {
    type Action = Action;
    type Data = Tasker;
    type Input = ();
    type Return = Option<TaskId>;

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        let Action::Key(key) = action;

        let input = data.quick_input.as_mut().unwrap();
        let search = &mut data.search.as_mut().unwrap();
        let list = &mut search.list;

        let mut send = false;
        if let KeyCode::Char(c) = key.code {
            input.text.push(c);
            send = true;
        }
        if key.code == KeyCode::Backspace {
            input.text.pop();
            send = true;
        }

        if send {
            search.filter.title = input.text.clone();
            list.apply_filter(&data.data, &search.filter);
        }

        if key.code == KeyCode::Enter {
            return self.pop(list.selection());
        }

        if key.code == KeyCode::Up {
            list.selection = list.selection.saturating_sub(1);
        }
        if key.code == KeyCode::Down && list.selection + 1 < list.tasks.len() {
            list.selection += 1;
        }

        if key.code == KeyCode::Esc {
            return self.pop(None);
        }

        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        let mut list = TaskList::default().title(&self.title);
        list.apply_filter(&data.data, &Filter::default());
        data.quick_input = Some(QuickInput::new("Search"));
        data.search = Some(Search {
            filter: Filter::default(),
            list,
        });
        ActResult::Nothing
    }

    fn on_exit(&mut self, data: &mut Self::Data) {
        data.quick_input = None;
        data.search = None;
    }
}

struct QuickCreateState;

impl State for QuickCreateState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<String>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("QuickCreateState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        if let Some(text) = value {
            let task = data.data.store.new_task();
            task.title = text;
            data.tasklist.tasks.push(task.id);
            data.tasklist.selection = data.tasklist.tasks.len() - 1;
        }

        self.pop(())
    }

    fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        self.push(QuickInputState::new("Title"))
    }
}

struct SetDescriptionState(TaskId);

impl State for SetDescriptionState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<String>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("SetDescriptionState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        if let Some(text) = value {
            let id = self.0;
            let task = data.data.store.get_task_mut(id);
            task.description = text;
        }

        self.pop(())
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        let id = self.0;
        let task = data.data.store.get_task_mut(id);
        self.push(QuickInputState::new("Description").text(task.description.clone()))
    }
}

struct SetFilterState;

impl State for SetFilterState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<String>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("SetFilterState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        if let Some(text) = value {
            if text == "Title" {
                return self.replace(SetFilterTitleState);
            }
            if text == "Todo" {
                data.filter.status = Some(task::Status::Todo);
            }
            if text == "Done" {
                data.filter.status = Some(task::Status::Done);
            }
            if text == "Clear" {
                data.filter = Filter::default();
            }
            data.tasklist.apply_filter(&data.data, &data.filter);
        }

        self.pop(())
    }

    fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        self.push(QuickSelectState::new(
            "Filter".into(),
            vec![('t', "Title"), ('d', "Todo"), ('D', "Done"), ('c', "Clear")],
        ))
    }
}

struct SetFilterTitleState;

impl State for SetFilterTitleState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<String>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("SetFilterTitleState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        _data: &mut Self::Data,
        _value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        self.pop(())
    }

    fn on_yield(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        if let Some(text) = value {
            data.filter.title = text;
            data.tasklist.apply_filter(&data.data, &data.filter);
        }

        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        self.push(
            QuickInputState::new("Filter [Title]")
                .text(data.filter.title.clone())
                .continuous(true),
        )
    }
}

#[derive(Debug, Default)]
struct QuickInputState {
    title: String,
    continuous: bool,
    text: String,
}

impl QuickInputState {
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

    fn continuous(mut self, v: bool) -> Self {
        self.continuous = v;
        self
    }
}

impl State for QuickInputState {
    type Action = Action;
    type Data = Tasker;
    type Input = ();
    type Return = Option<String>;

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        let Action::Key(key) = action;

        let input = data.quick_input.as_mut().unwrap();

        let mut send = false;
        if let KeyCode::Char(c) = key.code {
            input.text.push(c);
            send = true;
        }
        if key.code == KeyCode::Backspace {
            input.text.pop();
            send = true;
        }

        if send && self.continuous {
            return self.do_yield(Some(input.text.clone()));
        } else if key.code == KeyCode::Enter {
            return self.pop(Some(input.text.clone()));
        }

        if key.code == KeyCode::Esc {
            return self.pop(None);
        }

        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        data.quick_input = Some(QuickInput::new(&self.title).text(self.text.clone()));
        ActResult::Nothing
    }

    fn on_exit(&mut self, data: &mut Self::Data) {
        data.quick_input = None;
    }
}

#[derive(Debug, Default)]
struct QuickSelectState {
    title: String,
    choices: Vec<(char, String)>,
}

impl QuickSelectState {
    fn new(title: String, choices: impl IntoIterator<Item = (char, impl Into<String>)>) -> Self {
        Self {
            title,
            choices: choices.into_iter().map(|x| (x.0, x.1.into())).collect(),
        }
    }
}

impl State for QuickSelectState {
    type Action = Action;
    type Data = Tasker;
    type Input = ();
    type Return = Option<String>;

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        let Action::Key(key) = action;

        let input = data.quick_select.as_mut().unwrap();

        if let KeyCode::Char(c) = key.code {
            if let Some(choice) = input.choices.iter().find(|x| x.0 == c) {
                return self.pop(Some(choice.1.clone()));
            }
        }

        if key.code == KeyCode::Esc {
            return self.pop(None);
        }

        ActResult::Nothing
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        data.quick_select = Some(QuickSelect::new(&self.title).choices(self.choices.clone()));

        ActResult::Nothing
    }
    fn on_exit(&mut self, data: &mut Self::Data) {
        data.quick_select = None;
    }
}

#[derive(Debug, Default)]
struct Tasker {
    tasklist: TaskList,
    quick_input: Option<QuickInput>,
    quick_select: Option<QuickSelect>,
    search: Option<Search>,
    pane: Pane,
    data: AppData,
    filter: Filter,
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
