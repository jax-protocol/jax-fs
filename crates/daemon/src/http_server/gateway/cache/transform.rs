use std::io::Cursor;

use image::ImageFormat;

/// Parsed and validated image transform parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransformParams {
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub q: Option<u8>,
}

const MAX_DIMENSION: u32 = 4096;
const DEFAULT_QUALITY: u8 = 80;

/// MIME types eligible for image transform.
const IMAGE_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

impl TransformParams {
    /// Parse from raw query params, returning None if no transform is requested.
    pub fn from_query(w: Option<u32>, h: Option<u32>, q: Option<u8>) -> Option<Self> {
        if w.is_none() && h.is_none() && q.is_none() {
            return None;
        }
        Some(Self { w, h, q })
    }

    /// Validate params. Returns an error message if invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Some(w) = self.w {
            if w == 0 {
                return Err("w must be > 0");
            }
            if w > MAX_DIMENSION {
                return Err("w exceeds maximum (4096)");
            }
        }
        if let Some(h) = self.h {
            if h == 0 {
                return Err("h must be > 0");
            }
            if h > MAX_DIMENSION {
                return Err("h exceeds maximum (4096)");
            }
        }
        if let Some(q) = self.q {
            if q == 0 || q > 100 {
                return Err("q must be 1-100");
            }
        }
        Ok(())
    }

    /// Whether this transform applies to the given MIME type.
    pub fn applies_to_mime(mime: &str) -> bool {
        IMAGE_MIMES.contains(&mime)
    }

    /// Serialize to a stable string key for cache indexing.
    pub fn to_cache_key(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.w {
            parts.push(format!("w={}", w));
        }
        if let Some(h) = self.h {
            parts.push(format!("h={}", h));
        }
        if let Some(q) = self.q {
            parts.push(format!("q={}", q));
        }
        parts.join("&")
    }
}

/// Transform an image: resize and/or adjust quality.
///
/// Output format matches input. Returns transformed bytes.
pub fn transform_image(
    data: &[u8],
    mime: &str,
    params: &TransformParams,
) -> Result<Vec<u8>, TransformError> {
    let format = mime_to_format(mime)?;

    let img = image::load_from_memory_with_format(data, format)
        .map_err(|e| TransformError::Decode(e.to_string()))?;

    // Apply resize if w or h is specified
    let img = if params.w.is_some() || params.h.is_some() {
        let (orig_w, orig_h) = (img.width(), img.height());

        let (new_w, new_h) = match (params.w, params.h) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let ratio = w as f64 / orig_w as f64;
                (w, (orig_h as f64 * ratio).round() as u32)
            }
            (None, Some(h)) => {
                let ratio = h as f64 / orig_h as f64;
                ((orig_w as f64 * ratio).round() as u32, h)
            }
            (None, None) => (orig_w, orig_h),
        };

        img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Encode back to the same format
    let quality = params.q.unwrap_or(DEFAULT_QUALITY);
    encode_image(&img, format, quality)
}

fn mime_to_format(mime: &str) -> Result<ImageFormat, TransformError> {
    match mime {
        "image/jpeg" => Ok(ImageFormat::Jpeg),
        "image/png" => Ok(ImageFormat::Png),
        "image/webp" => Ok(ImageFormat::WebP),
        "image/gif" => Ok(ImageFormat::Gif),
        _ => Err(TransformError::UnsupportedFormat(mime.to_string())),
    }
}

fn encode_image(
    img: &image::DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, TransformError> {
    let mut buf = Cursor::new(Vec::new());

    match format {
        ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| TransformError::Encode(e.to_string()))?;
        }
        ImageFormat::Png => {
            img.write_to(&mut buf, ImageFormat::Png)
                .map_err(|e| TransformError::Encode(e.to_string()))?;
        }
        ImageFormat::WebP => {
            img.write_to(&mut buf, ImageFormat::WebP)
                .map_err(|e| TransformError::Encode(e.to_string()))?;
        }
        ImageFormat::Gif => {
            img.write_to(&mut buf, ImageFormat::Gif)
                .map_err(|e| TransformError::Encode(e.to_string()))?;
        }
        _ => return Err(TransformError::UnsupportedFormat(format!("{:?}", format))),
    }

    Ok(buf.into_inner())
}

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("unsupported image format: {0}")]
    UnsupportedFormat(String),
    #[error("failed to decode image: {0}")]
    Decode(String),
    #[error("failed to encode image: {0}")]
    Encode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a small test JPEG in memory.
    fn make_test_jpeg(width: u32, height: u32) -> Vec<u8> {
        let img = image::DynamicImage::new_rgb8(width, height);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
        buf.into_inner()
    }

    /// Create a small test PNG in memory.
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::DynamicImage::new_rgba8(width, height);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn test_resize_width_only() {
        let jpeg = make_test_jpeg(800, 600);
        let params = TransformParams {
            w: Some(200),
            h: None,
            q: None,
        };
        let result = transform_image(&jpeg, "image/jpeg", &params).unwrap();
        let decoded = image::load_from_memory_with_format(&result, ImageFormat::Jpeg).unwrap();
        assert_eq!(decoded.width(), 200);
        assert_eq!(decoded.height(), 150); // maintains aspect ratio
    }

    #[test]
    fn test_resize_width_and_height() {
        let jpeg = make_test_jpeg(800, 600);
        let params = TransformParams {
            w: Some(200),
            h: Some(150),
            q: None,
        };
        let result = transform_image(&jpeg, "image/jpeg", &params).unwrap();
        let decoded = image::load_from_memory_with_format(&result, ImageFormat::Jpeg).unwrap();
        assert_eq!(decoded.width(), 200);
        assert_eq!(decoded.height(), 150);
    }

    #[test]
    fn test_quality_reduction() {
        let jpeg = make_test_jpeg(100, 100);
        let high_q = TransformParams {
            w: None,
            h: None,
            q: Some(95),
        };
        let low_q = TransformParams {
            w: None,
            h: None,
            q: Some(10),
        };
        let high = transform_image(&jpeg, "image/jpeg", &high_q).unwrap();
        let low = transform_image(&jpeg, "image/jpeg", &low_q).unwrap();
        assert!(
            low.len() <= high.len(),
            "lower quality should produce smaller output"
        );
    }

    #[test]
    fn test_png_resize() {
        let png = make_test_png(400, 300);
        let params = TransformParams {
            w: Some(200),
            h: None,
            q: None,
        };
        let result = transform_image(&png, "image/png", &params).unwrap();
        let decoded = image::load_from_memory_with_format(&result, ImageFormat::Png).unwrap();
        assert_eq!(decoded.width(), 200);
    }

    #[test]
    fn test_validate_params() {
        assert!(TransformParams {
            w: Some(200),
            h: None,
            q: None
        }
        .validate()
        .is_ok());
        assert!(TransformParams {
            w: Some(0),
            h: None,
            q: None
        }
        .validate()
        .is_err());
        assert!(TransformParams {
            w: Some(5000),
            h: None,
            q: None
        }
        .validate()
        .is_err());
        assert!(TransformParams {
            w: None,
            h: None,
            q: Some(0)
        }
        .validate()
        .is_err());
        assert!(TransformParams {
            w: None,
            h: None,
            q: Some(101)
        }
        .validate()
        .is_err());
        assert!(TransformParams {
            w: None,
            h: None,
            q: Some(80)
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn test_cache_key_stable() {
        let params = TransformParams {
            w: Some(200),
            h: Some(150),
            q: Some(75),
        };
        assert_eq!(params.to_cache_key(), "w=200&h=150&q=75");

        let params2 = TransformParams {
            w: Some(200),
            h: None,
            q: None,
        };
        assert_eq!(params2.to_cache_key(), "w=200");
    }

    #[test]
    fn test_applies_to_mime() {
        assert!(TransformParams::applies_to_mime("image/jpeg"));
        assert!(TransformParams::applies_to_mime("image/png"));
        assert!(TransformParams::applies_to_mime("image/webp"));
        assert!(TransformParams::applies_to_mime("image/gif"));
        assert!(!TransformParams::applies_to_mime("text/plain"));
        assert!(!TransformParams::applies_to_mime("application/pdf"));
    }

    #[test]
    fn test_unsupported_format() {
        let result = transform_image(
            b"not an image",
            "text/plain",
            &TransformParams {
                w: Some(100),
                h: None,
                q: None,
            },
        );
        assert!(result.is_err());
    }
}
