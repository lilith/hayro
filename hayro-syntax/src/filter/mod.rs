//! Results from decoding filtered data streams.

mod ascii_85;
pub(crate) mod ascii_hex;
#[cfg(feature = "images")]
mod ccitt;
#[cfg(feature = "images")]
mod dct;
#[cfg(feature = "images")]
mod jbig2;
#[cfg(feature = "images")]
mod jpx;
mod lzw_flate;
/// Pluggable image decompressor support.
pub mod pluggable;
mod run_length;

use crate::object::Dict;
use crate::object::Name;
use crate::object::dict::keys::*;
use crate::object::stream::{DecodeFailure, FilterResult, ImageData, ImageDecodeParams};
use alloc::borrow::Cow;
use core::ops::Deref;
use pluggable::{DecompressorRegistry, ImageDecompressContext, ImageDecompressOutput};

/// A data filter.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Filter {
    /// ASCII hexadecimal encoding.
    AsciiHexDecode,
    /// ASCII base-85 encoding.
    Ascii85Decode,
    /// Lempel-Ziv-Welch (LZW) compression.
    LzwDecode,
    /// DEFLATE compression (zlib/gzip).
    FlateDecode,
    /// Run-length encoding compression.
    RunLengthDecode,
    /// CCITT Group 3 or Group 4 fax compression.
    CcittFaxDecode,
    /// JBIG2 compression for bi-level images.
    Jbig2Decode,
    /// JPEG (DCT) compression.
    DctDecode,
    /// JPEG 2000 compression.
    JpxDecode,
    /// Encryption filter.
    Crypt,
}

impl Filter {
    fn debug_name(&self) -> &'static str {
        match self {
            Self::AsciiHexDecode => "ascii_hex",
            Self::Ascii85Decode => "ascii_85",
            Self::LzwDecode => "lzw",
            Self::FlateDecode => "flate",
            Self::RunLengthDecode => "run-length",
            Self::CcittFaxDecode => "ccit_fax",
            Self::Jbig2Decode => "jbig2",
            Self::DctDecode => "dct",
            Self::JpxDecode => "jpx",
            Self::Crypt => "crypt",
        }
    }

    /// Whether this filter is an image decompression filter that can be
    /// overridden by a pluggable decompressor.
    fn is_image_filter(&self) -> bool {
        matches!(
            self,
            Self::DctDecode | Self::JpxDecode | Self::CcittFaxDecode | Self::Jbig2Decode
        )
    }

    pub(crate) fn from_name(name: Name<'_>) -> Option<Self> {
        match name.deref() {
            ASCII_HEX_DECODE | ASCII_HEX_DECODE_ABBREVIATION => Some(Self::AsciiHexDecode),
            ASCII85_DECODE | ASCII85_DECODE_ABBREVIATION => Some(Self::Ascii85Decode),
            LZW_DECODE | LZW_DECODE_ABBREVIATION => Some(Self::LzwDecode),
            FLATE_DECODE | FLATE_DECODE_ABBREVIATION => Some(Self::FlateDecode),
            RUN_LENGTH_DECODE | RUN_LENGTH_DECODE_ABBREVIATION => Some(Self::RunLengthDecode),
            CCITTFAX_DECODE | CCITTFAX_DECODE_ABBREVIATION => Some(Self::CcittFaxDecode),
            JBIG2_DECODE => Some(Self::Jbig2Decode),
            DCT_DECODE | DCT_DECODE_ABBREVIATION => Some(Self::DctDecode),
            JPX_DECODE => Some(Self::JpxDecode),
            CRYPT => Some(Self::Crypt),
            _ => {
                warn!("unknown filter: {}", name.as_str());

                None
            }
        }
    }

    pub(crate) fn apply(
        &self,
        data: &[u8],
        params: &Dict<'_>,
        #[cfg_attr(not(feature = "images"), allow(unused))] image_params: &ImageDecodeParams,
        registry: Option<&DecompressorRegistry>,
    ) -> Result<FilterResult<'static>, DecodeFailure> {
        // Check for a pluggable override before falling through to built-in decoders.
        if self.is_image_filter() {
            if let Some(reg) = registry {
                if let Some(decompressor) = reg.get(self) {
                    let ctx = ImageDecompressContext {
                        expected_width: image_params.width,
                        expected_height: image_params.height,
                        expected_components: image_params.num_components,
                        color_transform: params.get::<u8>(COLOR_TRANSFORM),
                        color_space_hint: None,
                        bits_per_component: image_params.bpc,
                        is_indexed: image_params.is_indexed,
                        target_dimension: image_params.target_dimension,
                    };

                    let result = decompressor
                        .decompress(data, &ctx)
                        .ok_or(DecodeFailure::ImageDecode);

                    if result.is_err() {
                        warn!(
                            "pluggable decompressor failed for filter {}",
                            self.debug_name()
                        );
                    }

                    return result.map(|output| output.into_filter_result());
                }
            }
        }

        let res = match self {
            Self::AsciiHexDecode => ascii_hex::decode(data)
                .map(FilterResult::from_data)
                .ok_or(DecodeFailure::StreamDecode),
            Self::Ascii85Decode => ascii_85::decode(data)
                .map(FilterResult::from_data)
                .ok_or(DecodeFailure::StreamDecode),
            Self::RunLengthDecode => run_length::decode(data)
                .map(FilterResult::from_data)
                .ok_or(DecodeFailure::StreamDecode),
            Self::LzwDecode => lzw_flate::lzw::decode(data, params)
                .map(FilterResult::from_data)
                .ok_or(DecodeFailure::StreamDecode),
            Self::FlateDecode => lzw_flate::flate::decode(data, params)
                .map(FilterResult::from_data)
                .ok_or(DecodeFailure::StreamDecode),
            #[cfg(feature = "images")]
            Self::DctDecode => {
                dct::decode(data, params, image_params).ok_or(DecodeFailure::ImageDecode)
            }
            #[cfg(feature = "images")]
            Self::CcittFaxDecode => {
                ccitt::decode(data, params, image_params).ok_or(DecodeFailure::ImageDecode)
            }
            #[cfg(feature = "images")]
            Self::Jbig2Decode => {
                jbig2::decode(data, params, image_params).ok_or(DecodeFailure::ImageDecode)
            }
            #[cfg(feature = "images")]
            Self::JpxDecode => jpx::decode(data, image_params).ok_or(DecodeFailure::ImageDecode),
            #[cfg(not(feature = "images"))]
            Self::DctDecode | Self::CcittFaxDecode | Self::Jbig2Decode | Self::JpxDecode => {
                warn!("image decoding is not supported (enable the `images` feature)");
                Err(DecodeFailure::ImageDecode)
            }
            _ => Err(DecodeFailure::StreamDecode),
        };

        if res.is_err() {
            warn!("failed to apply filter {}", self.debug_name());
        }

        res
    }
}

impl ImageDecompressOutput {
    /// Convert this output into a [`FilterResult`] for use in the filter pipeline.
    pub(crate) fn into_filter_result(self) -> FilterResult<'static> {
        FilterResult {
            data: Cow::Owned(self.pixels),
            image_data: Some(ImageData {
                alpha: self.alpha,
                color_space: Some(self.color_space),
                bits_per_component: self.bits_per_component,
                width: self.width,
                height: self.height,
            }),
        }
    }
}
