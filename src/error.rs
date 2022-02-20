use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct RaincoatError {
    pub cause: String
}

impl std::error::Error for RaincoatError {}

impl Display for RaincoatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cause: {}", self.cause)
    }
}
