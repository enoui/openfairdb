use super::models::*;
use crate::core::entities as e;
use std::str::FromStr;

impl From<e::Entry> for Entry {
    fn from(e: e::Entry) -> Self {
        let e::Entry {
            id,
            osm_node,
            created,
            version,
            title,
            description,
            location,
            contact,
            homepage,
            license,
            image_url,
            image_link_url,
            ..
        } = e;

        let e::Location { lat, lng, address } = location;

        let e::Address {
            street,
            zip,
            city,
            country,
        } = address.unwrap_or_else(|| e::Address {
            street: None,
            zip: None,
            city: None,
            country: None,
        });

        let e::Contact { email, telephone } = contact.unwrap_or_else(|| e::Contact {
            email: None,
            telephone: None,
        });

        Entry {
            id,
            osm_node: osm_node.map(|x| x as i64),
            created: created as i64,
            version: version as i64,
            current: true,
            title,
            description,
            lat,
            lng,
            street,
            zip,
            city,
            country,
            email,
            telephone,
            homepage,
            license,
            image_url,
            image_link_url,
        }
    }
}

impl From<e::Event> for Event {
    fn from(e: e::Event) -> Self {
        let e::Event {
            id,
            title,
            start,
            end,
            description,
            location,
            contact,
            homepage,
            ..
        } = e;

        let mut street = None;
        let mut zip = None;
        let mut city = None;
        let mut country = None;

        let (lat, lng) = if let Some(l) = location {
            if let Some(a) = l.address {
                street = a.street;
                zip = a.zip;
                city = a.city;
                country = a.country;
            }
            (Some(l.lat), Some(l.lng))
        } else {
            (None, None)
        };

        let (email, telephone) = if let Some(c) = contact {
            (c.email, c.telephone)
        } else {
            (None, None)
        };

        Event {
            id,
            title,
            description,
            start: start as i64,
            end: end.map(|x| x as i64),
            lat,
            lng,
            street,
            zip,
            city,
            country,
            telephone,
            email,
            homepage,
        }
    }
}

impl From<(Entry, &Vec<EntryCategoryRelation>, &Vec<EntryTagRelation>)> for e::Entry {
    fn from(d: (Entry, &Vec<EntryCategoryRelation>, &Vec<EntryTagRelation>)) -> Self {
        let (e, cat_rels, tag_rels) = d;
        let Entry {
            id,
            version,
            created,
            title,
            description,
            lat,
            lng,
            street,
            zip,
            city,
            country,
            email,
            telephone,
            license,
            homepage,
            image_url,
            image_link_url,
            ..
        } = e;
        let categories = cat_rels
            .iter()
            .filter(|r| r.entry_id == id)
            .filter(|r| r.entry_version == version)
            .map(|r| &r.category_id)
            .cloned()
            .collect();
        let tags = tag_rels
            .iter()
            .filter(|r| r.entry_id == id)
            .filter(|r| r.entry_version == version)
            .map(|r| &r.tag_id)
            .cloned()
            .collect();
        let location = e::Location {
            lat: lat as f64,
            lng: lng as f64,
            address: if street.is_some() || zip.is_some() || city.is_some() || country.is_some() {
                Some(e::Address {
                    street,
                    zip,
                    city,
                    country,
                })
            } else {
                None
            },
        };
        let contact = if email.is_some() || telephone.is_some() {
            Some(e::Contact { email, telephone })
        } else {
            None
        };
        e::Entry {
            id,
            osm_node: e.osm_node.map(|x| x as u64),
            created: created as u64,
            version: version as u64,
            title,
            description,
            location,
            contact,
            homepage,
            categories,
            tags,
            license,
            image_url,
            image_link_url,
        }
    }
}

impl From<(Event, &Vec<EventTagRelation>)> for e::Event {
    fn from(d: (Event, &Vec<EventTagRelation>)) -> Self {
        let (e, tag_rels) = d;
        let Event {
            id,
            title,
            description,
            start,
            end,
            lat,
            lng,
            street,
            zip,
            city,
            country,
            email,
            telephone,
            homepage,
            ..
        } = e;
        let tags = tag_rels
            .iter()
            .filter(|r| r.event_id == id)
            .map(|r| &r.tag_id)
            .cloned()
            .collect();
        let address = if street.is_some() || zip.is_some() || city.is_some() || country.is_some() {
            Some(e::Address {
                street,
                zip,
                city,
                country,
            })
        } else {
            None
        };
        let location = if address.is_some() || lat.is_some() || lng.is_some() {
            Some(e::Location {
                // TODO: How to handle missing lat/lng?
                lat: lat.unwrap_or(0.0),
                lng: lng.unwrap_or(0.0),
                address,
            })
        } else {
            None
        };
        let contact = if email.is_some() || telephone.is_some() {
            Some(e::Contact { email, telephone })
        } else {
            None
        };
        e::Event {
            id,
            title,
            description,
            start: start as u64,
            end: end.map(|x| x as u64),
            location,
            contact,
            homepage,
            tags,
        }
    }
}

impl From<Category> for e::Category {
    fn from(c: Category) -> e::Category {
        let Category {
            id,
            name,
            created,
            version,
        } = c;
        e::Category {
            id,
            name,
            created: created as u64,
            version: version as u64,
        }
    }
}

impl From<e::Category> for Category {
    fn from(c: e::Category) -> Category {
        let e::Category {
            id,
            name,
            created,
            version,
        } = c;
        Category {
            id,
            name,
            created: created as i64,
            version: version as i64,
        }
    }
}

impl From<Tag> for e::Tag {
    fn from(t: Tag) -> e::Tag {
        e::Tag { id: t.id }
    }
}

impl From<e::Tag> for Tag {
    fn from(t: e::Tag) -> Tag {
        Tag { id: t.id }
    }
}

impl From<User> for e::User {
    fn from(u: User) -> e::User {
        use num_traits::FromPrimitive;
        let User {
            id,
            username,
            password,
            email,
            email_confirmed,
            role,
        } = u;
        e::User {
            id,
            username,
            password,
            email,
            email_confirmed,
            role: e::Role::from_i16(role).unwrap_or_else(|| {
                warn!(
                    "Could not cast role from i16 (value: {}). Use {:?} instead.",
                    role,
                    e::Role::default()
                );
                e::Role::default()
            }),
        }
    }
}

impl From<e::User> for User {
    fn from(u: e::User) -> User {
        use num_traits::ToPrimitive;
        let e::User {
            id,
            username,
            password,
            email,
            email_confirmed,
            role,
        } = u;
        User {
            id,
            username,
            password,
            email,
            email_confirmed,
            role: role.to_i16().unwrap_or_else(|| {
                warn!("Could not convert role {:?} to i16. Use 0 instead.", role);
                0
            }),
        }
    }
}

impl From<Comment> for e::Comment {
    fn from(c: Comment) -> e::Comment {
        let Comment {
            id,
            created,
            text,
            rating_id,
        } = c;
        e::Comment {
            id,
            created: created as u64,
            text,
            rating_id,
        }
    }
}

impl From<e::Comment> for Comment {
    fn from(c: e::Comment) -> Comment {
        let e::Comment {
            id,
            created,
            text,
            rating_id,
        } = c;
        Comment {
            id,
            created: created as i64,
            text,
            rating_id,
        }
    }
}

impl From<Rating> for e::Rating {
    fn from(r: Rating) -> e::Rating {
        let Rating {
            id,
            entry_id,
            created,
            title,
            context,
            value,
            source,
        } = r;
        e::Rating {
            id,
            entry_id,
            created: created as u64,
            title,
            value: value as i8,
            context: context.parse().unwrap(),
            source,
        }
    }
}

impl From<e::Rating> for Rating {
    fn from(r: e::Rating) -> Rating {
        let e::Rating {
            id,
            created,
            title,
            context,
            value,
            source,
            entry_id,
        } = r;
        Rating {
            id,
            created: created as i64,
            title,
            value: i32::from(value),
            context: context.into(),
            source,
            entry_id,
        }
    }
}

impl From<BboxSubscription> for e::BboxSubscription {
    fn from(s: BboxSubscription) -> e::BboxSubscription {
        let BboxSubscription {
            id,
            south_west_lat,
            south_west_lng,
            north_east_lat,
            north_east_lng,
            username,
        } = s;
        e::BboxSubscription {
            id,
            bbox: e::Bbox {
                south_west: e::Coordinate {
                    lat: south_west_lat as f64,
                    lng: south_west_lng as f64,
                },
                north_east: e::Coordinate {
                    lat: north_east_lat as f64,
                    lng: north_east_lng as f64,
                },
            },
            username,
        }
    }
}

impl From<e::BboxSubscription> for BboxSubscription {
    fn from(s: e::BboxSubscription) -> BboxSubscription {
        let e::BboxSubscription { id, bbox, username } = s;
        BboxSubscription {
            id,
            south_west_lat: bbox.south_west.lat,
            south_west_lng: bbox.south_west.lng,
            north_east_lat: bbox.north_east.lat,
            north_east_lng: bbox.north_east.lng,
            username,
        }
    }
}

impl From<e::RatingContext> for String {
    fn from(context: e::RatingContext) -> String {
        match context {
            e::RatingContext::Diversity => "diversity",
            e::RatingContext::Renewable => "renewable",
            e::RatingContext::Fairness => "fairness",
            e::RatingContext::Humanity => "humanity",
            e::RatingContext::Transparency => "transparency",
            e::RatingContext::Solidarity => "solidarity",
        }
        .into()
    }
}

impl FromStr for e::RatingContext {
    type Err = String;
    fn from_str(context: &str) -> Result<e::RatingContext, String> {
        Ok(match context {
            "diversity" => e::RatingContext::Diversity,
            "renewable" => e::RatingContext::Renewable,
            "fairness" => e::RatingContext::Fairness,
            "humanity" => e::RatingContext::Humanity,
            "transparency" => e::RatingContext::Transparency,
            "solidarity" => e::RatingContext::Solidarity,
            _ => {
                return Err(format!("invalid RatingContext: '{}'", context));
            }
        })
    }
}
