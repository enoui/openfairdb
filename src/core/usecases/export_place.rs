use crate::core::prelude::*;

pub fn export_place<'a>(
    place: Place,
    role: Role,
    owned_tags: impl IntoIterator<Item = &'a str>,
) -> Place {
    if role < Role::Admin {
        let place = super::filter_place(place, owned_tags);
        if role < Role::Scout {
            place.strip_contact_details()
        } else {
            place
        }
    } else {
        place
    }
}
