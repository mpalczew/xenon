use anyhow::{Result, anyhow};
use image::{Rgba, RgbaImage};

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
