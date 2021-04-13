use std::collections::HashMap;

/// A correct-by-construction id for tasks. Can not be constructed for non-existing tasks.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TaskId(u64);

impl TaskId {
    pub fn id(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status {
    Todo,
    Done,
}

impl Default for Status {
    fn default() -> Self {
        Status::Todo
    }
}

#[derive(Debug)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub status: Status,
}

impl Task {
    pub fn toggle_status(&mut self) -> Status {
        self.status = match self.status {
            Status::Todo => Status::Done,
            Status::Done => Status::Todo,
        };
        self.status
    }
}

#[derive(Debug, Default)]
pub struct TaskStore {
    tasks: HashMap<TaskId, Task>,
    id_counter: u64,
}

impl TaskStore {
    pub fn new_task(&mut self) -> &mut Task {
        self.id_counter += 1;
        let id = TaskId(self.id_counter);
        let task = Task {
            id,
            title: String::new(),
            description: String::new(),
            status: Status::default(),
        };
        self.tasks.insert(id, task);
        self.tasks.get_mut(&id).unwrap()
    }

    pub fn get_task(&self, id: TaskId) -> &Task {
        self.tasks.get(&id).expect("Task doesn't exist")
    }

    pub fn get_task_mut(&mut self, id: TaskId) -> &mut Task {
        self.tasks.get_mut(&id).expect("Task doesn't exist")
    }
}
