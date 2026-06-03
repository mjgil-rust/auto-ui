//! Rust-native image processing helpers.
//!
//! Replaces ImageMagick `convert`, `identify`, and `import` for common
//! screenshot and crop operations.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use image::GenericImageView;

use crate::window_driver::VisualMetric;

/// Get the dimensions of an image file.
pub fn image_size(image_path: &Path) -> Result<(i32, i32)> {
    let img = image::open(image_path)
        .with_context(|| format!("failed to open image {}", image_path.display()))?;
    Ok((img.width() as i32, img.height() as i32))
}

/// Compute visual metrics for a cropped region of an image.
///
/// Returns standard deviation of grayscale pixel intensities and
/// the number of unique colors in the region.
pub fn crop_metric(
    image_path: &Path,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<VisualMetric> {
    let img = image::open(image_path)
        .with_context(|| format!("failed to open image {}", image_path.display()))?;

    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    let x = x.max(0).min(img_width);
    let y = y.max(0).min(img_height);
    let width = width.max(0).min(img_width - x);
    let height = height.max(0).min(img_height - y);

    if width == 0 || height == 0 {
        return Ok(VisualMetric {
            stddev: 0.0,
            colors: 0.0,
        });
    }

    let x = x as u32;
    let y = y as u32;
    let width = width as u32;
    let height = height as u32;

    let mut pixels = Vec::new();
    let mut unique_colors = HashSet::new();

    for py in y..(y + height) {
        for px in x..(x + width) {
            let image::Rgba([r, g, b, _a]) = img.get_pixel(px, py);
            unique_colors.insert((r, g, b));
            let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            pixels.push(luma);
        }
    }

    if pixels.is_empty() {
        return Ok(VisualMetric {
            stddev: 0.0,
            colors: 0.0,
        });
    }

    let n = pixels.len() as f64;
    let mean = pixels.iter().sum::<f64>() / n;
    let variance = pixels.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();
    let colors = unique_colors.len() as f64;

    Ok(VisualMetric { stddev, colors })
}

/// Crop a region from a source image and save to a target path.
pub fn crop_image(
    source: &Path,
    target: &Path,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<()> {
    let img = image::open(source)
        .with_context(|| format!("failed to open {}", source.display()))?;
    let (img_width, img_height) = (img.width() as i32, img.height() as i32);

    let x = x.max(0).min(img_width) as u32;
    let y = y.max(0).min(img_height) as u32;
    let width = width.max(0).min(img_width - x as i32) as u32;
    let height = height.max(0).min(img_height - y as i32) as u32;

    if width == 0 || height == 0 {
        let blank = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        blank
            .save(target)
            .with_context(|| format!("failed to save {}", target.display()))?;
        return Ok(());
    }

    let mut img_clone = img.clone();
    let cropped = image::imageops::crop(&mut img_clone, x, y, width, height);
    cropped
        .to_image()
        .save(target)
        .with_context(|| format!("failed to save {}", target.display()))?;
    Ok(())
}

/// Enhance a screenshot for visual inspection.
///
/// Converts to grayscale, normalizes contrast to the full range, and upscales 200%.
pub fn enhance_image(source: &Path, target: &Path) -> Result<()> {
    let img = image::open(source)
        .with_context(|| format!("failed to open {}", source.display()))?;

    // Convert to grayscale
    let gray = img.to_luma8();

    // Normalize histogram to full 0-255 range
    let mut normalized = gray.clone();
    let (mut min_val, mut max_val) = (255u8, 0u8);
    for p in gray.pixels() {
        min_val = min_val.min(p[0]);
        max_val = max_val.max(p[0]);
    }

    if max_val > min_val {
        let range = (max_val - min_val) as f32;
        for pixel in normalized.pixels_mut() {
            let new_val = ((pixel[0] - min_val) as f32 / range * 255.0).round() as u8;
            pixel[0] = new_val;
        }
    }

    // Resize 200%
    let (new_width, new_height) = (gray.width() * 2, gray.height() * 2);
    let resized = image::imageops::resize(
        &normalized,
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );

    resized
        .save(target)
        .with_context(|| format!("failed to save {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("auto-ui-{name}-{nanos}.png"))
    }

    #[test]
    fn image_size_reads_png_dimensions() {
        let path = temp_path("image-size");
        let img = ImageBuffer::from_pixel(10, 20, Rgba([255u8, 0, 0, 255]));
        img.save(&path).unwrap();

        let (w, h) = image_size(&path).unwrap();
        assert_eq!(w, 10);
        assert_eq!(h, 20);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn image_size_fails_for_non_image() {
        let path = temp_path("not-an-image");
        std::fs::write(&path, "not an image").unwrap();

        let result = image_size(&path);
        assert!(result.is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn crop_metric_solid_color() {
        let path = temp_path("solid");
        let img = ImageBuffer::from_pixel(100, 100, Rgba([128u8, 128, 128, 255]));
        img.save(&path).unwrap();

        let metric = crop_metric(&path, 0, 0, 50, 50).unwrap();
        assert_eq!(metric.colors, 1.0);
        assert!(metric.stddev < 0.001);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn crop_metric_gradient() {
        let path = temp_path("gradient");
        let img = ImageBuffer::from_fn(100, 100, |x, _y| {
            Rgba([x as u8, x as u8, x as u8, 255])
        });
        img.save(&path).unwrap();

        let metric = crop_metric(&path, 0, 0, 100, 100).unwrap();
        assert!(metric.colors > 10.0);
        assert!(metric.stddev > 0.01);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn crop_metric_clamps_out_of_bounds() {
        let path = temp_path("clamp");
        let img = ImageBuffer::from_pixel(10, 10, Rgba([255u8, 0, 0, 255]));
        img.save(&path).unwrap();

        let metric = crop_metric(&path, 5, 5, 100, 100).unwrap();
        assert_eq!(metric.colors, 1.0);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn crop_image_creates_subregion() {
        let source = temp_path("crop-source");
        let target = temp_path("crop-target");
        let img = ImageBuffer::from_fn(100, 100, |x, y| {
            if x < 50 && y < 50 {
                Rgba([255u8, 0, 0, 255])
            } else {
                Rgba([0u8, 255, 0, 255])
            }
        });
        img.save(&source).unwrap();

        crop_image(&source, &target, 0, 0, 50, 50).unwrap();

        let (w, h) = image_size(&target).unwrap();
        assert_eq!(w, 50);
        assert_eq!(h, 50);

        std::fs::remove_file(&source).ok();
        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn enhance_image_produces_larger_grayscale() {
        let source = temp_path("enhance-source");
        let target = temp_path("enhance-target");
        let img = ImageBuffer::from_pixel(50, 50, Rgba([100u8, 150, 200, 255]));
        img.save(&source).unwrap();

        enhance_image(&source, &target).unwrap();

        let (w, h) = image_size(&target).unwrap();
        assert_eq!(w, 100);
        assert_eq!(h, 100);

        std::fs::remove_file(&source).ok();
        std::fs::remove_file(&target).ok();
    }
}
