use super::*;

use diesel::connection::Connection;

pub fn exec_archive_places(
    connections: &sqlite::Connections,
    ids: &[&str],
    archived_by_email: Option<&str>,
) -> Result<()> {
    let mut repo_err = None;
    let connection = connections.exclusive()?;
    Ok(connection
        .transaction::<_, diesel::result::Error, _>(|| {
            usecases::archive_places(&*connection, ids, archived_by_email).map_err(|err| {
                warn!("Failed to archive {} entries: {}", ids.len(), err);
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

pub fn post_archive_places(indexer: &mut dyn PlaceIndexer, ids: &[&str]) -> Result<()> {
    // Remove archived entries from search index
    // TODO: Move to a separate task/thread that doesn't delay this request
    for id in ids {
        if let Err(err) = usecases::unindex_entry(indexer, id) {
            error!(
                "Failed to remove archived entry {} from search index: {}",
                id, err
            );
        }
    }
    if let Err(err) = indexer.flush() {
        error!(
            "Failed to finish updating the search index after archiving entries: {}",
            err
        );
    }
    Ok(())
}

pub fn archive_places(
    connections: &sqlite::Connections,
    indexer: &mut dyn PlaceIndexer,
    ids: &[&str],
    archived_by_email: Option<&str>,
) -> Result<()> {
    exec_archive_places(connections, ids, archived_by_email)?;
    post_archive_places(indexer, ids)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::prelude::*;

    fn archive_places(fixture: &EnvFixture, ids: &[&str]) -> super::Result<()> {
        super::archive_places(
            &fixture.db_connections,
            &mut *fixture.search_engine.borrow_mut(),
            ids,
            None,
        )
    }

    #[test]
    fn should_archive_multiple_places_only_once() {
        let fixture = EnvFixture::new();
        let place_uids = vec![
            fixture.create_place(0.into(), None),
            fixture.create_place(1.into(), None),
            fixture.create_place(2.into(), None),
        ];
        let entry_tags = vec![
            fixture
                .try_get_entry(&place_uids[0])
                .unwrap()
                .tags
                .into_iter()
                .take(1)
                .next()
                .unwrap(),
            fixture
                .try_get_entry(&place_uids[1])
                .unwrap()
                .tags
                .into_iter()
                .take(1)
                .next()
                .unwrap(),
            fixture
                .try_get_entry(&place_uids[2])
                .unwrap()
                .tags
                .into_iter()
                .take(1)
                .next()
                .unwrap(),
        ];

        assert!(fixture.entry_exists(&place_uids[0]));
        assert_eq!(
            place_uids[0],
            fixture.query_places_by_tag(&entry_tags[0])[0].id
        );
        assert!(fixture.entry_exists(&place_uids[1]));
        assert_eq!(
            place_uids[1],
            fixture.query_places_by_tag(&entry_tags[1])[0].id
        );
        assert!(fixture.entry_exists(&place_uids[2]));
        assert_eq!(
            place_uids[2],
            fixture.query_places_by_tag(&entry_tags[2])[0].id
        );

        assert!(archive_places(&fixture, &[&*place_uids[0], &*place_uids[2]]).is_ok());

        // Entries 0 and 2 disappeared
        assert!(!fixture.entry_exists(&place_uids[0]));
        assert!(fixture.query_places_by_tag(&entry_tags[0]).is_empty());
        assert!(fixture.entry_exists(&place_uids[1]));
        assert_eq!(
            place_uids[1],
            fixture.query_places_by_tag(&entry_tags[1])[0].id
        );
        assert!(!fixture.entry_exists(&place_uids[2]));
        assert!(fixture.query_places_by_tag(&entry_tags[2]).is_empty());

        assert_not_found(archive_places(
            &fixture,
            &[&*place_uids[1], &*place_uids[2]],
        ));

        // No changes, i.e.entry 1 still exists in both database and index
        assert!(!fixture.entry_exists(&place_uids[0]));
        assert!(fixture.query_places_by_tag(&entry_tags[0]).is_empty());
        assert!(fixture.entry_exists(&place_uids[1]));
        assert_eq!(
            place_uids[1],
            fixture.query_places_by_tag(&entry_tags[1])[0].id
        );
        assert!(!fixture.entry_exists(&place_uids[2]));
        assert!(fixture.query_places_by_tag(&entry_tags[2]).is_empty());
    }

    #[test]
    fn should_archive_places_with_ratings_and_comments() {
        let fixture = EnvFixture::new();
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

        for place_uid in &place_uids {
            assert!(fixture.entry_exists(place_uid));
        }
        for (rating_id, comment_id) in &rating_comment_ids {
            assert!(fixture.rating_exists(rating_id));
            assert!(fixture.comment_exists(comment_id));
        }

        assert!(archive_places(&fixture, &[&*place_uids[0]]).is_ok());

        assert!(!fixture.entry_exists(&place_uids[0]));
        assert!(fixture.entry_exists(&place_uids[1]));

        assert!(!fixture.rating_exists(&rating_comment_ids[0].0));
        assert!(!fixture.rating_exists(&rating_comment_ids[1].0));
        assert!(fixture.rating_exists(&rating_comment_ids[2].0));
        assert!(fixture.rating_exists(&rating_comment_ids[3].0));

        assert!(!fixture.comment_exists(&rating_comment_ids[0].1));
        assert!(!fixture.comment_exists(&rating_comment_ids[1].1));
        assert!(fixture.comment_exists(&rating_comment_ids[2].1));
        assert!(fixture.comment_exists(&rating_comment_ids[3].1));
    }
}
