use crate::core::prelude::*;
use crate::core::util::{self, filter, geo::MapBbox};

#[rustfmt::skip]
#[derive(Debug, Clone)]
pub struct SearchRequest<'a, 'b, 'c, 'd> {
    pub bbox       : MapBbox,
    pub ids        : Vec<&'b str>,
    pub categories : Vec<&'a str>,
    pub hash_tags  : Vec<&'c str>,
    pub text       : Option<&'d str>,
}

pub fn search(
    index: &dyn PlaceIndex,
    req: SearchRequest,
    limit: usize,
) -> Result<(Vec<IndexedPlace>, Vec<IndexedPlace>)> {
    let visible_bbox: MapBbox = req.bbox;

    let mut hash_tags = req.text.map(util::extract_hash_tags).unwrap_or_default();
    hash_tags.reserve(hash_tags.len() + req.hash_tags.len());
    for hashtag in req.hash_tags {
        hash_tags.push(hashtag.to_owned());
    }

    let text = req.text.map(util::remove_hash_tags).and_then(|text| {
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    });

    let text_tags = text
        .as_ref()
        .map(String::as_str)
        .map(filter::split_text_to_words)
        .unwrap_or_default();

    let visible_places_query = IndexQuery {
        include_bbox: Some(visible_bbox),
        exclude_bbox: None,
        categories: req.categories,
        ids: req.ids,
        hash_tags,
        text_tags,
        text,
        status: Some(vec![]),
        ..Default::default()
    };

    // 1st query: Search for visible results only
    // This is required to reliably retrieve all available results!
    // See also: https://github.com/slowtec/openfairdb/issues/183
    let visible_places = index
        .query_places(&visible_places_query, limit)
        .map_err(|err| RepoError::Other(Box::new(err.compat())))?;
    debug_assert!(visible_places
        .iter()
        .all(|e| visible_bbox.contains_point(e.pos)));

    // 2nd query: Search for remaining invisible results
    let invisible_places = if visible_places.len() < limit {
        let invisible_places_query = IndexQuery {
            include_bbox: Some(filter::extend_bbox(&visible_bbox)),
            exclude_bbox: visible_places_query.include_bbox,
            ..visible_places_query
        };
        index
            .query_places(&invisible_places_query, limit - visible_places.len())
            .map_err(|err| RepoError::Other(Box::new(err.compat())))?
    } else {
        vec![]
    };
    debug_assert!(!invisible_places
        .iter()
        .any(|e| visible_bbox.contains_point(e.pos)));

    Ok((visible_places, invisible_places))
}

/// The global search usecase is like the one
/// of usual internet search engines that exists
/// of only one single search input.
/// So here we don't care about tags, categories etc.
/// We also ignore the rating of an entry for now.
pub fn global_search(index: &dyn PlaceIndex, txt: &str, limit: usize) -> Result<Vec<IndexedPlace>> {
    let index_query = IndexQuery {
        text: Some(txt.into()),
        ..Default::default()
    };

    let entries = index
        .query_places(&index_query, limit)
        .map_err(|err| RepoError::Other(Box::new(err.compat())))?;

    Ok(entries)
}
