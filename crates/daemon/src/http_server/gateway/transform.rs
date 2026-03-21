use std::io::Cursor;

use image::ImageFormat;
use mime::Mime;

/// Parsed and validated image transform parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransformParams {
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub q: Option<u8>,
}

const MAX_DIMENSION: u32 = 4096;
const DEFAULT_QUALITY: u8 = 80;

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

    /// Whether the given MIME type is eligible for image transform.
    pub fn is_transformable(mime: &Mime) -> bool {
        mime.type_() == mime::IMAGE
            && matches!(mime.subtype().as_str(), "jpeg" | "png" | "webp" | "gif")
    }

    /// Serialize to a stable query string for cache indexing.
    pub fn to_query_string(&self) -> String {
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
    mime: &Mime,
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

fn mime_to_format(mime: &Mime) -> Result<ImageFormat, TransformError> {
    match mime.subtype().as_str() {
        "jpeg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "webp" => Ok(ImageFormat::WebP),
        "gif" => Ok(ImageFormat::Gif),
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

    fn jpeg_mime() -> Mime {
        "image/jpeg".parse().unwrap()
    }

    fn png_mime() -> Mime {
        "image/png".parse().unwrap()
    }

    fn make_test_jpeg(width: u32, height: u32) -> Vec<u8> {
        let img = image::DynamicImage::new_rgb8(width, height);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Jpeg).unwrap();
        buf.into_inner()
    }

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
        let result = transform_image(&jpeg, &jpeg_mime(), &params).unwrap();
        let decoded = image::load_from_memory_with_format(&result, ImageFormat::Jpeg).unwrap();
        assert_eq!(decoded.width(), 200);
        assert_eq!(decoded.height(), 150);
    }

    #[test]
    fn test_resize_width_and_height() {
        let jpeg = make_test_jpeg(800, 600);
        let params = TransformParams {
            w: Some(200),
            h: Some(150),
            q: None,
        };
        let result = transform_image(&jpeg, &jpeg_mime(), &params).unwrap();
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
        let high = transform_image(&jpeg, &jpeg_mime(), &high_q).unwrap();
        let low = transform_image(&jpeg, &jpeg_mime(), &low_q).unwrap();
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
        let result = transform_image(&png, &png_mime(), &params).unwrap();
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
    }

    #[test]
    fn test_query_string_stable() {
        let params = TransformParams {
            w: Some(200),
            h: Some(150),
            q: Some(75),
        };
        assert_eq!(params.to_query_string(), "w=200&h=150&q=75");
    }

    #[test]
    fn test_is_transformable() {
        assert!(TransformParams::is_transformable(
            &"image/jpeg".parse().unwrap()
        ));
        assert!(TransformParams::is_transformable(
            &"image/png".parse().unwrap()
        ));
        assert!(TransformParams::is_transformable(
            &"image/webp".parse().unwrap()
        ));
        assert!(TransformParams::is_transformable(
            &"image/gif".parse().unwrap()
        ));
        assert!(!TransformParams::is_transformable(
            &"text/plain".parse().unwrap()
        ));
        assert!(!TransformParams::is_transformable(
            &"application/pdf".parse().unwrap()
        ));
    }

    #[test]
    fn test_unsupported_format() {
        let result = transform_image(
            b"not an image",
            &"text/plain".parse().unwrap(),
            &TransformParams {
                w: Some(100),
                h: None,
                q: None,
            },
        );
        assert!(result.is_err());
    }
}
