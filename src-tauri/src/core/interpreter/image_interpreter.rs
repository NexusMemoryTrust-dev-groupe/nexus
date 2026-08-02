use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use std::path::Path;

/// Result of interpreting an image
pub struct ImageInterpretation {
    pub entity: Entity,
    pub metadata: ImageMetadata,
    pub summary: String,
}

/// Image metadata
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    pub format: String,
    pub size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Interpret an image file (extract basic metadata)
pub fn interpret_image(path: &Path, content: &[u8]) -> ImageInterpretation {
    let file_name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let format = match ext.as_str() {
        "png" => "PNG",
        "jpg" | "jpeg" => "JPEG",
        "gif" => "GIF",
        "webp" => "WebP",
        "bmp" => "BMP",
        "svg" => "SVG",
        "ico" => "ICO",
        "tiff" | "tif" => "TIFF",
        _ => "Unknown",
    };

    // Try to detect dimensions from file headers
    let (width, height) = detect_dimensions(content, &ext);

    let metadata = ImageMetadata {
        format: format.to_string(),
        size_bytes: content.len() as u64,
        width,
        height,
    };

    let dims_str = match (width, height) {
        (Some(w), Some(h)) => format!("{}x{}", w, h),
        _ => "unknown dimensions".to_string(),
    };

    let description = format!("Image: {} ({}, {})", file_name, format, dims_str);
    let mut entity = Entity::new(EntityType::Document, file_name.clone(), description);
    entity.metadata.insert("kind".into(), serde_json::json!("image"));
    entity.metadata.insert("format".into(), serde_json::json!(format));
    if let Some(w) = width { entity.metadata.insert("width".into(), serde_json::json!(w)); }
    if let Some(h) = height { entity.metadata.insert("height".into(), serde_json::json!(h)); }

    let summary = format!("Image: {} — {} {}, {}",
        file_name, format, dims_str, format_size(content.len() as u64));

    ImageInterpretation { entity, metadata, summary }
}

/// Detect image dimensions from file header bytes
fn detect_dimensions(content: &[u8], ext: &str) -> (Option<u32>, Option<u32>) {
    if content.len() < 8 {
        return (None, None);
    }

    match ext {
        "png" => {
            // PNG: signature (8 bytes) + IHDR chunk
            if content.len() >= 24 {
                let width = u32::from_be_bytes([content[16], content[17], content[18], content[19]]);
                let height = u32::from_be_bytes([content[20], content[21], content[22], content[23]]);
                return (Some(width), Some(height));
            }
        }
        "jpg" | "jpeg" => {
            // JPEG: scan for SOF marker.
            // Uses `i + 8 < len` rather than `i < len - 9` — the latter underflows
            // (and panics) on files shorter than 9 bytes.
            let mut i = 2; // Skip SOI marker
            while i + 8 < content.len() {
                if content[i] == 0xFF {
                    let marker = content[i + 1];
                    match marker {
                        0xC0..=0xCF if marker != 0xC4 && marker != 0xC8 && marker != 0xCC => {
                            // SOF marker
                            let height = u16::from_be_bytes([content[i + 5], content[i + 6]]) as u32;
                            let width = u16::from_be_bytes([content[i + 7], content[i + 8]]) as u32;
                            return (Some(width), Some(height));
                        }
                        0xD9 => break, // EOI
                        _ => {
                            // Skip marker segment. A segment length must cover its own
                            // 2 length bytes; anything smaller is malformed, so bail out
                            // instead of looping forever on a non-advancing index.
                            let seg_len = u16::from_be_bytes([content[i + 2], content[i + 3]]) as usize;
                            if seg_len < 2 {
                                break;
                            }
                            i += 2 + seg_len;
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }
        "gif" => {
            // GIF: signature (6 bytes) + width (2) + height (2)
            if content.len() >= 10 {
                let width = u16::from_le_bytes([content[6], content[7]]) as u32;
                let height = u16::from_le_bytes([content[8], content[9]]) as u32;
                return (Some(width), Some(height));
            }
        }
        "bmp" => {
            // BMP: header (14 bytes) + DIB header
            if content.len() >= 26 {
                let width = i32::from_le_bytes([content[18], content[19], content[20], content[21]]) as u32;
                let height = i32::from_le_bytes([content[22], content[23], content[24], content[25]]).unsigned_abs();
                return (Some(width), Some(height));
            }
        }
        "webp" => {
            // WebP: RIFF header + WEBP + VP8/VP8L chunk
            if content.len() >= 30 && &content[8..12] == b"WEBP" {
                match &content[12..16] {
                    b"VP8 " => {
                        // Lossy
                        if content.len() >= 30 {
                            let width = u16::from_le_bytes([content[26], content[27]]) & 0x3FFF;
                            let height = u16::from_le_bytes([content[28], content[29]]) & 0x3FFF;
                            return (Some(width as u32), Some(height as u32));
                        }
                    }
                    b"VP8L" => {
                        // Lossless
                        if content.len() >= 25 {
                            let bits = u32::from_le_bytes([content[21], content[22], content[23], content[24]]);
                            let width = (bits & 0x3FFF) + 1;
                            let height = ((bits >> 14) & 0x3FFF) + 1;
                            return (Some(width), Some(height));
                        }
                    }
                    _ => {}
                }
            }
        }
        "svg" => {
            // SVG: try to find viewBox or width/height attributes
            let text = String::from_utf8_lossy(content);
            let width = extract_svg_attr(&text, "width");
            let height = extract_svg_attr(&text, "height");
            if width.is_some() && height.is_some() {
                return (width, height);
            }
            // Try viewBox
            if let Some(viewbox) = text.find("viewBox") {
                let rest = &text[viewbox..];
                if let Some(start) = rest.find('"').or_else(|| rest.find('\'')) {
                    let after_quote = &rest[start+1..];
                    if let Some(end) = after_quote.find('"').or_else(|| after_quote.find('\'')) {
                        let coords: Vec<&str> = after_quote[..end].split_whitespace().collect();
                        if coords.len() == 4 {
                            if let (Ok(_), Ok(h)) = (coords[2].parse::<f64>(), coords[3].parse::<f64>()) {
                                return (coords[2].parse().ok(), Some(h as u32));
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    (None, None)
}

/// Extract numeric attribute from SVG text
fn extract_svg_attr(text: &str, attr: &str) -> Option<u32> {
    if let Some(pos) = text.find(attr) {
        let rest = &text[pos + attr.len()..];
        if let Some(eq_pos) = rest.find('=') {
            let after_eq = rest[eq_pos+1..].trim_start();
            if let Some(quote) = after_eq.find('"').or_else(|| after_eq.find('\'')) {
                let val_str = &after_eq[quote+1..];
                if let Some(end_quote) = val_str.find('"').or_else(|| val_str.find('\'')) {
                    let val = val_str[..end_quote].trim().trim_end_matches("px").trim_end_matches("pt");
                    if let Ok(val) = val.parse::<f64>() {
                        return Some(val as u32);
                    }
                }
            }
        }
    }
    None
}

/// Format file size to human-readable string
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Check if a file extension is an image
pub fn is_image(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" | "tiff" | "tif"
    )
}
