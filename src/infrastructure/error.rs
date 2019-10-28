use crate::core::error::{Error as BError, RepoError};
use diesel::r2d2;
use diesel_migrations::RunMigrationsError;
use serde_json;
use std::{error, io};

use diesel::result::Error as DieselError;

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> AppError {
        AppError::Business(BError::Repo(err))
    }
}

impl From<DieselError> for RepoError {
    fn from(err: DieselError) -> RepoError {
        match err {
            DieselError::NotFound => RepoError::NotFound,
            _ => RepoError::Other(Box::new(err)),
        }
    }
}

impl From<RunMigrationsError> for AppError {
    fn from(err: RunMigrationsError) -> AppError {
        AppError::Other(Box::new(err))
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum AppError {
        Business(err: BError){
            from()
            cause(err)
            description(err.description())
        }
        Serialize(err: serde_json::Error){
            from()
            cause(err)
            description(err.description())
        }
        Other(err: Box<dyn error::Error + Send + Sync>){
            from()
            description(err.description())
            from(err: io::Error) -> (Box::new(err))
        }
        R2d2(err: r2d2::PoolError){
            from()
        }
        CsvIntoInner(err: ::csv::IntoInnerError<::csv::Writer<::std::vec::Vec<u8>>>){
            from()
        }
        String(err: ::std::string::FromUtf8Error){
            from()
        }
        Str(err: ::std::str::Utf8Error){
            from()
        }
        Csv(err: ::csv::Error){
            from()
        }
    }
}

impl From<failure::Error> for AppError {
    fn from(from: failure::Error) -> Self {
        AppError::Other(Box::new(from.compat()))
    }
}
