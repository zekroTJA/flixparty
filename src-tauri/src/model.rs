use serde::Serialize;
use std::fmt;

#[derive(Serialize, Debug)]
pub struct Error {
    pub message: String,
}

impl<T> From<T> for Error
where
    T: fmt::Display,
{
    fn from(value: T) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}
