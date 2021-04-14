#![allow(dead_code)]
use std::any::Any;

pub struct Machine<A, D> {
    state: Box<dyn ErasedState<Action = A, Data = D>>,
    stack: Vec<Box<dyn ErasedState<Action = A, Data = D>>>,
}

enum PrivilegedActResult<A, D> {
    To(Box<dyn ErasedState<Action = A, Data = D>>),
    Replace(Box<dyn ErasedState<Action = A, Data = D>>),
    Push(Box<dyn ErasedState<Action = A, Data = D>>),
    Return(Box<dyn Any>),
    Yield(Box<dyn Any>),
}

pub struct PrivActResult<A, D>(PrivilegedActResult<A, D>);

#[must_use]
pub enum ActResult<A, D> {
    Priv(PrivActResult<A, D>),
    Nothing,
}

impl<A, D> From<PrivilegedActResult<A, D>> for ActResult<A, D> {
    fn from(x: PrivilegedActResult<A, D>) -> Self {
        ActResult::Priv(PrivActResult(x))
    }
}

trait ErasedState {
    type Action;
    type Data;
    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data>;
    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Box<dyn Any>,
    ) -> ActResult<Self::Action, Self::Data>;
    fn on_yield(
        &mut self,
        data: &mut Self::Data,
        value: Box<dyn Any>,
    ) -> ActResult<Self::Action, Self::Data>;
    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data>;
    fn on_exit(&mut self, data: &mut Self::Data);
}

pub trait State {
    type Action;
    type Data;
    type Input: 'static;
    type Return: 'static;

    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data>;

    fn resume(
        &mut self,
        _data: &mut Self::Data,
        _value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        ActResult::Nothing
    }

    fn on_yield(
        &mut self,
        _data: &mut Self::Data,
        _value: Self::Input,
    ) -> ActResult<Self::Action, Self::Data> {
        ActResult::Nothing
    }

    fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        ActResult::Nothing
    }

    fn on_exit(&mut self, _data: &mut Self::Data) {}
}

pub trait StateTools: State {
    fn transition<I, R>(
        &self,
        state: impl State<Action = Self::Action, Data = Self::Data, Input = I, Return = R> + 'static,
    ) -> ActResult<Self::Action, Self::Data> {
        PrivilegedActResult::To(Box::new(state) as _).into()
    }

    fn replace<I>(
        &self,
        state: impl State<Action = Self::Action, Data = Self::Data, Input = I, Return = Self::Return>
            + 'static,
    ) -> ActResult<Self::Action, Self::Data> {
        PrivilegedActResult::Replace(Box::new(state) as _).into()
    }

    fn push<I>(
        &self,
        state: impl State<Action = Self::Action, Data = Self::Data, Input = I, Return = Self::Input>
            + 'static,
    ) -> ActResult<Self::Action, Self::Data> {
        PrivilegedActResult::Push(Box::new(state) as _).into()
    }

    fn do_yield(&self, value: Self::Return) -> ActResult<Self::Action, Self::Data> {
        PrivilegedActResult::Yield(Box::new(value) as _).into()
    }

    fn pop(&self, value: Self::Return) -> ActResult<Self::Action, Self::Data> {
        PrivilegedActResult::Return(Box::new(value) as _).into()
    }
}

impl<T> StateTools for T where T: State {}

impl<T> ErasedState for T
where
    T: State,
{
    type Action = T::Action;
    type Data = T::Data;
    fn act(
        &mut self,
        data: &mut Self::Data,
        action: Self::Action,
    ) -> ActResult<Self::Action, Self::Data> {
        State::act(self, data, action)
    }

    fn resume(
        &mut self,
        data: &mut Self::Data,
        value: Box<dyn Any>,
    ) -> ActResult<Self::Action, Self::Data> {
        let value = value
            .downcast::<T::Input>()
            .expect("State resumed with wrong input type");
        State::resume(self, data, *value)
    }

    fn on_yield(
        &mut self,
        data: &mut Self::Data,
        value: Box<dyn Any>,
    ) -> ActResult<Self::Action, Self::Data> {
        let value = value
            .downcast::<T::Input>()
            .expect("State yielded to with wrong input type");
        State::on_yield(self, data, *value)
    }

    fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
        State::on_enter(self, data)
    }

    fn on_exit(&mut self, data: &mut Self::Data) {
        State::on_exit(self, data)
    }
}

impl<A, D> Machine<A, D> {
    pub fn new(state: impl State<Action = A, Data = D> + 'static) -> Self {
        Self {
            state: Box::new(state) as _,
            stack: Vec::new(),
        }
    }

    pub fn act(&mut self, data: &mut D, action: A) {
        let result = self.state.act(data, action);
        self.apply_result(data, result, self.stack.len());
    }

    fn apply_result(&mut self, data: &mut D, result: ActResult<A, D>, stack_pos: usize) {
        match result {
            ActResult::Priv(PrivActResult(PrivilegedActResult::To(state))) => {
                self.state.on_exit(data);
                for mut state in self.stack.drain(..).rev() {
                    state.on_exit(data);
                }
                self.state = state;
                let result = self.state.on_enter(data);
                self.apply_result(data, result, 0);
            }
            ActResult::Priv(PrivActResult(PrivilegedActResult::Replace(state))) => {
                // TODO: Make this forbidden if not on top of stack
                self.state.on_exit(data);
                self.state = state;
                let result = self.state.on_enter(data);
                self.apply_result(data, result, stack_pos);
            }
            ActResult::Priv(PrivActResult(PrivilegedActResult::Push(state))) => {
                // TODO: Make this forbidden if not on top of stack
                let old = std::mem::replace(&mut self.state, state);
                self.stack.push(old);
                let result = self.state.on_enter(data);
                self.apply_result(data, result, self.stack.len());
            }
            ActResult::Priv(PrivActResult(PrivilegedActResult::Return(value))) => {
                // TODO: Make this forbidden if not on top of stack
                self.state.on_exit(data);
                self.state = self.stack.pop().expect("Returned on empty stack");
                let result = self.state.resume(data, value);
                self.apply_result(data, result, stack_pos - 1);
            }
            ActResult::Priv(PrivActResult(PrivilegedActResult::Yield(value))) => {
                if stack_pos == 0 {
                    panic!("Yielded on bottom of stack");
                }
                let state = &mut self.stack[stack_pos - 1];
                let result = state.on_yield(data, value);
                self.apply_result(data, result, stack_pos - 1);
            }
            ActResult::Nothing => {}
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct Begin;
    struct GetValue;
    struct End(i32);
    enum Action {
        Begin,
        Set(i32),
    }

    struct Data {
        value: i32,
    }

    impl State for Begin {
        type Action = Action;
        type Data = Data;
        type Input = i32;
        type Return = ();

        fn act(
            &mut self,
            _data: &mut Self::Data,
            action: Self::Action,
        ) -> ActResult<Self::Action, Self::Data> {
            if let Action::Begin = action {
                return self.push(GetValue);
            }
            ActResult::Nothing
        }

        fn resume(
            &mut self,
            _data: &mut Self::Data,
            value: Self::Input,
        ) -> ActResult<Self::Action, Self::Data> {
            println!("Resume Begin");
            self.transition(End(value))
        }

        fn on_exit(&mut self, _data: &mut Self::Data) {
            println!("Exit Begin");
        }
    }

    impl State for GetValue {
        type Action = Action;
        type Data = Data;
        type Input = ();
        type Return = i32;

        fn act(
            &mut self,
            _data: &mut Self::Data,
            action: Self::Action,
        ) -> ActResult<Self::Action, Self::Data> {
            if let Action::Set(v) = action {
                return self.pop(v);
            }
            ActResult::Nothing
        }

        fn on_enter(&mut self, _data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
            println!("Enter GetValue");
            ActResult::Nothing
        }

        fn on_exit(&mut self, _data: &mut Self::Data) {
            println!("Exit GetValue");
        }
    }

    impl State for End {
        type Action = Action;
        type Data = Data;
        type Input = ();
        type Return = ();

        fn act(
            &mut self,
            _data: &mut Self::Data,
            _action: Self::Action,
        ) -> ActResult<Self::Action, Self::Data> {
            ActResult::Nothing
        }

        fn on_enter(&mut self, data: &mut Self::Data) -> ActResult<Self::Action, Self::Data> {
            println!("Enter End");
            data.value = self.0;
            ActResult::Nothing
        }
    }

    #[test]
    fn test_machine() {
        let mut machine = Machine::new(Begin);
        let mut data = Data { value: 0 };

        machine.act(&mut data, Action::Begin);
        machine.act(&mut data, Action::Set(10));

        assert_eq!(data.value, 10);
    }
}
