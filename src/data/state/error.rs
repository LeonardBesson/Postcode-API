use std::fmt::Formatter;

#[derive(Debug)]
pub enum RefreshError {
    IO(Box<dyn std::fmt::Debug + Send>),
    InvalidZip(Box<zip::result::ZipError>),
    InvalidData(Box<dyn std::fmt::Debug + Send>),
    FileNotFound
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let msg = match self {
            RefreshError::IO(inner) => {
                format!("IO: {:?}", inner)
            },
            RefreshError::InvalidZip(inner) => {
                format!("Invalid zip data file: {}", inner)
            },
            RefreshError::InvalidData(inner) => {
                format!("Invalid address data: {:?}", inner)
            },
            RefreshError::FileNotFound => {
                "Could not find data file".into()
            }
        };
        write!(f, "Refresh error: {}", msg)
    }
}

impl From<diesel::result::Error> for RefreshError {
    fn from(error: diesel::result::Error) -> Self {
        RefreshError::IO(Box::new(error))
    }
}

impl <E> From<actix_web::error::BlockingError<E>> for RefreshError
    where
        E: std::fmt::Debug + Send + 'static,
{
    fn from(error: actix_web::error::BlockingError<E>) -> Self {
        match error {
            actix_web::error::BlockingError::Error(inner) =>
                RefreshError::IO(Box::new(inner)),
            _ =>
                RefreshError::IO(Box::new(error)),
        }
    }
}

impl From<reqwest::Error> for RefreshError {
    fn from(error: reqwest::Error) -> Self {
        RefreshError::IO(Box::new(error))
    }
}

impl From<zip::result::ZipError> for RefreshError {
    fn from(error: zip::result::ZipError) -> Self {
        RefreshError::InvalidZip(Box::new(error))
    }
}

impl From<csv::Error> for RefreshError {
    fn from(error: csv::Error) -> Self {
        RefreshError::InvalidData(Box::new(error))
    }
}
