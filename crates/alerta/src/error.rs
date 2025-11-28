use std::fmt;

/// The main error type of this library.
pub struct Error(Box<dyn std::error::Error + Send + Sync>);

impl Error {
    pub(crate) fn new(inner: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(inner.into())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub(crate) fn err(inner: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(inner)
}
