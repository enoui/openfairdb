use crate::core::prelude::*;

use itertools::Either;

fn is_event_owned(event: &Event, owned_tags: &[String]) -> bool {
    // Exclusive ownership of events is determined by the associated
    // tags.
    owned_tags
        .iter()
        .any(|ref tag| event.tags.iter().any(|ref e_tag| e_tag == tag))
}

pub fn strip_event_details_on_search(
    event_iter: impl Iterator<Item = Event>,
    owned_tags: Vec<String>,
) -> impl Iterator<Item = Event> {
    event_iter.map(move |e| {
        // Hide the created_by e-mail address if the event is not owned.
        if is_event_owned(&e, &owned_tags) {
            // Include created_by
            e
        } else {
            // Exclude created_by
            Event {
                created_by: None,
                ..e
            }
        }
    })
}

pub fn strip_event_details_on_export(
    event_iter: impl Iterator<Item = Event>,
    role: Role,
    owned_tags: Vec<String>,
) -> impl Iterator<Item = Event> {
    if role >= Role::Admin {
        // Admin sees everything even if no owned tags are provided
        Either::Left(event_iter)
    } else {
        Either::Right(event_iter.map(move |e| {
            // Contact details are only visible for scouts and admins
            if role >= Role::Scout {
                // Include contact details
                let owned = is_event_owned(&e, &owned_tags);
                if owned {
                    // Include created_by
                    e
                } else {
                    // Exclude created_by
                    Event {
                        created_by: None,
                        ..e
                    }
                }
            } else {
                // Exclude both contact details and created_by
                Event {
                    contact: None,
                    created_by: None,
                    ..e
                }
            }
        }))
    }
}
