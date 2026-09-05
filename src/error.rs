use std::fmt;
use std::io;

#[derive(Debug)]
pub enum IoError {
    Io(io::Error),
    Csv(csv::Error),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Csv(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
        }
    }
}

impl From<io::Error> for IoError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for IoError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}
