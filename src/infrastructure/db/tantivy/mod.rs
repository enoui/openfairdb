use crate::core::{
    db::{IndexedPlace, PlaceIndex, IndexQuery, PlaceIndexer},
    entities::{AvgRatingValue, AvgRatings, Category, Place, RatingContext, Id},
    util::geo::{LatCoord, LngCoord, MapPoint, RawCoord},
};

use failure::{bail, Fallible};
use std::{
    ops::Bound,
    path::Path,
    sync::{Arc, Mutex},
};
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, Occur, Query, QueryParser, RangeQuery, TermQuery},
    schema::*,
    tokenizer::{LowerCaser, RawTokenizer, Tokenizer},
    DocId, Document, Index, IndexReader, IndexWriter, ReloadPolicy, Score, SegmentReader,
};

const OVERALL_INDEX_HEAP_SIZE_IN_BYTES: usize = 50_000_000;

// Shared fields for both places and events
struct IndexedFields {
    id: Field,
    lat: Field,
    lng: Field,
    title: Field,
    description: Field,
    address_street: Field,
    address_city: Field,
    address_zip: Field,
    address_country: Field,
    tag: Field,
    ratings_diversity: Field,
    ratings_fairness: Field,
    ratings_humanity: Field,
    ratings_renewable: Field,
    ratings_solidarity: Field,
    ratings_transparency: Field,
    total_rating: Field,
}

impl IndexedFields {
    fn build_schema() -> (Self, Schema) {
        let id_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(ID_TOKENIZER)
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored();
        let tag_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TAG_TOKENIZER)
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let address_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TEXT_TOKENIZER)
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            // Address fields currently don't need to be stored
            //.set_stored()
            ;
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TEXT_TOKENIZER)
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        let mut schema_builder = SchemaBuilder::default();
        let fields = Self {
            id: schema_builder.add_text_field("id", id_options),
            lat: schema_builder.add_i64_field("lat", INDEXED | STORED),
            lng: schema_builder.add_i64_field("lon", INDEXED | STORED),
            title: schema_builder.add_text_field("tit", text_options.clone()),
            description: schema_builder.add_text_field("dsc", text_options.clone()),
            address_street: schema_builder
                .add_text_field("adr_street", address_options.clone()),
            address_city: schema_builder.add_text_field("adr_city", address_options.clone()),
            address_zip: schema_builder.add_text_field("adr_zip", address_options.clone()),
            address_country: schema_builder.add_text_field("adr_country", address_options),
            tag: schema_builder.add_text_field("tag", tag_options),
            ratings_diversity: schema_builder.add_u64_field("rat_diversity", STORED),
            ratings_fairness: schema_builder.add_u64_field("rat_fairness", STORED),
            ratings_humanity: schema_builder.add_u64_field("rat_humanity", STORED),
            ratings_renewable: schema_builder.add_u64_field("rat_renewable", STORED),
            ratings_solidarity: schema_builder.add_u64_field("rat_solidarity", STORED),
            ratings_transparency: schema_builder.add_u64_field("rat_transparency", STORED),
            total_rating: schema_builder.add_u64_field("rat_total", STORED | FAST),
        };
        (fields, schema_builder.build())
    }

    fn read_indexed_place(&self, doc: &Document) -> IndexedPlace {
        let mut lat: Option<LatCoord> = Default::default();
        let mut lng: Option<LngCoord> = Default::default();
        let mut entry = IndexedPlace::default();
        entry.tags.reserve(32);
        for field_value in doc.field_values() {
            match field_value {
                fv if fv.field() == self.lat => {
                    debug_assert!(lat.is_none());
                    let raw_val = fv.value().i64_value();
                    debug_assert!(raw_val >= LatCoord::min().to_raw().into());
                    debug_assert!(raw_val <= LatCoord::max().to_raw().into());
                    lat = Some(LatCoord::from_raw(raw_val as RawCoord));
                }
                fv if fv.field() == self.lng => {
                    debug_assert!(lng.is_none());
                    let raw_val = fv.value().i64_value();
                    debug_assert!(raw_val >= LngCoord::min().to_raw().into());
                    debug_assert!(raw_val <= LngCoord::max().to_raw().into());
                    lng = Some(LngCoord::from_raw(raw_val as RawCoord));
                }
                fv if fv.field() == self.id => {
                    debug_assert!(entry.id.is_empty());
                    if let Some(id) = fv.value().text() {
                        entry.id = id.into();
                    } else {
                        error!("Invalid id value: {:?}", fv.value());
                    }
                }
                fv if fv.field() == self.title => {
                    debug_assert!(entry.title.is_empty());
                    if let Some(title) = fv.value().text() {
                        entry.title = title.into();
                    } else {
                        error!("Invalid title value: {:?}", fv.value());
                    }
                }
                fv if fv.field() == self.description => {
                    debug_assert!(entry.description.is_empty());
                    if let Some(description) = fv.value().text() {
                        entry.description = description.into();
                    } else {
                        error!("Invalid description value: {:?}", fv.value());
                    }
                }
                fv if fv.field() == self.tag => {
                    if let Some(tag) = fv.value().text() {
                        entry.tags.push(tag.into());
                    } else {
                        error!("Invalid tag value: {:?}", fv.value());
                    }
                }
                fv if fv.field() == self.ratings_diversity => {
                    debug_assert!(entry.ratings.diversity == Default::default());
                    entry.ratings.diversity = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.ratings_fairness => {
                    debug_assert!(entry.ratings.fairness == Default::default());
                    entry.ratings.fairness = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.ratings_humanity => {
                    debug_assert!(entry.ratings.humanity == Default::default());
                    entry.ratings.humanity = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.ratings_renewable => {
                    debug_assert!(entry.ratings.renewable == Default::default());
                    entry.ratings.renewable = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.ratings_solidarity => {
                    debug_assert!(entry.ratings.solidarity == Default::default());
                    entry.ratings.solidarity = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.ratings_transparency => {
                    debug_assert!(entry.ratings.transparency == Default::default());
                    entry.ratings.transparency = u64_to_avg_rating(fv.value().u64_value());
                }
                fv if fv.field() == self.total_rating => (),
                // Address fields are currently not stored
                //fv if fv.field() == self.address_street => (),
                //fv if fv.field() == self.address_city => (),
                //fv if fv.field() == self.address_zip => (),
                //fv if fv.field() == self.address_country => (),
                fv => {
                    error!("Unexpected field value: {:?}", fv);
                }
            }
        }
        if let (Some(lat), Some(lng)) = (lat, lng) {
            entry.pos = MapPoint::new(lat, lng);
        } else {
            error!("Invalid position: lat = {:?}, lng = {:?}", lat, lng);
        }
        entry
    }
}

pub(crate) struct TantivyPlaceIndex {
    fields: IndexedFields,
    index_reader: IndexReader,
    index_writer: IndexWriter,
    text_query_parser: QueryParser,
}

const ID_TOKENIZER: &str = "raw";
const TAG_TOKENIZER: &str = "tag";
const TEXT_TOKENIZER: &str = "default";

fn register_tokenizers(index: &Index) {
    // Predefined tokenizers
    debug_assert!(index.tokenizers().get(ID_TOKENIZER).is_some());
    debug_assert!(index.tokenizers().get(TEXT_TOKENIZER).is_some());
    // Custom tokenizer(s)
    debug_assert!(index.tokenizers().get(TAG_TOKENIZER).is_none());
    index
        .tokenizers()
        .register(TAG_TOKENIZER, RawTokenizer.filter(LowerCaser));
}

fn f64_to_u64(val: f64, min: f64, max: f64) -> u64 {
    debug_assert!(val >= min);
    debug_assert!(val <= max);
    debug_assert!(min < max);
    if (val - max).abs() <= std::f64::EPSILON {
        u64::max_value()
    } else if (val - min).abs() <= std::f64::EPSILON {
        0u64
    } else {
        let norm = (val.max(min).min(max) - min) / (max - min);
        let mapped = u64::max_value() as f64 * norm;
        mapped.round() as u64
    }
}

fn u64_to_f64(val: u64, min: f64, max: f64) -> f64 {
    debug_assert!(min < max);
    if val == u64::max_value() {
        max
    } else if val == 0 {
        min
    } else {
        min + val as f64 * ((max - min) / u64::max_value() as f64)
    }
}

fn avg_rating_to_u64(avg_rating: AvgRatingValue) -> u64 {
    f64_to_u64(
        avg_rating.into(),
        AvgRatingValue::min().into(),
        AvgRatingValue::max().into(),
    )
}

fn u64_to_avg_rating(val: u64) -> AvgRatingValue {
    u64_to_f64(
        val,
        AvgRatingValue::min().into(),
        AvgRatingValue::max().into(),
    )
    .into()
}

#[derive(Copy, Clone, Debug)]
enum TopDocsMode {
    Rating,
    ScoreBoostedByRating,
}

impl TantivyPlaceIndex {
    pub fn create_in_ram() -> Fallible<Self> {
        let no_path: Option<&Path> = None;
        Self::create(no_path)
    }

    pub fn create<P: AsRef<Path>>(path: Option<P>) -> Fallible<Self> {
        let (fields, schema) = IndexedFields::build_schema();

        // TODO: Open index from existing directory
        let index = if let Some(path) = path {
            info!(
                "Creating full-text search index in directory: {}",
                path.as_ref().to_string_lossy()
            );
            Index::create_in_dir(path, schema)?
        } else {
            warn!("Creating full-text search index in RAM");
            Index::create_in_ram(schema)
        };

        register_tokenizers(&index);

        // Prefer to manually reload the index reader during `flush()`
        // to ensure that all committed changes become visible immediately.
        // Otherwise ReloadPolicy::OnCommit will delay the changes and
        // many tests would fail without modification.
        let index_reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let index_writer = index.writer(OVERALL_INDEX_HEAP_SIZE_IN_BYTES)?;
        let text_query_parser = QueryParser::for_index(
            &index,
            vec![
                fields.title,
                fields.description,
                fields.address_street,
                fields.address_city,
                fields.address_zip,
                fields.address_country,
            ],
        );
        Ok(Self {
            fields,
            index_reader,
            index_writer,
            text_query_parser,
        })
    }

    fn build_query(&self, query: &IndexQuery) -> (BooleanQuery, TopDocsMode) {
        let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(1 + 2 + 1 + 1 + 1);

        if !query.ids.is_empty() {
            let ids_query: Box<dyn Query> = if query.ids.len() > 1 {
                debug!("Query multiple ids: {:?}", query.ids);
                let mut id_queries: Vec<(Occur, Box<dyn Query>)> =
                    Vec::with_capacity(query.ids.len());
                for id in &query.ids {
                    debug_assert!(!id.trim().is_empty());
                    let id_term = Term::from_field_text(self.fields.id, id);
                    let id_query = TermQuery::new(id_term, IndexRecordOption::Basic);
                    id_queries.push((Occur::Should, Box::new(id_query)));
                }
                Box::new(BooleanQuery::from(id_queries))
            } else {
                let id = &query.ids[0];
                debug!("Query single id: {:?}", id);
                debug_assert!(!id.trim().is_empty());
                let id_term = Term::from_field_text(self.fields.id, &id);
                Box::new(TermQuery::new(id_term, IndexRecordOption::Basic))
            };
            sub_queries.push((Occur::Must, ids_query));
        }

        // Bbox (include)
        if let Some(ref bbox) = query.include_bbox {
            debug!("Query bbox (include): {}", bbox);
            debug_assert!(bbox.is_valid());
            debug_assert!(!bbox.is_empty());
            let lat_query = RangeQuery::new_i64_bounds(
                self.fields.lat,
                Bound::Included(i64::from(bbox.south_west().lat().to_raw())),
                Bound::Included(i64::from(bbox.north_east().lat().to_raw())),
            );
            // Latitude query: Always inclusive
            sub_queries.push((Occur::Must, Box::new(lat_query)));
            // Longitude query: Either inclusive or exclusive (wrap around)
            if bbox.south_west().lng() <= bbox.north_east().lng() {
                // regular (inclusive)
                let lng_query = RangeQuery::new_i64_bounds(
                    self.fields.lng,
                    Bound::Included(i64::from(bbox.south_west().lng().to_raw())),
                    Bound::Included(i64::from(bbox.north_east().lng().to_raw())),
                );
                sub_queries.push((Occur::Must, Box::new(lng_query)));
            } else {
                // inverse (exclusive)
                let lng_query = RangeQuery::new_i64_bounds(
                    self.fields.lng,
                    Bound::Excluded(i64::from(bbox.north_east().lng().to_raw())),
                    Bound::Excluded(i64::from(bbox.south_west().lng().to_raw())),
                );
                sub_queries.push((Occur::MustNot, Box::new(lng_query)));
            }
        }

        // Inverse Bbox (exclude)
        if let Some(ref bbox) = query.exclude_bbox {
            debug!("Query bbox (exclude): {}", bbox);
            debug_assert!(bbox.is_valid());
            debug_assert!(!bbox.is_empty());
            let lat_query = RangeQuery::new_i64_bounds(
                self.fields.lat,
                Bound::Included(i64::from(bbox.south_west().lat().to_raw())),
                Bound::Included(i64::from(bbox.north_east().lat().to_raw())),
            );
            // Latitude query: Always exclusive
            sub_queries.push((Occur::MustNot, Box::new(lat_query)));
            // Longitude query: Either exclusive or inclusive (wrap around)
            if bbox.south_west().lng() <= bbox.north_east().lng() {
                // regular (exclusive)
                let lng_query = RangeQuery::new_i64_bounds(
                    self.fields.lng,
                    Bound::Included(i64::from(bbox.south_west().lng().to_raw())),
                    Bound::Included(i64::from(bbox.north_east().lng().to_raw())),
                );
                sub_queries.push((Occur::MustNot, Box::new(lng_query)));
            } else {
                // inverse (inclusive)
                let lng_query = RangeQuery::new_i64_bounds(
                    self.fields.lng,
                    Bound::Excluded(i64::from(bbox.north_east().lng().to_raw())),
                    Bound::Excluded(i64::from(bbox.south_west().lng().to_raw())),
                );
                sub_queries.push((Occur::Must, Box::new(lng_query)));
            }
        }

        let merged_tags = Category::merge_ids_into_tags(
            query.categories.iter().map(|c| Id::from(*c)).collect(),
            query.hash_tags.clone(),
        );
        let (tags, categories) = Category::split_from_tags(merged_tags);

        // Categories (= mapped to predefined tags + separate sub-query)
        if !categories.is_empty() {
            let categories_query: Box<dyn Query> = if categories.len() > 1 {
                debug!("Query multiple categories: {:?}", categories);
                let mut category_queries: Vec<(Occur, Box<dyn Query>)> =
                    Vec::with_capacity(categories.len());
                for category in &categories {
                    let tag_term = Term::from_field_text(self.fields.tag, &category.tag);
                    let tag_query = TermQuery::new(tag_term, IndexRecordOption::Basic);
                    category_queries.push((Occur::Should, Box::new(tag_query)));
                }
                Box::new(BooleanQuery::from(category_queries))
            } else {
                let category = &categories[0];
                debug!("Query single category: {:?}", category);
                let tag_term = Term::from_field_text(self.fields.tag, &category.tag);
                Box::new(TermQuery::new(tag_term, IndexRecordOption::Basic))
            };
            sub_queries.push((Occur::Must, categories_query));
        }

        // Hash tags (mandatory)
        for tag in &tags {
            debug!("Query hash tag (mandatory): {}", tag);
            debug_assert!(!tag.trim().is_empty());
            let tag_term = Term::from_field_text(self.fields.tag, &tag.to_lowercase());
            let tag_query = TermQuery::new(tag_term, IndexRecordOption::Basic);
            sub_queries.push((Occur::Must, Box::new(tag_query)));
        }

        let mut text_and_tags_queries: Vec<(Occur, Box<dyn Query>)> =
            Vec::with_capacity(1 + query.text_tags.len());

        // Text
        if let Some(text) = &query.text {
            debug!("Query text: {}", text);
            debug_assert!(!text.trim().is_empty());
            let text = text.to_lowercase();
            match self.text_query_parser.parse_query(&text) {
                Ok(text_query) => {
                    if query.hash_tags.is_empty() && query.text_tags.is_empty() {
                        sub_queries.push((Occur::Must, Box::new(text_query)));
                    } else {
                        text_and_tags_queries.push((Occur::Should, Box::new(text_query)));
                    }
                }
                Err(err) => {
                    warn!("Failed to parse query text '{}': {:?}", text, err);
                }
            }
        }

        // Text tags (optional)
        for tag in &query.text_tags {
            debug!("Query text tag (optional): {}", tag);
            debug_assert!(!tag.trim().is_empty());
            let tag_term = Term::from_field_text(self.fields.tag, &tag.to_lowercase());
            let tag_query = TermQuery::new(tag_term, IndexRecordOption::Basic);
            text_and_tags_queries.push((Occur::Should, Box::new(tag_query)));
        }

        // Boosting the score by the rating does only make sense if the
        // query actually contains search terms or tags. Otherwise the
        // results are sorted only by their rating, e.g. if the query
        // contains just the bounding box or ids.
        if text_and_tags_queries.is_empty() {
            (sub_queries.into(), TopDocsMode::Rating)
        } else {
            sub_queries.push((
                Occur::Must,
                Box::new(BooleanQuery::from(text_and_tags_queries)),
            ));
            (sub_queries.into(), TopDocsMode::ScoreBoostedByRating)
        }
    }
}

impl PlaceIndexer for TantivyPlaceIndex {
    fn add_or_update_place(&mut self, place: &Place, ratings: &AvgRatings) -> Fallible<()> {
        let id_term = Term::from_field_text(self.fields.id, place.id.as_ref());
        self.index_writer.delete_term(id_term);
        let mut doc = Document::default();
        doc.add_text(self.fields.id, place.id.as_ref());
        doc.add_i64(
            self.fields.lat,
            i64::from(place.location.pos.lat().to_raw()),
        );
        doc.add_i64(
            self.fields.lng,
            i64::from(place.location.pos.lng().to_raw()),
        );
        doc.add_text(self.fields.title, &place.title);
        doc.add_text(self.fields.description, &place.description);
        if let Some(street) = place
            .location
            .address
            .as_ref()
            .and_then(|address| address.street.as_ref())
        {
            doc.add_text(self.fields.address_street, street);
        }
        if let Some(city) = place
            .location
            .address
            .as_ref()
            .and_then(|address| address.city.as_ref())
        {
            doc.add_text(self.fields.address_city, city);
        }
        if let Some(zip) = place
            .location
            .address
            .as_ref()
            .and_then(|address| address.zip.as_ref())
        {
            doc.add_text(self.fields.address_zip, zip);
        }
        if let Some(country) = place
            .location
            .address
            .as_ref()
            .and_then(|address| address.country.as_ref())
        {
            doc.add_text(self.fields.address_country, country);
        }
        for tag in &place.tags {
            doc.add_text(self.fields.tag, tag);
        }
        doc.add_u64(self.fields.total_rating, avg_rating_to_u64(ratings.total()));
        doc.add_u64(
            self.fields.ratings_diversity,
            avg_rating_to_u64(ratings.diversity),
        );
        doc.add_u64(
            self.fields.ratings_fairness,
            avg_rating_to_u64(ratings.fairness),
        );
        doc.add_u64(
            self.fields.ratings_humanity,
            avg_rating_to_u64(ratings.humanity),
        );
        doc.add_u64(
            self.fields.ratings_renewable,
            avg_rating_to_u64(ratings.renewable),
        );
        doc.add_u64(
            self.fields.ratings_solidarity,
            avg_rating_to_u64(ratings.solidarity),
        );
        doc.add_u64(
            self.fields.ratings_transparency,
            avg_rating_to_u64(ratings.transparency),
        );
        self.index_writer.add_document(doc);
        Ok(())
    }

    fn remove_place_by_id(&mut self, id: &str) -> Fallible<()> {
        let id_term = Term::from_field_text(self.fields.id, id);
        self.index_writer.delete_term(id_term);
        Ok(())
    }

    fn flush(&mut self) -> Fallible<()> {
        self.index_writer.commit()?;
        // Manually reload the reader to ensure that all committed changes
        // become visible immediately.
        self.index_reader.reload()?;
        Ok(())
    }
}

impl PlaceIndex for TantivyPlaceIndex {
    #[allow(clippy::absurd_extreme_comparisons)]
    fn query_places(&self, query: &IndexQuery, limit: usize) -> Fallible<Vec<IndexedPlace>> {
        if limit <= 0 {
            bail!("Invalid limit: {}", limit);
        }

        let (search_query, top_docs_mode) = self.build_query(query);
        let searcher = self.index_reader.searcher();
        // TODO: Try to combine redundant code from different search strategies
        match top_docs_mode {
            TopDocsMode::Rating => {
                let collector =
                    TopDocs::with_limit(limit).order_by_u64_field(self.fields.total_rating);
                searcher.search(&search_query, &collector)?;
                let top_docs = searcher.search(&search_query, &collector)?;
                let mut entries = Vec::with_capacity(top_docs.len());
                for (_, doc_addr) in top_docs {
                    match searcher.doc(doc_addr) {
                        Ok(ref doc) => {
                            entries.push(self.fields.read_indexed_place(doc));
                        }
                        Err(err) => {
                            warn!("Failed to load document {:?}: {}", doc_addr, err);
                        }
                    }
                }
                Ok(entries)
            }
            TopDocsMode::ScoreBoostedByRating => {
                let collector = {
                    let total_rating_field = self.fields.total_rating;
                    TopDocs::with_limit(limit).tweak_score(move |segment_reader: &SegmentReader| {
                        let total_rating_reader = segment_reader
                            .fast_fields()
                            .u64(total_rating_field)
                            .unwrap();

                        move |doc: DocId, original_score: Score| {
                            let total_rating =
                                f64::from(u64_to_avg_rating(total_rating_reader.get(doc)));
                            let boost_factor =
                                if total_rating < f64::from(AvgRatingValue::default()) {
                                    // Negative ratings result in a boost factor < 1
                                    (total_rating - f64::from(AvgRatingValue::min()))
                                        / (f64::from(AvgRatingValue::default())
                                            - f64::from(AvgRatingValue::min()))
                                } else {
                                    // Default rating results in a boost factor of 1
                                    // Positive ratings result in a boost factor > 1
                                    // The total rating is scaled by the number of different rating context
                                    // variants to achieve better results by emphasizing the rating factor.
                                    1.0 + f64::from(RatingContext::total_count())
                                        * (total_rating - f64::from(AvgRatingValue::default()))
                                };
                            // Transform the original score by log2() to narrow the range. Otherwise
                            // the rating boost factor is not powerful enough to promote highly
                            // rated entries over entries that received a much higher score.
                            debug_assert!(original_score >= 0.0);
                            let unboosted_score = (1.0 + original_score).log2();
                            unboosted_score * (boost_factor as f32)
                        }
                    })
                };
                let top_docs = searcher.search(&search_query, &collector)?;
                let mut entries = Vec::with_capacity(top_docs.len());
                for (_, doc_addr) in top_docs {
                    match searcher.doc(doc_addr) {
                        Ok(ref doc) => {
                            entries.push(self.fields.read_indexed_place(doc));
                        }
                        Err(err) => {
                            warn!("Failed to load document {:?}: {}", doc_addr, err);
                        }
                    }
                }
                Ok(entries)
            }
        }
    }
}

#[derive(Clone)]
pub struct SearchEngine(Arc<Mutex<Box<dyn PlaceIndexer + Send>>>);

impl SearchEngine {
    pub fn init_in_ram() -> Fallible<SearchEngine> {
        let entry_index = TantivyPlaceIndex::create_in_ram()?;
        Ok(SearchEngine(Arc::new(Mutex::new(Box::new(entry_index)))))
    }

    pub fn init_with_path<P: AsRef<Path>>(path: Option<P>) -> Fallible<SearchEngine> {
        let entry_index = TantivyPlaceIndex::create(path)?;
        Ok(SearchEngine(Arc::new(Mutex::new(Box::new(entry_index)))))
    }
}

impl PlaceIndex for SearchEngine {
    fn query_places(&self, query: &IndexQuery, limit: usize) -> Fallible<Vec<IndexedPlace>> {
        let entry_index = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        entry_index.query_places(query, limit)
    }
}

impl PlaceIndexer for SearchEngine {
    fn add_or_update_place(&mut self, place: &Place, ratings: &AvgRatings) -> Fallible<()> {
        let mut inner = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.add_or_update_place(place, ratings)
    }

    fn remove_place_by_id(&mut self, id: &str) -> Fallible<()> {
        let mut inner = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.remove_place_by_id(id)
    }

    fn flush(&mut self) -> Fallible<()> {
        let mut inner = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.flush()
    }
}
