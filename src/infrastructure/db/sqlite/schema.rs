///////////////////////////////////////////////////////////////////////
// Tags
///////////////////////////////////////////////////////////////////////

table! {
    tags (id) {
        id -> Text,
    }
}

///////////////////////////////////////////////////////////////////////
// Organizations
///////////////////////////////////////////////////////////////////////

table! {
    organizations (id) {
        id -> Text,
        name -> Text,
        api_token -> Text,
    }
}

table! {
    org_tag_relations (org_id, tag_id) {
        org_id -> Text,
        tag_id -> Text,
    }
}

joinable!(org_tag_relations -> organizations (org_id));
joinable!(org_tag_relations -> tags (tag_id));

///////////////////////////////////////////////////////////////////////
// Users
///////////////////////////////////////////////////////////////////////

table! {
    users (id) {
        id -> BigInt,
        email -> Text,
        email_confirmed -> Bool,
        password -> Text,
        role -> SmallInt,
    }
}

table! {
    user_tokens (id) {
        id -> BigInt,
        user_id -> BigInt,
        expires_at -> BigInt,
        nonce -> Text,
    }
}

joinable!(user_tokens -> users (user_id));

///////////////////////////////////////////////////////////////////////
// Places
///////////////////////////////////////////////////////////////////////

table! {
    place (rowid) {
        rowid -> BigInt,
        current_rev -> BigInt,
        id -> Text,
        license -> Text,
    }
}

table! {
    place_revision (rowid) {
        rowid -> BigInt,
        parent_rowid -> BigInt,
        rev -> BigInt,
        created_at -> BigInt,
        created_by -> Nullable<BigInt>,
        current_status -> SmallInt,
        title -> Text,
        description -> Text,
        lat -> Double,
        lon -> Double,
        street -> Nullable<Text>,
        zip -> Nullable<Text>,
        city -> Nullable<Text>,
        country -> Nullable<Text>,
        state -> Nullable<Text>,
        email -> Nullable<Text>,
        phone -> Nullable<Text>,
        homepage -> Nullable<Text>,
        opening_hours -> Nullable<Text>,
        image_url -> Nullable<Text>,
        image_link_url -> Nullable<Text>,
    }
}

joinable!(place_revision -> place (parent_rowid));

table! {
    place_revision_tag (parent_rowid, tag) {
        parent_rowid -> BigInt,
        tag -> Text,
    }
}

joinable!(place_revision_tag -> place_revision (parent_rowid));

table! {
    place_revision_review (rowid) {
        rowid -> BigInt,
        parent_rowid -> BigInt,
        rev -> BigInt,
        created_at -> BigInt,
        created_by -> Nullable<BigInt>,
        status -> SmallInt,
        context -> Nullable<Text>,
        comment -> Nullable<Text>,
    }
}

joinable!(place_revision_review -> place_revision (parent_rowid));

table! {
    place_rating (rowid) {
        rowid -> BigInt,
        parent_rowid -> BigInt,
        created_at -> BigInt,
        created_by -> Nullable<BigInt>,
        archived_at -> Nullable<BigInt>,
        archived_by -> Nullable<BigInt>,
        id -> Text,
        title -> Text,
        value -> SmallInt,
        context -> Text,
        source -> Nullable<Text>,
    }
}

joinable!(place_rating -> place (parent_rowid));

table! {
    place_rating_comment (rowid) {
        rowid -> BigInt,
        parent_rowid -> BigInt,
        created_at -> BigInt,
        created_by -> Nullable<BigInt>,
        archived_at -> Nullable<BigInt>,
        archived_by -> Nullable<BigInt>,
        id -> Text,
        text -> Text,
    }
}

joinable!(place_rating_comment -> place_rating (parent_rowid));

///////////////////////////////////////////////////////////////////////
// Events
///////////////////////////////////////////////////////////////////////

// TODO: Rename id -> rowid
// TODO: Rename uid -> id
table! {
    events (id) {
        id -> BigInt,
        uid -> Text,
        title -> Text,
        description -> Nullable<Text>,
        start -> BigInt,
        end -> Nullable<BigInt>,
        lat -> Nullable<Double>,
        lng -> Nullable<Double>,
        street -> Nullable<Text>,
        zip -> Nullable<Text>,
        city -> Nullable<Text>,
        country -> Nullable<Text>,
        state -> Nullable<Text>,
        email -> Nullable<Text>,
        telephone -> Nullable<Text>,
        homepage -> Nullable<Text>,
        created_by -> Nullable<BigInt>,
        registration -> Nullable<SmallInt>,
        organizer -> Nullable<Text>,
        archived -> Nullable<BigInt>,
        image_url -> Nullable<Text>,
        image_link_url -> Nullable<Text>,
    }
}

joinable!(events -> users (created_by));

table! {
    event_tags (event_id, tag) {
        event_id -> BigInt,
        tag -> Text,
    }
}

joinable!(event_tags -> events (event_id));

///////////////////////////////////////////////////////////////////////
// Subscriptions
///////////////////////////////////////////////////////////////////////

// TODO: Rename id -> rowid
// TODO: Rename uid -> id
// TODO: Rename user_id -> user_rowid
table! {
    bbox_subscriptions (id) {
        id -> BigInt,
        uid -> Text,
        user_id -> BigInt,
        south_west_lat -> Double,
        south_west_lng -> Double,
        north_east_lat -> Double,
        north_east_lng -> Double,
    }
}

joinable!(bbox_subscriptions -> users (user_id));

///////////////////////////////////////////////////////////////////////

allow_tables_to_appear_in_same_query!(
    bbox_subscriptions,
    events,
    event_tags,
    place,
    place_rating,
    place_rating_comment,
    place_revision,
    place_revision_review,
    place_revision_tag,
    org_tag_relations,
    organizations,
    tags,
    users,
    user_tokens,
);
