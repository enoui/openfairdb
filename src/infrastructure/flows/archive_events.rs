use super::*;

use diesel::connection::Connection;

pub fn archive_events(connections: &sqlite::Connections, ids: &[&str]) -> Result<()> {
    let mut repo_err = None;
    let connection = connections.exclusive()?;
    Ok(connection
        .transaction::<_, diesel::result::Error, _>(|| {
            usecases::archive_events(&*connection, ids).map_err(|err| {
                warn!("Failed to archive {} events: {}", ids.len(), err);
                repo_err = Some(err);
                diesel::result::Error::RollbackTransaction
            })
        })
        .map_err(|err| {
            if let Some(repo_err) = repo_err {
                repo_err
            } else {
                RepoError::from(err).into()
            }
        })?)
}
