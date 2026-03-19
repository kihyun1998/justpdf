use std::collections::HashMap;
use tiny_skia::Path;

/// Cache key for a glyph path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphKey {
    /// Hash of the font data (to distinguish fonts).
    font_hash: u64,
    /// Glyph ID within the font.
    glyph_id: u16,
}

/// A cache for pre-built glyph paths to avoid re-parsing font data
/// and re-building paths for each character occurrence.
pub struct GlyphCache {
    paths: HashMap<GlyphKey, Option<Path>>,
    /// Maps font data pointer (as `usize`) to a fast hash, so we only
    /// hash each font's bytes once per unique allocation.
    font_hashes: HashMap<usize, u64>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

const DEFAULT_CAPACITY: usize = 4096;

impl GlyphCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            paths: HashMap::with_capacity(capacity),
            font_hashes: HashMap::new(),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Get or insert a glyph path.
    ///
    /// `font_data` is the raw font file bytes.
    /// `glyph_id` is the glyph index.
    /// `build_fn` is called on cache miss to build the path.
    pub fn get_or_insert(
        &mut self,
        font_data: &[u8],
        glyph_id: u16,
        build_fn: impl FnOnce() -> Option<Path>,
    ) -> Option<&Path> {
        let font_hash = self.font_hash(font_data);
        let key = GlyphKey {
            font_hash,
            glyph_id,
        };

        if self.paths.contains_key(&key) {
            self.hits += 1;
            return self.paths.get(&key).and_then(|opt| opt.as_ref());
        }

        self.misses += 1;

        // Evict all entries if we hit capacity (simple strategy).
        if self.paths.len() >= self.capacity {
            self.paths.clear();
            self.font_hashes.clear();
        }

        let path = build_fn();
        self.paths.insert(key.clone(), path);
        self.paths.get(&key).and_then(|opt| opt.as_ref())
    }

    /// Cache hit rate for debugging/profiling.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Clear the cache and reset statistics.
    pub fn clear(&mut self) {
        self.paths.clear();
        self.font_hashes.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Compute (or retrieve cached) hash for font data.
    ///
    /// Uses a fast hash of the first 256 bytes + the total length,
    /// which is sufficient to distinguish fonts without hashing
    /// megabytes of data. The hash is cached per font data pointer
    /// so repeated calls for the same allocation are free.
    fn font_hash(&mut self, font_data: &[u8]) -> u64 {
        let ptr = font_data.as_ptr() as usize;
        if let Some(&h) = self.font_hashes.get(&ptr) {
            return h;
        }
        let h = compute_font_hash(font_data);
        self.font_hashes.insert(ptr, h);
        h
    }
}

/// Fast, non-cryptographic hash of font data for cache keying.
/// Hashes the first 256 bytes + the total length.
fn compute_font_hash(data: &[u8]) -> u64 {
    // FNV-1a 64-bit
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

    let mut hash = FNV_OFFSET;

    // Mix in the length first
    let len_bytes = (data.len() as u64).to_le_bytes();
    for &b in &len_bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    // Hash up to 256 bytes of content
    let prefix_len = data.len().min(256);
    for &b in &data[..prefix_len] {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::PathBuilder;

    fn make_test_path(x: f32) -> Option<Path> {
        let mut pb = PathBuilder::new();
        pb.move_to(x, 0.0);
        pb.line_to(x + 10.0, 0.0);
        pb.line_to(x + 10.0, 10.0);
        pb.close();
        pb.finish()
    }

    fn fake_font_data(tag: u8) -> Vec<u8> {
        // Generate 300 bytes so we exercise the 256-byte prefix window
        let mut data = vec![tag; 300];
        // Vary the "font header" area
        data[0] = tag;
        data[4] = tag.wrapping_add(1);
        data
    }

    #[test]
    fn cache_hit_on_second_access() {
        let mut cache = GlyphCache::new(128);
        let font = fake_font_data(0xAA);

        // First access — miss
        let p = cache.get_or_insert(&font, 42, || make_test_path(0.0));
        assert!(p.is_some());
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 0);

        // Second access — hit
        let p = cache.get_or_insert(&font, 42, || {
            panic!("build_fn should not be called on cache hit")
        });
        assert!(p.is_some());
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn different_fonts_produce_different_entries() {
        let mut cache = GlyphCache::new(128);
        let font_a = fake_font_data(0xAA);
        let font_b = fake_font_data(0xBB);

        cache.get_or_insert(&font_a, 1, || make_test_path(0.0));
        cache.get_or_insert(&font_b, 1, || make_test_path(5.0));

        // Both should be misses (different fonts, same glyph_id)
        assert_eq!(cache.misses, 2);
        assert_eq!(cache.hits, 0);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn cache_miss_returns_built_path() {
        let mut cache = GlyphCache::new(128);
        let font = fake_font_data(0xCC);

        let p = cache.get_or_insert(&font, 10, || make_test_path(3.0));
        assert!(p.is_some());
        assert_eq!(cache.len(), 1);

        // None path should also be cached
        let p = cache.get_or_insert(&font, 99, || None);
        assert!(p.is_none());
        assert_eq!(cache.len(), 2);

        // Second access to the None entry should still be a hit
        let p = cache.get_or_insert(&font, 99, || {
            panic!("should not rebuild None entry")
        });
        assert!(p.is_none());
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn hit_rate_tracking() {
        let mut cache = GlyphCache::new(128);
        let font = fake_font_data(0xDD);

        assert_eq!(cache.hit_rate(), 0.0);

        cache.get_or_insert(&font, 1, || make_test_path(0.0)); // miss
        cache.get_or_insert(&font, 1, || make_test_path(0.0)); // hit
        cache.get_or_insert(&font, 1, || make_test_path(0.0)); // hit
        cache.get_or_insert(&font, 1, || make_test_path(0.0)); // hit

        // 3 hits / 4 total = 0.75
        assert!((cache.hit_rate() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn clear_empties_cache() {
        let mut cache = GlyphCache::new(128);
        let font = fake_font_data(0xEE);

        cache.get_or_insert(&font, 1, || make_test_path(0.0));
        cache.get_or_insert(&font, 2, || make_test_path(0.0));
        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn eviction_on_capacity() {
        let mut cache = GlyphCache::new(2);
        let font = fake_font_data(0xFF);

        cache.get_or_insert(&font, 1, || make_test_path(0.0));
        cache.get_or_insert(&font, 2, || make_test_path(0.0));
        assert_eq!(cache.len(), 2);

        // This should trigger eviction (capacity = 2)
        cache.get_or_insert(&font, 3, || make_test_path(0.0));
        // After eviction + insert, we should have 1 entry
        assert_eq!(cache.len(), 1);
    }
}
