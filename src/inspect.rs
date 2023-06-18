use std::fmt::Debug;

pub trait Inspect {
    fn inspect(self) -> Self;
}

impl<T: Debug> Inspect for T {
    fn inspect(self) -> Self {
        dbg!(&self);
        self
    }
}
