use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use unicode_segmentation::UnicodeSegmentation;

use crate::task::{self, Filter, TaskId};
use crate::AppData;

fn status_to_span(status: task::Status) -> Span<'static> {
    match status {
        task::Status::Todo => Span::styled("TODO", Style::default().add_modifier(Modifier::BOLD)),
        task::Status::Done => Span::styled("DONE", Style::default().add_modifier(Modifier::DIM)),
    }
}

#[derive(Debug, Default)]
pub(crate) struct TaskList {
    pub(crate) tasks: Vec<TaskId>,
    pub(crate) selection: usize,
    pub(crate) list_state: ListState,
    pub(crate) title: Option<String>,
}

impl TaskList {
    pub(crate) fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub(crate) fn selection(&self) -> Option<TaskId> {
        self.tasks.get(self.selection).copied()
    }

    pub(crate) fn apply_filter(&mut self, data: &AppData, filter: &Filter) {
        self.tasks = filter.apply(&data.store);
        self.selection = 0;
        self.list_state = Default::default();
    }

    pub(crate) fn show<'a>(
        &mut self,
        data: &'a AppData,
        frame: &mut Frame<impl Backend>,
        size: Rect,
    ) {
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
pub(crate) struct TaskView {
    pub(crate) task_id: TaskId,
    pub(crate) link_list: TaskList,
    pub(crate) show_full: bool,
}

impl TaskView {
    pub(crate) fn new(task_id: TaskId, data: &AppData, show_full: bool) -> Self {
        let task = data.store.get_task(task_id);
        let mut link_list = TaskList::default().title("Linked tasks");
        link_list.tasks = task.links.clone();
        Self {
            task_id,
            link_list,
            show_full,
        }
    }

    pub(crate) fn show(&mut self, data: &AppData, frame: &mut Frame<impl Backend>, size: Rect) {
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
pub(crate) struct QuickInput {
    pub(crate) title: String,
    pub(crate) text: String,
}

impl QuickInput {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            text: String::new(),
        }
    }

    pub(crate) fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    pub(crate) fn show(&self, _data: &AppData) -> (Paragraph, u16) {
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
pub(crate) struct QuickSelect {
    pub(crate) title: String,
    pub(crate) choices: Vec<(char, String)>,
}

impl QuickSelect {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            choices: Vec::new(),
        }
    }

    pub(crate) fn choices(
        mut self,
        choices: impl IntoIterator<Item = (char, impl Into<String>)>,
    ) -> Self {
        self.choices
            .extend(choices.into_iter().map(|x| (x.0, x.1.into())));
        self
    }

    pub(crate) fn show(&self, _data: &AppData) -> Paragraph {
        let mut spans = vec![Span::from(self.title.as_str()), Span::from(": ")];
        for (key, text) in &self.choices {
            spans.push(Span::raw(format!("[{}] {} ", key, text)));
        }
        Paragraph::new(vec![Spans::from(spans)])
    }
}
