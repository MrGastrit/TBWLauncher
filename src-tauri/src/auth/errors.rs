use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AuthError {
    Validation(String),
    Internal(String),
}

impl Display for AuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AuthError {}
