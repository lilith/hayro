//! Pluggable image decompressor support.
//!
//! This module defines the [`ImageDecompressor`] trait and [`DecompressorRegistry`],
//! allowing callers to replace the built-in image decoders (JPEG, JPEG 2000, JBIG2,
//! CCITT) with custom implementations.

use crate::filter::Filter;
use crate::object::stream::ImageColorSpace;
use crate::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;

/// Output from an image decompressor.
pub struct ImageDecompressOutput {
    /// The decoded pixel data.
    pub pixels: Vec<u8>,
    /// An optional alpha channel, separate from the pixel data.
    pub alpha: Option<Vec<u8>>,
    /// The color space of the decoded pixels.
    pub color_space: ImageColorSpace,
    /// The width of the decoded image in pixels.
    pub width: u32,
    /// The height of the decoded image in pixels.
    pub height: u32,
    /// The bits per component of the decoded image.
    pub bits_per_component: u8,
}

/// Context hints from the PDF container for the decompressor.
///
/// These values come from the image dictionary and decode parameters
/// in the PDF, providing the decompressor with information it may
/// need to correctly interpret the compressed data.
pub struct ImageDecompressContext {
    /// The width declared in the image dictionary.
    pub expected_width: u32,
    /// The height declared in the image dictionary.
    pub expected_height: u32,
    /// The number of color components, if known from the color space.
    pub expected_components: Option<u8>,
    /// The color transform hint (from DCTDecode parameters).
    pub color_transform: Option<u8>,
    /// The color space hint from the image dictionary.
    pub color_space_hint: Option<ImageColorSpace>,
    /// The bits per component from the image dictionary.
    pub bits_per_component: Option<u8>,
    /// Whether the color space is an indexed color space.
    pub is_indexed: bool,
    /// A target resolution hint for the image (width, height).
    pub target_dimension: Option<(u32, u32)>,
}

/// Trait for pluggable image decompressors (JPEG, JPEG 2000, JBIG2, CCITT, etc.).
///
/// Implement this trait to provide a custom decoder for one or more image filter
/// types. Register implementations via [`DecompressorRegistry`].
#[cfg(feature = "std")]
pub trait ImageDecompressor: Send + Sync {
    /// Decompress the given data using the provided context hints.
    ///
    /// Returns `None` if decompression fails, in which case the built-in
    /// decoder (if any) will NOT be used as a fallback.
    fn decompress(
        &self,
        data: &[u8],
        ctx: &ImageDecompressContext,
    ) -> Option<ImageDecompressOutput>;
}

/// Trait for pluggable image decompressors (JPEG, JPEG 2000, JBIG2, CCITT, etc.).
///
/// Implement this trait to provide a custom decoder for one or more image filter
/// types. Register implementations via [`DecompressorRegistry`].
#[cfg(not(feature = "std"))]
pub trait ImageDecompressor {
    /// Decompress the given data using the provided context hints.
    ///
    /// Returns `None` if decompression fails, in which case the built-in
    /// decoder (if any) will NOT be used as a fallback.
    fn decompress(
        &self,
        data: &[u8],
        ctx: &ImageDecompressContext,
    ) -> Option<ImageDecompressOutput>;
}

/// Registry mapping [`Filter`] variants to custom [`ImageDecompressor`] implementations.
///
/// When a filter has a registered decompressor, it will be used instead of the
/// built-in decoder. Filters without overrides fall through to the built-in
/// decoders as usual.
pub struct DecompressorRegistry {
    overrides: HashMap<Filter, Arc<dyn ImageDecompressor>>,
}

impl DecompressorRegistry {
    /// Create an empty registry with no overrides.
    pub fn new() -> Self {
        Self {
            overrides: HashMap::new(),
        }
    }

    /// Register a custom decompressor for a given filter type.
    ///
    /// This replaces any previously registered decompressor for that filter.
    pub fn register(&mut self, filter: Filter, decompressor: Arc<dyn ImageDecompressor>) {
        self.overrides.insert(filter, decompressor);
    }

    /// Look up a registered decompressor for the given filter.
    pub fn get(&self, filter: &Filter) -> Option<&Arc<dyn ImageDecompressor>> {
        self.overrides.get(filter)
    }
}

impl Default for DecompressorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
