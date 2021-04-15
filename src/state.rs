pub(crate) mod dialogs;

use crate::{
    automaton::*,
    components::{TaskView, Timer},
    task::{self, Filter, TaskId},
    Action, Pane, Tasker,
};
use crossterm::event::KeyCode;

use dialogs::*;

pub(crate) struct NormalState;

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
            KeyCode::Char('p') => {
                if let Some(id) = data.tasklist.selection() {
                    return self.push(SetPomodoroState(id));
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

pub(crate) struct OneTaskState(TaskId);

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

pub(crate) struct AddLinkState(TaskId);

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

pub(crate) struct QuickCreateState;

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

pub(crate) struct SetDescriptionState(TaskId);

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

pub(crate) struct SetFilterState;

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

pub(crate) struct SetFilterTitleState;

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

pub(crate) struct SetPomodoroState(TaskId);

impl State for SetPomodoroState {
    type Action = Action;
    type Data = Tasker;
    type Input = Option<String>;
    type Return = ();

    fn act(
        &mut self,
        _data: &mut Self::Data,
        _action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        panic!("SetPomodoroState shouldn't receive actions");
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        let id = self.0;
        if let Some(text) = value {
            if text == "Start" {
                data.timer = Some(Timer::trigger_in(
                    "WORK",
                    std::time::Duration::from_secs(60 * 25),
                    move |data| {
                        let task = data.store.get_task_mut(id);
                        task.pomodoros += 1;
                    },
                ));
            }
            if text == "Break 5m" {
                data.timer = Some(Timer::trigger_in(
                    "BREAK",
                    std::time::Duration::from_secs(60 * 5),
                    |_| {},
                ));
            }
            if text == "Break 10m" {
                data.timer = Some(Timer::trigger_in(
                    "BREAK",
                    std::time::Duration::from_secs(60 * 10),
                    |_| {},
                ));
            }
            if text == "Test" {
                data.timer = Some(Timer::trigger_in(
                    "TEST",
                    std::time::Duration::from_secs(5),
                    |_| {},
                ));
            }
            if text == "Clear" {
                data.timer = None;
            }
            data.tasklist.apply_filter(&data.data, &data.filter);
        }

        self.pop(())
    }

    fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        self.push(QuickSelectState::new(
            "Pomodoro".into(),
            vec![
                ('p', "Start"),
                ('b', "Break 5m"),
                ('B', "Break 10m"),
                ('t', "Test"),
                ('c', "Clear"),
            ],
        ))
    }
}
