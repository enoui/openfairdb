use super::*;

use diesel::connection::Connection;

pub fn exec_archive_ratings(
    connections: &sqlite::Connections,
    account_email: &str,
    ids: &[&str],
) -> Result<()> {
    //TODO: check if user is allowed to archive the ratings
    let mut repo_err = None;
    let connection = connections.exclusive()?;
    Ok(connection
        .transaction::<_, diesel::result::Error, _>(|| {
            usecases::archive_ratings(&*connection, account_email, ids).map_err(|err| {
                warn!("Failed to archive {} ratings: {}", ids.len(), err);
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

pub fn post_archive_ratings(
    connections: &sqlite::Connections,
    indexer: &mut dyn PlaceIndexer,
    ids: &[&str],
) -> Result<()> {
    let connection = connections.shared()?;
    let place_uids = connection.load_place_uids_of_ratings(ids)?;
    for place_uid in place_uids {
        let (entry, _) = match connection.get_place(&place_uid) {
            Ok(entry) => entry,
            Err(err) => {
                error!(
                    "Failed to load entry {} for reindexing after archiving ratings: {}",
                    place_uid, err
                );
                // Skip entry
                continue;
            }
        };
        let ratings = match connection.load_ratings_of_entry(entry.uid.as_ref()) {
            Ok(ratings) => ratings,
            Err(err) => {
                error!(
                    "Failed to load ratings for entry {} for reindexing after archiving ratings: {}",
                    entry.uid, err
                );
                // Skip entry
                continue;
            }
        };
        if let Err(err) = usecases::index_entry(indexer, &entry, &ratings) {
            error!(
                "Failed to reindex entry {} after archiving ratings: {}",
                entry.uid, err
            );
        }
    }
    if let Err(err) = indexer.flush() {
        error!(
            "Failed to finish updating the search index after archiving ratings: {}",
            err
        );
    }
    Ok(())
}

pub fn archive_ratings(
    connections: &sqlite::Connections,
    indexer: &mut dyn PlaceIndexer,
    account_email: &str,
    ids: &[&str],
) -> Result<()> {
    exec_archive_ratings(connections, account_email, ids)?;
    post_archive_ratings(connections, indexer, ids)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::prelude::*;

    fn archive_ratings(fixture: &EnvFixture, ids: &[&str]) -> super::Result<()> {
        super::archive_ratings(
            &fixture.db_connections,
            &mut *fixture.search_engine.borrow_mut(),
            "scout@foo.tld",
            ids,
        )
    }

    #[test]
    fn should_archive_multiple_ratings_only_once() {
        let fixture = EnvFixture::new();

        fixture.create_user(
            usecases::NewUser {
                email: "scout@foo.tld".into(),
                password: "123456".into(),
            },
            Some(Role::Scout),
        );

        let place_uids = vec![
            fixture.create_place(0.into(), None),
            fixture.create_place(1.into(), None),
        ];
        let rating_comment_ids = vec![
            fixture.create_rating(new_entry_rating(
                0,
                &place_uids[0],
                RatingContext::Diversity,
                RatingValue::new(-1),
            )),
            fixture.create_rating(new_entry_rating(
                1,
                &place_uids[0],
                RatingContext::Fairness,
                RatingValue::new(0),
            )),
            fixture.create_rating(new_entry_rating(
                2,
                &place_uids[1],
                RatingContext::Transparency,
                RatingValue::new(1),
            )),
            fixture.create_rating(new_entry_rating(
                3,
                &place_uids[1],
                RatingContext::Renewable,
                RatingValue::new(2),
            )),
        ];

        assert!(fixture.entry_exists(&place_uids[0]));
        assert!(fixture.entry_exists(&place_uids[1]));

        assert!(fixture.rating_exists(&rating_comment_ids[0].0));
        assert!(fixture.rating_exists(&rating_comment_ids[1].0));
        assert!(fixture.rating_exists(&rating_comment_ids[2].0));
        assert!(fixture.rating_exists(&rating_comment_ids[3].0));

        assert!(fixture.comment_exists(&rating_comment_ids[0].1));
        assert!(fixture.comment_exists(&rating_comment_ids[1].1));
        assert!(fixture.comment_exists(&rating_comment_ids[2].1));
        assert!(fixture.comment_exists(&rating_comment_ids[3].1));

        // Archive ratings 1 and 2
        assert!(archive_ratings(
            &fixture,
            &[&*rating_comment_ids[1].0, &*rating_comment_ids[2].0]
        )
        .is_ok());

        // Entries still exist
        assert!(fixture.entry_exists(&place_uids[0]));
        assert!(fixture.entry_exists(&place_uids[1]));

        // Ratings 1 and 2 disappeared
        assert!(fixture.rating_exists(&rating_comment_ids[0].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[1].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[2].0));
        assert!(fixture.rating_exists(&rating_comment_ids[3].0));

        // Comments for ratings 1 and 2 disappeared
        assert!(fixture.comment_exists(&rating_comment_ids[0].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[1].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[2].1));
        assert!(fixture.comment_exists(&rating_comment_ids[3].1));

        // Try to archive ratings 0 and 1 (already archived)
        assert_not_found(archive_ratings(
            &fixture,
            &[&*rating_comment_ids[0].0, &*rating_comment_ids[1].0],
        ));

        // No changes due to rollback
        assert!(fixture.entry_exists(&place_uids[0]));
        assert!(fixture.entry_exists(&place_uids[1]));
        assert!(fixture.rating_exists(&rating_comment_ids[0].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[1].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[2].0));
        assert!(fixture.rating_exists(&rating_comment_ids[3].0));
        assert!(fixture.comment_exists(&rating_comment_ids[0].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[1].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[2].1));
        assert!(fixture.comment_exists(&rating_comment_ids[3].1));

        // Archive remaining ratings
        assert!(archive_ratings(
            &fixture,
            &[&*rating_comment_ids[0].0, &*rating_comment_ids[3].0]
        )
        .is_ok());

        // Entries still exist
        assert!(fixture.entry_exists(&place_uids[0]));
        assert!(fixture.entry_exists(&place_uids[1]));

        // All ratings disappeared
        assert!(!fixture.rating_exists(&rating_comment_ids[0].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[1].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[2].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[3].0));

        // All comments disappeared
        assert!(!fixture.comment_exists(&rating_comment_ids[0].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[1].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[2].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[3].1));
    }
}
