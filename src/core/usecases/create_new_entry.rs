use crate::core::{
    prelude::*,
    util::{parse::parse_url_param, validate::Validate},
};

#[rustfmt::skip]
#[derive(Deserialize, Debug, Clone)]
pub struct NewEntry {
    pub title          : String,
    pub description    : String,
    pub lat            : f64,
    pub lng            : f64,
    pub street         : Option<String>,
    pub zip            : Option<String>,
    pub city           : Option<String>,
    pub country        : Option<String>,
    pub email          : Option<String>,
    pub telephone      : Option<String>,
    pub homepage       : Option<String>,
    pub categories     : Vec<String>,
    pub tags           : Vec<String>,
    pub license        : String,
    pub image_url      : Option<String>,
    pub image_link_url : Option<String>,
}

#[derive(Debug, Clone)]
pub struct Storable(Place);

pub fn prepare_new_place_rev<D: Db>(
    db: &D,
    e: NewEntry,
    created_by_email: Option<&str>,
) -> Result<Storable> {
    let NewEntry {
        title,
        description,
        categories,
        email,
        telephone,
        lat,
        lng,
        street,
        zip,
        city,
        country,
        tags,
        license,
        ..
    } = e;
    let pos = match MapPoint::try_from_lat_lng_deg(lat, lng) {
        None => return Err(ParameterError::InvalidPosition.into()),
        Some(pos) => pos,
    };
    let categories = categories.into_iter().map(Uid::from).collect();
    let tags = super::prepare_tag_list(Category::merge_uids_into_tags(categories, tags));
    super::check_and_count_owned_tags(db, &tags, None)?;
    let address = Address {
        street,
        zip,
        city,
        country,
    };
    let address = if address.is_empty() {
        None
    } else {
        Some(address)
    };
    let location = Location { pos, address };
    let contact = if email.is_some() || telephone.is_some() {
        Some(Contact {
            email: email.map(Into::into),
            phone: telephone,
        })
    } else {
        None
    };
    let homepage = e.homepage.map(|ref url| parse_url_param(url)).transpose()?;
    let image_url = e
        .image_url
        .map(|ref url| parse_url_param(url))
        .transpose()?;
    let image_link_url = e
        .image_link_url
        .map(|ref url| parse_url_param(url))
        .transpose()?;

    let e = Place {
        uid: Uid::new_uuid(),
        rev: Revision::initial(),
        created: Activity::now(created_by_email.map(Into::into)),
        license,
        title,
        description,
        location,
        contact,
        homepage,
        image_url,
        image_link_url,
        tags,
    };
    e.validate()?;
    Ok(Storable(e))
}

pub fn store_new_place_rev<D: Db>(db: &D, s: Storable) -> Result<(Place, Vec<Rating>)> {
    let Storable(place_rev) = s;
    debug!("Storing new place revision: {:?}", place_rev);
    for t in &place_rev.tags {
        db.create_tag_if_it_does_not_exist(&Tag { id: t.clone() })?;
    }
    db.create_place_rev(place_rev.clone())?;
    // No initial ratings so far
    let ratings = vec![];
    Ok((place_rev, ratings))
}

#[cfg(test)]
mod tests {

    use super::super::tests::MockDb;
    use super::*;

    #[test]
    fn create_new_valid_entry() {
        #[rustfmt::skip]
        let x = NewEntry {
            title       : "foo".into(),
            description : "bar".into(),
            lat         : 0.0,
            lng         : 0.0,
            street      : None,
            zip         : None,
            city        : None,
            country     : None,
            email       : None,
            telephone   : None,
            homepage    : None,
            categories  : vec![],
            tags        : vec![],
            license     : "CC0-1.0".into(),
            image_url     : None,
            image_link_url: None,
        };
        let mock_db = MockDb::default();
        let storable = prepare_new_place_rev(&mock_db, x, None).unwrap();
        let (_, initial_ratings) = store_new_place_rev(&mock_db, storable).unwrap();
        assert!(initial_ratings.is_empty());
        assert_eq!(mock_db.entries.borrow().len(), 1);
        let (x, _) = &mock_db.entries.borrow()[0];
        assert_eq!(x.title, "foo");
        assert_eq!(x.description, "bar");
        assert_eq!(x.rev, Revision::initial());
    }

    #[test]
    fn create_entry_with_invalid_email() {
        #[rustfmt::skip]
        let x = NewEntry {
            title       : "foo".into(),
            description : "bar".into(),
            lat         : 0.0,
            lng         : 0.0,
            street      : None,
            zip         : None,
            city        : None,
            country     : None,
            email       : Some("fooo-not-ok".into()),
            telephone   : None,
            homepage    : None,
            categories  : vec![],
            tags        : vec![],
            license     : "CC0-1.0".into(),
            image_url     : None,
            image_link_url: None,
        };
        let mock_db: MockDb = MockDb::default();
        assert!(prepare_new_place_rev(&mock_db, x, None).is_err());
    }

    #[test]
    fn add_new_valid_entry_with_tags() {
        #[rustfmt::skip]
        let x = NewEntry {
            title       : "foo".into(),
            description : "bar".into(),
            lat         : 0.0,
            lng         : 0.0,
            street      : None,
            zip         : None,
            city        : None,
            country     : None,
            email       : None,
            telephone   : None,
            homepage    : None,
            categories  : vec![],
            tags        : vec!["foo".into(),"bar".into()],
            license     : "CC0-1.0".into(),
            image_url     : None,
            image_link_url: None,
        };
        let mock_db = MockDb::default();
        let e = prepare_new_place_rev(&mock_db, x, None).unwrap();
        assert!(store_new_place_rev(&mock_db, e).is_ok());
        assert_eq!(mock_db.tags.borrow().len(), 2);
        assert_eq!(mock_db.entries.borrow().len(), 1);
    }
}
