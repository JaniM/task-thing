use crate::automaton::*;
use crate::task::{Filter, TaskId};
use crate::{Action, QuickInput, QuickSelect, Search, TaskList, Tasker};
use crossterm::event::KeyCode;

pub(crate) struct SearchTaskState {
    pub(crate) title: String,
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

#[derive(Debug, Default)]
pub(crate) struct QuickInputState {
    pub(crate) title: String,
    pub(crate) continuous: bool,
    pub(crate) text: String,
}

impl QuickInputState {
    pub(crate) fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            text: String::new(),
            continuous: false,
        }
    }

    pub(crate) fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    pub(crate) fn continuous(mut self, v: bool) -> Self {
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
pub(crate) struct QuickSelectState {
    pub(crate) title: String,
    pub(crate) choices: Vec<(char, String)>,
}

impl QuickSelectState {
    pub(crate) fn new(
        title: String,
        choices: impl IntoIterator<Item = (char, impl Into<String>)>,
    ) -> Self {
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
