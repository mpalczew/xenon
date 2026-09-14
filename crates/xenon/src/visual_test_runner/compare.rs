use anyhow::{Result, anyhow};
use image::{Rgba, RgbaImage};
use std::path::Path;

pub fn assert_filled(image: &RgbaImage, name: &str) -> Result<()> {
    let (width, height) = image.dimensions();
    if width < 200 || height < 200 {
        return Err(anyhow!("{name}: capture too small ({width}x{height})"));
    }
    let first = image.pixels().next().copied().unwrap_or(Rgba([0, 0, 0, 0]));
    let mut same = 0u64;
    let total = u64::from(width) * u64::from(height);
    for pixel in image.pixels() {
        if *pixel == first {
            same += 1;
        }
    }
    let filled = 1.0 - (same as f64 / total as f64);
    if filled < 0.02 {
        return Err(anyhow!(
            "{name}: capture looks blank ({filled:.3} painted fraction)"
        ));
    }
    Ok(())
}

pub fn compare_or_update(
    actual: &RgbaImage,
    baseline: &Path,
    output: &Path,
    update: bool,
) -> Result<()> {
    actual.save(output)?;
    if update {
        if let Some(parent) = baseline.parent() {
            std::fs::create_dir_all(parent)?;
        }
        actual.save(baseline)?;
        println!("  baseline updated {}", baseline.display());
        return Ok(());
    }
    if !baseline.exists() {
        return Err(anyhow!(
            "missing baseline {} (run with UPDATE_BASELINE=1)",
            baseline.display()
        ));
    }
    let expected = image::open(baseline)?.to_rgba8();
    if expected.dimensions() != actual.dimensions() {
        return Err(anyhow!(
            "size {:?} vs baseline {:?}",
            actual.dimensions(),
            expected.dimensions()
        ));
    }
    let diff = diff_pixels(&expected, actual);
    if diff > 0 {
        return Err(anyhow!(
            "{} pixels differ from {}",
            diff,
            baseline.display()
        ));
    }
    Ok(())
}

fn diff_pixels(expected: &RgbaImage, actual: &RgbaImage) -> u64 {
    expected
        .pixels()
        .zip(actual.pixels())
        .filter(|(a, b)| !close(**a, **b))
        .count() as u64
}

fn close(a: Rgba<u8>, b: Rgba<u8>) -> bool {
    a.0.iter()
        .zip(b.0.iter())
        .all(|(l, r)| l.abs_diff(*r) <= 1)
}
