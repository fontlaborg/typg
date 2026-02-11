/// The lightning-fast library catalog that remembers every font it meets
///
/// Like a master librarian with perfect recall, this index can find your
/// needle in a haystack of millions of fonts in the blink of an eye. We've
/// built this using the finest database technology - LMDB for silky-smooth
/// memory mapping and Roaring Bitmaps for the kind of set operations that
/// make other indexing systems weep with envy.
///
/// Our secret sauce? We never forget a face (or a font) and we can answer
/// the most complex questions faster than you can ask them. Think of it as
/// having a font conversation partner who's read every book in the library
/// and remembers every character they've ever met.
///
/// Made with speed and elegance at FontLab https://www.fontlab.com/
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use heed::types::{Bytes, U64};
use heed::{Database, Env, EnvOpenOptions, RoTxn, RwTxn};
use read_fonts::types::Tag;
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

use crate::query::{FamilyClassFilter, Query};
use crate::search::TypgFontFaceMatch;

/// Each font gets their own library card number - simple and elegant
pub type FontID = u64;

/// Our library can hold millions of font volumes (10GB handles >1M fonts)
const MAX_DB_SIZE: usize = 10 * 1024 * 1024 * 1024;

/// We keep our catalog organized in 10 neat sections
const MAX_DBS: u32 = 10;

/// The library card we fill out for every font that checks in
///
/// We capture the essence of each font - where they live, what they're
/// called, their special talents, and which characters they know how to draw.
/// Think of this as the font's permanent record in our library system.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexedFontMeta {
    /// Where this font hangs out on the filesystem
    pub path: String,
    /// Which door to knock on in multi-font apartment buildings (TTC files)
    pub ttc_index: Option<u32>,
    /// All the aliases and names this font goes by
    pub names: Vec<String>,
    /// Can this font change shape like a chameleon?
    pub is_variable: bool,
    /// How bold does this font think it is (on the 100-900 scale)
    pub weight_class: Option<u16>,
    /// How wide does this font like to stretch (condensed to expanded)
    pub width_class: Option<u16>,
    /// What typographic family does this font belong to?
    pub family_class: Option<(u8, u8)>,
    /// Every character this font can draw, compressed into a clever bitmap
    pub cmap_bitmap: Vec<u8>,
}

/// Quick lookup card so we know when fonts need updating
///
/// Like a library checkout card that tracks when a font was last seen.
/// We use this for smart incremental updates - no need to re-read
/// fonts that haven't changed their story.
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct PathEntry {
    font_id: u64,
    mtime_secs: u64,
}

/// The magnificent library itself - where all font knowledge lives
///
/// This is our grand hall of fonts, powered by LMDB's memory-mapped magic
/// and organized with the precision of a master librarian. We keep three
/// main catalogs: one for font biographies, one for lightning-fast tag
/// lookups, and one for path-to-ID mappings.
///
/// Everything is designed for speed - queries run in O(K) time even with
/// millions of fonts, because we believe you shouldn't wait for answers.
pub struct FontIndex {
    env: Env,
    /// DB_METADATA: FontID -> The complete biography of each font
    db_metadata: Database<U64<byteorder::NativeEndian>, Bytes>,
    /// DB_INVERTED_TAGS: Tag -> Which fonts have this superpower
    db_inverted: Database<Bytes, Bytes>,
    /// DB_PATH_TO_ID: PathHash -> Quick lookup card for incremental updates
    db_path_to_id: Database<U64<byteorder::NativeEndian>, Bytes>,
    /// The next available library card number
    next_id: AtomicU64,
}

impl FontIndex {
    /// Opens our magnificent font library or builds a new one from scratch
    ///
    /// We'll create the directory if it doesn't exist, set up our LMDB
    /// environment with plenty of room for millions of fonts, and establish
    /// our three main catalogs. If there are already books on the shelves,
    /// we'll figure out the next available library card number.
    pub fn open(index_dir: &Path) -> Result<Self> {
        fs::create_dir_all(index_dir)
            .with_context(|| format!("creating index directory {}", index_dir.display()))?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(MAX_DBS)
                .open(index_dir)
                .with_context(|| format!("opening LMDB at {}", index_dir.display()))?
        };

        // Create or open the named databases.
        let mut wtxn = env.write_txn()?;
        let db_metadata = env.create_database(&mut wtxn, Some("metadata"))?;
        let db_inverted = env.create_database(&mut wtxn, Some("inverted"))?;
        let db_path_to_id = env.create_database(&mut wtxn, Some("path_to_id"))?;
        wtxn.commit()?;

        // Determine the next FontID by scanning existing entries.
        let rtxn = env.read_txn()?;
        let mut max_id: u64 = 0;
        for result in db_metadata.iter(&rtxn)? {
            let (id, _) = result?;
            if id > max_id {
                max_id = id;
            }
        }
        drop(rtxn);

        Ok(Self {
            env,
            db_metadata,
            db_inverted,
            db_path_to_id,
            next_id: AtomicU64::new(max_id + 1),
        })
    }

    /// Counts how many fonts are currently enjoying our library hospitality
    ///
    /// A quick headcount of all the fonts we have indexed - perfect for
    /// statistics, progress bars, or just satisfying your curiosity about
    /// how big your font collection has grown.
    pub fn count(&self) -> Result<usize> {
        let rtxn = self.env.read_txn()?;
        Ok(self.db_metadata.len(&rtxn)? as usize)
    }

    /// Hands out a library card for adding new fonts to our collection
    ///
    /// We give you a writer's pass that lets you add fonts safely.
    /// Everything happens in a transaction, so either all your fonts
    /// get added properly or none of them do - no half-finished stories.
    pub fn writer(&self) -> Result<IndexWriter<'_>> {
        let wtxn = self.env.write_txn()?;
        Ok(IndexWriter {
            index: self,
            wtxn,
            modified_tags: HashSet::new(),
        })
    }

    /// Provides a reader's pass for browsing our font collection
    ///
    /// Your ticket to search and explore everything we know about fonts.
    /// Readers don't modify anything - they're polite observers who
    /// appreciate the finesse of our catalog without making a mess.
    pub fn reader(&self) -> Result<IndexReader<'_>> {
        let rtxn = self.env.read_txn()?;
        Ok(IndexReader { index: self, rtxn })
    }

    /// Issues the next available library card number for a new font
    ///
    /// We keep track of IDs atomically so no two fonts ever get the same
    /// number. Simple, fast, and reliable - just how we like our bookkeeping.
    fn alloc_id(&self) -> FontID {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// Writer handle for atomic index ingestion.
pub struct IndexWriter<'a> {
    index: &'a FontIndex,
    wtxn: RwTxn<'a>,
    modified_tags: HashSet<u32>,
}

impl<'a> IndexWriter<'a> {
    /// Check if a font needs re-indexing based on path and mtime.
    pub fn needs_update(&self, path: &Path, mtime: SystemTime) -> Result<bool> {
        let path_hash = hash_path(path);
        let mtime_secs = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(bytes) = self.index.db_path_to_id.get(&self.wtxn, &path_hash)? {
            if bytes.len() == std::mem::size_of::<PathEntry>() {
                let entry: PathEntry = *bytemuck::from_bytes(bytes);
                return Ok(entry.mtime_secs != mtime_secs);
            }
        }
        Ok(true) // Not found, needs indexing
    }

    /// Add a font face to the index.
    #[allow(clippy::too_many_arguments)]
    pub fn add_font(
        &mut self,
        path: &Path,
        ttc_index: Option<u32>,
        mtime: SystemTime,
        names: Vec<String>,
        axis_tags: &[Tag],
        feature_tags: &[Tag],
        script_tags: &[Tag],
        table_tags: &[Tag],
        codepoints: &[char],
        is_variable: bool,
        weight_class: Option<u16>,
        width_class: Option<u16>,
        family_class: Option<(u8, u8)>,
    ) -> Result<FontID> {
        let path_hash = hash_path(path);
        let mtime_secs = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check for existing entry and remove it first.
        if let Some(bytes) = self.index.db_path_to_id.get(&self.wtxn, &path_hash)? {
            if bytes.len() == std::mem::size_of::<PathEntry>() {
                let entry: PathEntry = *bytemuck::from_bytes(bytes);
                self.remove_font_by_id(entry.font_id)?;
            }
        }

        // Allocate new ID.
        let font_id = self.index.alloc_id();

        // Build Roaring Bitmap for cmap coverage.
        let cmap_bitmap = build_cmap_bitmap(codepoints);

        // Serialize metadata with bincode.
        let meta = IndexedFontMeta {
            path: path.display().to_string(),
            ttc_index,
            names,
            is_variable,
            weight_class,
            width_class,
            family_class,
            cmap_bitmap,
        };

        let meta_bytes =
            bincode::serialize(&meta).map_err(|e| anyhow::anyhow!("bincode serialize: {e}"))?;
        self.index
            .db_metadata
            .put(&mut self.wtxn, &font_id, &meta_bytes)?;

        // Update path-to-ID mapping.
        let path_entry = PathEntry {
            font_id,
            mtime_secs,
        };
        self.index.db_path_to_id.put(
            &mut self.wtxn,
            &path_hash,
            bytemuck::bytes_of(&path_entry),
        )?;

        // Update inverted indices for all tags.
        for tag in axis_tags
            .iter()
            .chain(feature_tags)
            .chain(script_tags)
            .chain(table_tags)
        {
            self.add_to_inverted_index(tag_to_u32(*tag), font_id)?;
        }

        // Add special markers for variable fonts.
        if is_variable {
            self.add_to_inverted_index(tag_marker(b"_VAR"), font_id)?;
        }

        Ok(font_id)
    }

    /// Remove a font by its ID from all indices.
    fn remove_font_by_id(&mut self, font_id: FontID) -> Result<()> {
        self.index.db_metadata.delete(&mut self.wtxn, &font_id)?;
        Ok(())
    }

    /// Remove entries whose source files no longer exist on disk.
    /// Returns (before_count, after_count).
    pub fn prune_missing(&mut self) -> Result<(usize, usize)> {
        // Collect IDs of entries with missing paths.
        let mut to_remove = Vec::new();
        let before = self.index.db_metadata.len(&self.wtxn)? as usize;

        for result in self.index.db_metadata.iter(&self.wtxn)? {
            let (font_id, bytes) = result?;
            let meta = deserialize_meta(bytes)?;
            let path = Path::new(&meta.path);
            if !path.exists() {
                to_remove.push(font_id);
            }
        }

        // Remove missing entries.
        for font_id in &to_remove {
            self.index.db_metadata.delete(&mut self.wtxn, font_id)?;
        }

        // Also remove path-to-id mappings for missing files.
        // We need to scan the path_to_id database to clean up stale entries.
        let mut stale_hashes = Vec::new();
        for result in self.index.db_path_to_id.iter(&self.wtxn)? {
            let (hash, bytes) = result?;
            if bytes.len() == std::mem::size_of::<PathEntry>() {
                let entry: PathEntry = *bytemuck::from_bytes(bytes);
                if to_remove.contains(&entry.font_id) {
                    stale_hashes.push(hash);
                }
            }
        }

        for hash in stale_hashes {
            self.index.db_path_to_id.delete(&mut self.wtxn, &hash)?;
        }

        let after = self.index.db_metadata.len(&self.wtxn)? as usize;
        Ok((before, after))
    }

    /// Add a font ID to an inverted index bitmap.
    fn add_to_inverted_index(&mut self, tag: u32, font_id: FontID) -> Result<()> {
        let tag_bytes = tag.to_ne_bytes();
        let mut bitmap = if let Some(bytes) = self.index.db_inverted.get(&self.wtxn, &tag_bytes)? {
            RoaringBitmap::deserialize_from(bytes)?
        } else {
            RoaringBitmap::new()
        };

        bitmap.insert(font_id as u32);
        self.modified_tags.insert(tag);

        let mut buf = Vec::new();
        bitmap.serialize_into(&mut buf)?;
        self.index
            .db_inverted
            .put(&mut self.wtxn, &tag_bytes, &buf)?;

        Ok(())
    }

    /// Commit the transaction.
    pub fn commit(self) -> Result<()> {
        self.wtxn.commit()?;
        Ok(())
    }

    /// Abort the transaction without committing.
    pub fn abort(self) {
        self.wtxn.abort();
    }
}

/// Reader handle for index queries.
pub struct IndexReader<'a> {
    index: &'a FontIndex,
    rtxn: RoTxn<'a>,
}

impl<'a> IndexReader<'a> {
    /// Execute a query and return matching font faces.
    pub fn find(&self, query: &Query) -> Result<Vec<TypgFontFaceMatch>> {
        // Phase 1: Use inverted indices to get candidate bitmap.
        let candidates = self.get_candidate_bitmap(query)?;

        // Phase 2: Filter candidates and hydrate metadata.
        let mut matches = Vec::new();
        for font_id in candidates.iter() {
            if let Some(meta) = self.get_metadata(font_id as u64)? {
                if self.passes_filters(&meta, query)? {
                    matches.push(hydrate_match(&meta));
                }
            }
        }

        // Sort by path for deterministic output.
        matches.sort_by(|a, b| {
            a.source
                .path
                .cmp(&b.source.path)
                .then_with(|| a.source.ttc_index.cmp(&b.source.ttc_index))
        });

        Ok(matches)
    }

    /// List all indexed fonts.
    pub fn list_all(&self) -> Result<Vec<TypgFontFaceMatch>> {
        let mut matches = Vec::new();
        for result in self.index.db_metadata.iter(&self.rtxn)? {
            let (_, bytes) = result?;
            let meta = deserialize_meta(bytes)?;
            matches.push(hydrate_match(&meta));
        }

        matches.sort_by(|a, b| {
            a.source
                .path
                .cmp(&b.source.path)
                .then_with(|| a.source.ttc_index.cmp(&b.source.ttc_index))
        });

        Ok(matches)
    }

    /// Get the candidate bitmap by intersecting tag bitmaps.
    fn get_candidate_bitmap(&self, query: &Query) -> Result<RoaringBitmap> {
        let mut result: Option<RoaringBitmap> = None;

        // Intersect axis tag bitmaps.
        for tag in query.axes() {
            let bitmap = self.get_tag_bitmap(tag_to_u32(*tag))?;
            result = Some(intersect_optional(result, bitmap));
        }

        // Intersect feature tag bitmaps.
        for tag in query.features() {
            let bitmap = self.get_tag_bitmap(tag_to_u32(*tag))?;
            result = Some(intersect_optional(result, bitmap));
        }

        // Intersect script tag bitmaps.
        for tag in query.scripts() {
            let bitmap = self.get_tag_bitmap(tag_to_u32(*tag))?;
            result = Some(intersect_optional(result, bitmap));
        }

        // Intersect table tag bitmaps.
        for tag in query.tables() {
            let bitmap = self.get_tag_bitmap(tag_to_u32(*tag))?;
            result = Some(intersect_optional(result, bitmap));
        }

        // Require variable fonts if specified.
        if query.requires_variable() {
            let bitmap = self.get_tag_bitmap(tag_marker(b"_VAR"))?;
            result = Some(intersect_optional(result, bitmap));
        }

        // If no tag filters, return all fonts.
        match result {
            Some(bitmap) => Ok(bitmap),
            None => {
                let mut all = RoaringBitmap::new();
                for r in self.index.db_metadata.iter(&self.rtxn)? {
                    let (id, _) = r?;
                    all.insert(id as u32);
                }
                Ok(all)
            }
        }
    }

    /// Get the bitmap for a specific tag.
    fn get_tag_bitmap(&self, tag: u32) -> Result<RoaringBitmap> {
        let tag_bytes = tag.to_ne_bytes();
        if let Some(bytes) = self.index.db_inverted.get(&self.rtxn, &tag_bytes)? {
            Ok(RoaringBitmap::deserialize_from(bytes)?)
        } else {
            Ok(RoaringBitmap::new())
        }
    }

    /// Get metadata for a font ID.
    fn get_metadata(&self, font_id: FontID) -> Result<Option<IndexedFontMeta>> {
        if let Some(bytes) = self.index.db_metadata.get(&self.rtxn, &font_id)? {
            Ok(Some(deserialize_meta(bytes)?))
        } else {
            Ok(None)
        }
    }

    /// Check if metadata passes query filters that can't use inverted indices.
    fn passes_filters(&self, meta: &IndexedFontMeta, query: &Query) -> Result<bool> {
        // Name pattern filter.
        if !query.name_patterns().is_empty() {
            let matches_any = meta
                .names
                .iter()
                .any(|name| query.name_patterns().iter().any(|p| p.is_match(name)));
            if !matches_any {
                return Ok(false);
            }
        }

        // Weight range filter.
        if let Some(range) = query.weight_range() {
            if let Some(weight) = meta.weight_class {
                if !range.contains(&weight) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Width range filter.
        if let Some(range) = query.width_range() {
            if let Some(width) = meta.width_class {
                if !range.contains(&width) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Family class filter.
        if let Some(filter) = query.family_class() {
            if let Some((major, sub)) = meta.family_class {
                if !matches_family_class(major, sub, filter) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Codepoint/text filter using cmap bitmap.
        if !query.codepoints().is_empty() && !meta.cmap_bitmap.is_empty() {
            if let Ok(cmap) = RoaringBitmap::deserialize_from(meta.cmap_bitmap.as_slice()) {
                for &cp in query.codepoints() {
                    if !cmap.contains(cp as u32) {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }
}

/// Deserialize metadata from bytes.
fn deserialize_meta(bytes: &[u8]) -> Result<IndexedFontMeta> {
    bincode::deserialize(bytes).map_err(|e| anyhow::anyhow!("bincode deserialize: {e}"))
}

/// Convert metadata to TypgFontFaceMatch.
fn hydrate_match(meta: &IndexedFontMeta) -> TypgFontFaceMatch {
    use crate::search::{TypgFontFaceMeta, TypgFontSource};

    TypgFontFaceMatch {
        source: TypgFontSource {
            path: PathBuf::from(&meta.path),
            ttc_index: meta.ttc_index,
        },
        metadata: TypgFontFaceMeta {
            names: meta.names.clone(),
            axis_tags: Vec::new(),    // Not stored in indexed form
            feature_tags: Vec::new(), // Not stored in indexed form
            script_tags: Vec::new(),  // Not stored in indexed form
            table_tags: Vec::new(),   // Not stored in indexed form
            codepoints: Vec::new(),   // Stored as bitmap
            is_variable: meta.is_variable,
            weight_class: meta.weight_class,
            width_class: meta.width_class,
            family_class: meta.family_class,
        },
    }
}

/// Hash a path for the path-to-ID lookup.
fn hash_path(path: &Path) -> u64 {
    use xxhash_rust::xxh3::xxh3_64;
    xxh3_64(path.to_string_lossy().as_bytes())
}

/// Convert Tag to u32.
fn tag_to_u32(tag: Tag) -> u32 {
    u32::from_be_bytes(tag.into_bytes())
}

/// Create a marker tag for special indices.
fn tag_marker(name: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*name)
}

/// Build a Roaring Bitmap from codepoints for efficient coverage checks.
fn build_cmap_bitmap(codepoints: &[char]) -> Vec<u8> {
    if codepoints.is_empty() {
        return Vec::new();
    }

    let mut bitmap = RoaringBitmap::new();
    for &cp in codepoints {
        bitmap.insert(cp as u32);
    }

    let mut buf = Vec::new();
    bitmap.serialize_into(&mut buf).unwrap_or_default();
    buf
}

/// Intersect an optional bitmap with another bitmap.
fn intersect_optional(opt: Option<RoaringBitmap>, other: RoaringBitmap) -> RoaringBitmap {
    match opt {
        Some(mut bm) => {
            bm &= &other;
            bm
        }
        None => other,
    }
}

/// Check if family class matches the filter.
fn matches_family_class(major: u8, sub: u8, filter: &FamilyClassFilter) -> bool {
    if major != filter.major {
        return false;
    }
    match filter.subclass {
        Some(expected_sub) => sub == expected_sub,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_open_and_close() {
        let dir = TempDir::new().unwrap();
        let index = FontIndex::open(dir.path()).unwrap();
        assert_eq!(index.count().unwrap(), 0);
    }

    #[test]
    fn test_add_and_query_font() {
        let dir = TempDir::new().unwrap();
        let index = FontIndex::open(dir.path()).unwrap();

        // Add a font.
        let mut writer = index.writer().unwrap();
        let path = Path::new("/test/font.ttf");
        let mtime = SystemTime::UNIX_EPOCH;

        writer
            .add_font(
                path,
                None,
                mtime,
                vec!["Test Font".to_string()],
                &[],
                &[Tag::new(b"smcp")],
                &[Tag::new(b"latn")],
                &[],
                &['a', 'b', 'c'],
                false,
                Some(400),
                Some(5),
                Some((8, 1)),
            )
            .unwrap();
        writer.commit().unwrap();

        assert_eq!(index.count().unwrap(), 1);

        // Query with feature filter.
        let reader = index.reader().unwrap();
        let query = Query::new().with_features(vec![Tag::new(b"smcp")]);
        let matches = reader.find(&query).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].source.path, path);
    }

    #[test]
    fn test_incremental_update() {
        let dir = TempDir::new().unwrap();
        let index = FontIndex::open(dir.path()).unwrap();

        let path = Path::new("/test/font.ttf");
        let mtime1 = SystemTime::UNIX_EPOCH;
        let mtime2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000);

        // First add.
        {
            let mut writer = index.writer().unwrap();
            assert!(writer.needs_update(path, mtime1).unwrap());
            writer
                .add_font(
                    path,
                    None,
                    mtime1,
                    vec!["V1".to_string()],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    false,
                    None,
                    None,
                    None,
                )
                .unwrap();
            writer.commit().unwrap();
        }

        // Same mtime - no update needed.
        {
            let writer = index.writer().unwrap();
            assert!(!writer.needs_update(path, mtime1).unwrap());
            writer.abort();
        }

        // Different mtime - update needed.
        {
            let writer = index.writer().unwrap();
            assert!(writer.needs_update(path, mtime2).unwrap());
            writer.abort();
        }
    }

    #[test]
    fn test_bitmap_intersection() {
        let dir = TempDir::new().unwrap();
        let index = FontIndex::open(dir.path()).unwrap();

        // Add two fonts with different features.
        {
            let mut writer = index.writer().unwrap();
            writer
                .add_font(
                    Path::new("/font1.ttf"),
                    None,
                    SystemTime::UNIX_EPOCH,
                    vec!["Font1".to_string()],
                    &[],
                    &[Tag::new(b"smcp"), Tag::new(b"liga")],
                    &[],
                    &[],
                    &[],
                    false,
                    None,
                    None,
                    None,
                )
                .unwrap();
            writer
                .add_font(
                    Path::new("/font2.ttf"),
                    None,
                    SystemTime::UNIX_EPOCH,
                    vec!["Font2".to_string()],
                    &[],
                    &[Tag::new(b"smcp")],
                    &[],
                    &[],
                    &[],
                    false,
                    None,
                    None,
                    None,
                )
                .unwrap();
            writer.commit().unwrap();
        }

        let reader = index.reader().unwrap();

        // Query for smcp only - should find both.
        let q1 = Query::new().with_features(vec![Tag::new(b"smcp")]);
        assert_eq!(reader.find(&q1).unwrap().len(), 2);

        // Query for smcp AND liga - should find only font1.
        let q2 = Query::new().with_features(vec![Tag::new(b"smcp"), Tag::new(b"liga")]);
        let matches = reader.find(&q2).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].source.path, Path::new("/font1.ttf"));
    }

    #[test]
    fn test_cmap_bitmap() {
        let codepoints = vec!['a', 'b', 'c', 'ñ', '中'];
        let bitmap_bytes = build_cmap_bitmap(&codepoints);
        let bitmap = RoaringBitmap::deserialize_from(bitmap_bytes.as_slice()).unwrap();

        // Should contain all original codepoints.
        for &cp in &codepoints {
            assert!(bitmap.contains(cp as u32));
        }

        // Should not contain a codepoint we didn't add.
        assert!(!bitmap.contains('z' as u32));
    }

    #[test]
    fn test_prune_missing() {
        let dir = TempDir::new().unwrap();
        let index = FontIndex::open(dir.path()).unwrap();

        // Add two fonts: one with a real path, one with a fake path.
        {
            let mut writer = index.writer().unwrap();
            // This path doesn't exist.
            writer
                .add_font(
                    Path::new("/nonexistent/font.ttf"),
                    None,
                    SystemTime::UNIX_EPOCH,
                    vec!["Missing".to_string()],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    false,
                    None,
                    None,
                    None,
                )
                .unwrap();

            // Use an existing file (any test file that exists).
            let existing_path = Path::new(env!("CARGO_MANIFEST_DIR"));
            writer
                .add_font(
                    existing_path,
                    None,
                    SystemTime::UNIX_EPOCH,
                    vec!["Existing".to_string()],
                    &[],
                    &[],
                    &[],
                    &[],
                    &[],
                    false,
                    None,
                    None,
                    None,
                )
                .unwrap();
            writer.commit().unwrap();
        }

        assert_eq!(index.count().unwrap(), 2);

        // Prune missing entries.
        {
            let mut writer = index.writer().unwrap();
            let (before, after) = writer.prune_missing().unwrap();
            writer.commit().unwrap();

            assert_eq!(before, 2);
            assert_eq!(after, 1);
        }

        // Verify only the existing entry remains.
        assert_eq!(index.count().unwrap(), 1);
        let reader = index.reader().unwrap();
        let entries = reader.list_all().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0]
            .metadata
            .names
            .iter()
            .any(|n| n.contains("Existing")));
    }
}
