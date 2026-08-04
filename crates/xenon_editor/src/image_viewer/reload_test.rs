//! Disk-reload unit tests for the image tab.

use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Duration;

use image::{ImageBuffer, Rgba};
use tempfile::NamedTempFile;

use super::ImageViewer;

fn write_png(path: &Path, color: [u8; 4], width: u32, height: u32) {
    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_pixel(width, height, Rgba(color));
    img.save(path).unwrap();
}

#[test]
fn reloads_when_file_changes_on_disk() {
    let mut file = NamedTempFile::with_suffix(".png").unwrap();
    write_png(file.path(), [255, 0, 0, 255], 2, 2);
    file.flush().unwrap();

    let mut viewer = ImageViewer::new(file.path().to_path_buf());
    let first_id = viewer.image.as_ref().map(|i| i.id()).expect("first");
    assert_eq!(
        viewer.natural_size.map(|s| (s.width, s.height)),
        Some((2., 2.))
    );
    assert!(!viewer.sync_from_disk());

    // Coarse mtime filesystems need a beat before the rewrite is observable.
    thread::sleep(Duration::from_millis(20));
    write_png(file.path(), [0, 255, 0, 255], 4, 3);

    assert!(viewer.sync_from_disk());
    let second_id = viewer.image.as_ref().map(|i| i.id()).expect("second");
    assert_ne!(first_id, second_id);
    assert_eq!(
        viewer.natural_size.map(|s| (s.width, s.height)),
        Some((4., 3.))
    );
    assert!(!viewer.sync_from_disk());
}
