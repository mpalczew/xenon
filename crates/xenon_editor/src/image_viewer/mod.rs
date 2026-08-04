//! Image tab: zoom/pan state, disk reload, and GPUI paint element.

mod element;
#[cfg(test)]
mod reload_test;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{Bounds, Image, ImageFormat, Pixels, Point, ScrollDelta, ScrollWheelEvent, point, px};
use image::ImageReader;

pub(super) use element::ImageContentElement;

const MIN_IMAGE_ZOOM: f32 = 0.05;
const MAX_IMAGE_ZOOM: f32 = 16.0;

pub(super) struct ImageViewer {
    pub(super) path: PathBuf,
    /// Disk mtime at last successful load; used to detect external rewrites.
    disk_mtime: Option<SystemTime>,
    /// Content-hashed GPUI image so path-keyed asset cache is not sticky.
    image: Option<Arc<Image>>,
    natural_size: Option<ImageSize>,
    zoom: ImageZoom,
    pub(super) pan_offset: Point<Pixels>,
    pub(super) drag_last: Option<Point<Pixels>>,
    pub(super) viewport: Option<Bounds<Pixels>>,
}

#[derive(Clone, Copy)]
struct ImageSize {
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
enum ImageZoom {
    Fit,
    Manual(f32),
}

impl ImageViewer {
    pub(super) fn new(path: PathBuf) -> Self {
        let mut viewer = Self {
            path,
            disk_mtime: None,
            image: None,
            natural_size: None,
            zoom: ImageZoom::Fit,
            pan_offset: Point::default(),
            drag_last: None,
            viewport: None,
        };
        viewer.reload_from_disk();
        viewer
    }

    /// Compare against disk; reload when the file's mtime moved.
    /// Returns true when the displayed image was refreshed.
    pub(super) fn sync_from_disk(&mut self) -> bool {
        let current = fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if current.is_none() {
            // Deleted: keep last decoded frame; nothing new to show.
            return false;
        }
        if current == self.disk_mtime {
            return false;
        }
        self.reload_from_disk();
        true
    }

    fn reload_from_disk(&mut self) {
        self.disk_mtime = fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        self.natural_size = read_image_size(&self.path);
        // Image::id is a content hash, so a real rewrite is a new GPUI asset key
        // (path-keyed img(path) would keep showing the first decode forever).
        self.image = load_gpui_image(&self.path);
        self.clamp_pan();
    }

    pub(super) fn zoom_by(&mut self, factor: f32) {
        let zoom = self.current_zoom() * factor;
        self.zoom = ImageZoom::Manual(zoom.clamp(MIN_IMAGE_ZOOM, MAX_IMAGE_ZOOM));
        self.clamp_pan();
    }

    pub(super) fn zoom_at(&mut self, factor: f32, position: Point<Pixels>) {
        let old_zoom = self.current_zoom();
        let new_zoom = (old_zoom * factor).clamp(MIN_IMAGE_ZOOM, MAX_IMAGE_ZOOM);
        self.zoom = ImageZoom::Manual(new_zoom);
        self.anchor_zoom(old_zoom, new_zoom, position);
    }

    pub(super) fn pan(&mut self, delta: Point<Pixels>) {
        self.pan_offset += delta;
        self.clamp_pan();
    }

    pub(super) fn fit(&mut self) {
        self.zoom = ImageZoom::Fit;
        self.pan_offset = Point::default();
    }

    pub(super) fn actual_size(&mut self) {
        self.zoom = ImageZoom::Manual(1.);
        self.pan_offset = Point::default();
    }

    fn current_zoom(&self) -> f32 {
        match self.zoom {
            ImageZoom::Fit => self.fit_zoom(),
            ImageZoom::Manual(zoom) => zoom,
        }
    }

    fn fit_zoom(&self) -> f32 {
        let Some(size) = self.natural_size else {
            return 1.;
        };
        let Some(viewport) = self.viewport.map(|bounds| bounds.size) else {
            return 1.;
        };
        if viewport.width.as_f32() <= 0. || viewport.height.as_f32() <= 0. {
            return 1.;
        }
        (viewport.width.as_f32() / size.width)
            .min(viewport.height.as_f32() / size.height)
            .clamp(MIN_IMAGE_ZOOM, 1.)
    }

    fn scaled_size(&self, zoom: f32) -> ImageSize {
        self.natural_size
            .map(|size| ImageSize {
                width: size.width * zoom,
                height: size.height * zoom,
            })
            .unwrap_or(ImageSize {
                width: 1.,
                height: 1.,
            })
    }

    fn zoom_for_bounds(&self, bounds: Bounds<Pixels>) -> f32 {
        match self.zoom {
            ImageZoom::Fit => fit_zoom_for(bounds, self.natural_size),
            ImageZoom::Manual(zoom) => zoom,
        }
    }

    fn clamp_pan(&mut self) {
        if let Some(bounds) = self.viewport {
            self.pan_offset = clamp_pan(
                self.pan_offset,
                bounds,
                self.natural_size,
                self.current_zoom(),
            );
        }
    }

    fn anchor_zoom(&mut self, old_zoom: f32, new_zoom: f32, position: Point<Pixels>) {
        let Some(bounds) = self.viewport else {
            self.clamp_pan();
            return;
        };
        let relative_center = point(
            position.x - bounds.origin.x - bounds.size.width / 2.,
            position.y - bounds.origin.y - bounds.size.height / 2.,
        );
        let offset_from_image = relative_center - self.pan_offset;
        let ratio = new_zoom / old_zoom;
        self.pan_offset += offset_from_image * (1. - ratio);
        self.pan_offset = clamp_pan(self.pan_offset, bounds, self.natural_size, new_zoom);
    }
}

pub(super) fn event_delta(event: &ScrollWheelEvent) -> Point<Pixels> {
    match event.delta {
        ScrollDelta::Pixels(delta) => delta,
        ScrollDelta::Lines(delta) => delta.map(|value| px(value * 20.)),
    }
}

pub(super) fn zoom_factor_for_scroll(delta: Pixels) -> f32 {
    let delta: f32 = delta.into();
    if delta > 0. {
        1. + delta.abs() * 0.01
    } else {
        1. / (1. + delta.abs() * 0.01)
    }
}

fn clamp_pan(
    pan: Point<Pixels>,
    bounds: Bounds<Pixels>,
    natural_size: Option<ImageSize>,
    zoom: f32,
) -> Point<Pixels> {
    let Some(size) = natural_size else {
        return Point::default();
    };
    let overflow_x = (px(size.width * zoom) - bounds.size.width).max(px(0.)) / 2.;
    let overflow_y = (px(size.height * zoom) - bounds.size.height).max(px(0.)) / 2.;
    point(
        pan.x.max(-overflow_x).min(overflow_x),
        pan.y.max(-overflow_y).min(overflow_y),
    )
}

fn fit_zoom_for(bounds: Bounds<Pixels>, natural_size: Option<ImageSize>) -> f32 {
    let Some(size) = natural_size else {
        return 1.;
    };
    if bounds.size.width.as_f32() <= 0. || bounds.size.height.as_f32() <= 0. {
        return 1.;
    }
    (bounds.size.width.as_f32() / size.width)
        .min(bounds.size.height.as_f32() / size.height)
        .clamp(MIN_IMAGE_ZOOM, 1.)
}

fn read_image_size(path: &Path) -> Option<ImageSize> {
    let size = ImageReader::open(path).ok()?.into_dimensions().ok()?;
    Some(ImageSize {
        width: size.0 as f32,
        height: size.1 as f32,
    })
}

fn load_gpui_image(path: &Path) -> Option<Arc<Image>> {
    let bytes = fs::read(path).ok()?;
    let format = gpui_format(&bytes, path)?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

fn gpui_format(bytes: &[u8], path: &Path) -> Option<ImageFormat> {
    if let Ok(format) = image::guess_format(bytes) {
        return match format {
            image::ImageFormat::Png => Some(ImageFormat::Png),
            image::ImageFormat::Jpeg => Some(ImageFormat::Jpeg),
            image::ImageFormat::WebP => Some(ImageFormat::Webp),
            image::ImageFormat::Gif => Some(ImageFormat::Gif),
            image::ImageFormat::Bmp => Some(ImageFormat::Bmp),
            image::ImageFormat::Tiff => Some(ImageFormat::Tiff),
            image::ImageFormat::Ico => Some(ImageFormat::Ico),
            image::ImageFormat::Pnm => Some(ImageFormat::Pnm),
            _ => None,
        };
    }
    // SVG has no binary magic the `image` crate recognizes.
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("svg") => Some(ImageFormat::Svg),
        _ => None,
    }
}
