use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Parse(String),
    Usage(String),
    MongoDb(mongodb::error::Error),
    Bson(bson::ser::Error),
    Json(serde_json::Error),
    Json5(json5::Error),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(v) => write!(f, "parser: {}", v),
            Error::Usage(v) => write!(f, "{}", v),
            Error::MongoDb(v) => write!(f, "{}", v),
            Error::Bson(v) => write!(f, "bson: {}", v),
            Error::Json(v) => write!(f, "json: {}", v),
            Error::Json5(v) => write!(f, "json5: {}", v),
            Error::Io(v) => write!(f, "io: {}", v),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(v: String) -> Self {
        Error::Parse(v)
    }
}

impl From<mongodb::error::Error> for Error {
    fn from(v: mongodb::error::Error) -> Self {
        Error::MongoDb(v)
    }
}

// impl From<bson::de::Error> for Error {
//     fn from(v: bson::de::Error) -> Self {
//         Error::BsonDe(v)
//     }
// }

impl From<bson::ser::Error> for Error {
    fn from(v: bson::ser::Error) -> Self {
        Error::Bson(v)
    }
}

impl From<serde_json::Error> for Error {
    fn from(v: serde_json::Error) -> Self {
        Error::Json(v)
    }
}

impl From<json5::Error> for Error {
    fn from(v: json5::Error) -> Self {
        Error::Json5(v)
    }
}

impl From<io::Error> for Error {
    fn from(v: io::Error) -> Self {
        Error::Io(v)
    }
}
