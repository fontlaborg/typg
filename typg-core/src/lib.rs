/// typg-core: The gentle detective of font discovery
///
/// Like a seasoned librarian who knows every book by heart, this library
/// helps you find fonts based on their hidden stories. It's the quiet hero
/// for font management systems that need to ask polite questions and get
/// thoughtful answers from massive font collections.
///
/// ## Three Acts of Font Discovery
///
/// **Discovery**: The gentle art of finding what's already there
/// - Befriends all font formats (TTF, OTF, TTC, OTC, WOFF, WOFF2)
/// - Handles font collections like a skilled orchestra conductor
/// - Processes collections in parallel, never making anyone wait too long
///
/// **Indexing**: Getting acquainted with every font's personality
/// - Remembers names, family secrets, and style preferences  
/// - Takes careful notes on variable font axes and their moods
/// - Catalogs OpenType features and script talents
/// - Classifies fonts by weight, width, and family tendencies
///
/// **Search**: Asking the right questions to find kindred spirits
/// - Matches names with the graceful accuracy of a matchmaker
/// - Finds fonts that speak specific languages and write specific scripts
/// - Filters by variable font capabilities like a talent scout
/// - Looks for special features hidden in the font's DNA
/// - Combines criteria with boolean logic that's more poetry than math
///
/// ## A Sample Conversation
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use typg_core::query::Query;
/// use typg_core::search::{search, SearchOptions};
/// use typg_core::tags::tag4;
///
/// // Let's find an Arabic font that's flexible like a dancer.
/// let query = Query::new()
///     .with_scripts(vec![tag4("arab").unwrap()])
///     .with_axes(vec![tag4("wght").unwrap()])
///     .require_variable(true);
///
/// // Where do we look for our font friends?
/// let font_dirs = vec![
///     PathBuf::from("/System/Library/Fonts"),
///     PathBuf::from("/Library/Fonts"), 
///     PathBuf::from("~/fonts"),
/// ];
///
/// let options = SearchOptions::default();
/// let results = search(&font_dirs, &query, &options)?;
///
/// println!("Found {} fonts that caught our eye:", results.len());
/// for font in results {
///     println!("  {} ({})", 
///         font.metadata.names.first().unwrap_or(&"<mysterious>".to_string()),
///         if font.metadata.is_variable { "adaptable" } else { "steady" }
///     );
/// }
/// #
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## The Art of Performance
///
/// - **Parallel Processing**: Uses Rayon like a well-coordinated kitchen staff
/// - **Cached Metadata**: Remembers what it learned, like a good friend
/// - **Memory Efficiency**: Takes small bites, never choking on the whole meal
/// - **Configurable Threading**: Lets you decide how many hands on deck
///
/// ## The Cast of Characters
///
/// - [`Query`]: Your diplomatic envoy to the font kingdom
/// - [`TypgFontFaceMeta`]: The comprehensive biography of every font
/// - [`TypgFontSource`]: Where each font calls home and how it dresses
/// - [`TypgFontFaceMatch`]: The perfect marriage of metadata and location
///
/// ## Playground Rules
///
/// Built on cross-platform font playground equipment (read-fonts, skrifa)
/// and plays nicely with all major operating systems. No proprietary 
/// gatekeeping - just pure metadata magic that works everywhere.
///
/// ## Making Friends
///
/// - Use with typg-python for Python conversations
/// - Pair with typg-index for database-backed social networking
/// - Cache your discoveries to avoid repeating stories
/// - Be mindful of memory when hosting large font parties
///
/// ---
///
/// Crafted with care at FontLab https://www.fontlab.com/

pub mod discovery;
#[cfg(feature = "hpindex")]
pub mod index;
pub mod output;
pub mod query;
pub mod search;
pub mod tags;
