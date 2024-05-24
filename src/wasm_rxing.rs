use std::collections::HashMap;

use rxing::{self, Exceptions};

/// Decode a barcode from an array of 8bit luma data
pub(crate) fn decode_barcode(
    data: Vec<u8>,
    width: u32,
    height: u32,
    try_harder: Option<bool>,
    filter_image: Option<bool>,
) -> Result<rxing::RXingResult, Exceptions> {
    let mut hints: rxing::DecodingHintDictionary = HashMap::new();
    if let Some(true) = try_harder {
        hints.insert(
            rxing::DecodeHintType::TRY_HARDER,
            rxing::DecodeHintValue::TryHarder(true),
        );
    }

    let detection_function = if matches!(filter_image, Some(true)) {
        rxing::helpers::detect_in_luma_filtered_with_hints
    } else {
        rxing::helpers::detect_in_luma_with_hints
    };

    detection_function(data, width, height, None, &mut hints)
}

/// Convert a javascript image context's data into luma 8.
///
/// Data for this function can be found from any canvas object
/// using the `data` property of an `ImageData` object.
/// Such an object could be obtained using the `getImageData`
/// method of a `CanvasRenderingContext2D` object.
pub(crate) fn convert_js_image_to_luma(data: &[u8]) -> Vec<u8> {
    let mut luma_data = Vec::with_capacity(data.len() / 4);
    for src_pixel in data.chunks_exact(4) {
        let [red, green, blue, alpha] = src_pixel else {
            continue;
        };
        let pixel = if *alpha == 0 {
            // white, so we know its luminance is 255
            0xFF
        } else {
            // .299R + 0.587G + 0.114B (YUV/YIQ for PAL and NTSC),
            // (306*R) >> 10 is approximately equal to R*0.299, and so on.
            // 0x200 >> 10 is 0.5, it implements rounding.

            ((306 * (*red as u64) + 601 * (*green as u64) + 117 * (*blue as u64) + 0x200) >> 10)
                as u8
        };
        luma_data.push(pixel);
    }

    luma_data
}
