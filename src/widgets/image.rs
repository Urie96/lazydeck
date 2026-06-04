use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Instant, UNIX_EPOCH},
};

use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageReader, Rgba, RgbaImage};
use mlua::prelude::*;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

static BLOCK_PREVIEWS: OnceLock<Mutex<BlockPreviewCache>> = OnceLock::new();

#[derive(Clone)]
pub struct RenderedImage {
    pub lines: Vec<Line<'static>>,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug)]
pub struct LuaImage {
    pub path: PathBuf,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
}

impl LuaImage {
    pub fn new(path: PathBuf, max_width: Option<u16>, max_height: Option<u16>) -> Self {
        Self {
            path,
            max_width,
            max_height,
        }
    }

    pub fn render_block_preview(
        &self,
        available_width: u16,
        height_limit: Option<u16>,
    ) -> mlua::Result<RenderedImage> {
        let width = self
            .max_width
            .map(|max| max.min(available_width))
            .unwrap_or(available_width);
        let max_height = match (self.max_height, height_limit) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if width == 0 {
            return Ok(RenderedImage {
                lines: Vec::new(),
                width: 0,
                height: 0,
            });
        }

        let key = BlockPreviewKey::new(&self.path, width, max_height)?;
        let cache = BLOCK_PREVIEWS.get_or_init(|| Mutex::new(BlockPreviewCache::new(16)));
        if let Some(rendered) = cache
            .lock()
            .expect("block image preview cache mutex poisoned")
            .get(&key)
        {
            return Ok(rendered);
        }

        let started = Instant::now();
        let image = read_image(&self.path)?;
        let decoded = started.elapsed();
        let resized = resize_image(&image, width, max_height);
        let resized_dims = (resized.width(), resized.height());
        let lines = rgba_to_lines(&resized);
        let rendered = RenderedImage {
            width: resized.width() as u16,
            height: lines.len() as u16,
            lines,
        };
        let elapsed = started.elapsed();
        if elapsed.as_millis() >= 100 {
            tracing::info!(
                path = %self.path.display(),
                width,
                ?max_height,
                decoded_ms = decoded.as_millis(),
                total_ms = elapsed.as_millis(),
                resized_width = resized_dims.0,
                resized_height = resized_dims.1,
                "rendered block image preview"
            );
        }

        cache
            .lock()
            .expect("block image preview cache mutex poisoned")
            .insert(key, rendered.clone());
        Ok(rendered)
    }
}

impl LuaUserData for LuaImage {}

fn read_image(path: &Path) -> mlua::Result<DynamicImage> {
    let image = ImageReader::open(path)
        .map_err(|err| {
            LuaError::RuntimeError(format!("failed to open image '{}': {err}", path.display()))
        })?
        .with_guessed_format()
        .map_err(|err| {
            LuaError::RuntimeError(format!(
                "failed to guess image format '{}': {err}",
                path.display()
            ))
        })?
        .decode()
        .map_err(|err| {
            LuaError::RuntimeError(format!(
                "failed to decode image '{}': {err}",
                path.display()
            ))
        })?;
    Ok(image)
}

fn resize_image(image: &DynamicImage, width: u16, max_height: Option<u16>) -> RgbaImage {
    let (src_w, src_h) = image.dimensions();
    if src_w == 0 || src_h == 0 {
        return RgbaImage::new(0, 0);
    }

    let max_w = width as u32;
    let max_h = max_height.map(|h| h as u32 * 2).unwrap_or(u32::MAX);
    let scale = ((max_w as f32 / src_w as f32).min(max_h as f32 / src_h as f32)).min(1.0);

    let target_w = ((src_w as f32 * scale).round().max(1.0)) as u32;
    let target_h = ((src_h as f32 * scale).round().max(1.0)) as u32;

    if target_w == src_w && target_h == src_h {
        image.to_rgba8()
    } else {
        image
            .resize_exact(target_w, target_h, FilterType::Triangle)
            .into_rgba8()
    }
}

fn rgba_to_lines(image: &RgbaImage) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(image.height().div_ceil(2) as usize);

    for y in (0..image.height()).step_by(2) {
        let mut spans = Vec::with_capacity(image.width() as usize);
        for x in 0..image.width() {
            let top = *image.get_pixel(x, y);
            let bottom = if y + 1 < image.height() {
                *image.get_pixel(x, y + 1)
            } else {
                Rgba([0, 0, 0, 0])
            };
            spans.push(pixel_pair_to_span(top, bottom));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn pixel_pair_to_span(top: Rgba<u8>, bottom: Rgba<u8>) -> Span<'static> {
    let top = rgba_to_color(top);
    let bottom = rgba_to_color(bottom);

    match (top, bottom) {
        (None, None) => Span::raw(" "),
        (Some(fg), None) => Span::styled("▀", Style::default().fg(fg)),
        (None, Some(fg)) => Span::styled("▄", Style::default().fg(fg)),
        (Some(fg), Some(bg)) => Span::styled("▀", Style::default().fg(fg).bg(bg)),
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BlockPreviewKey {
    path: PathBuf,
    len: u64,
    modified_ns: u128,
    width: u16,
    max_height: Option<u16>,
}

impl BlockPreviewKey {
    fn new(path: &Path, width: u16, max_height: Option<u16>) -> mlua::Result<Self> {
        let meta = std::fs::metadata(path).map_err(|err| {
            LuaError::RuntimeError(format!(
                "failed to stat image '{}': {err}",
                path.display()
            ))
        })?;
        let modified_ns = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        Ok(Self {
            path: path.to_path_buf(),
            len: meta.len(),
            modified_ns,
            width,
            max_height,
        })
    }
}

struct BlockPreviewCache {
    entries: HashMap<BlockPreviewKey, RenderedImage>,
    order: VecDeque<BlockPreviewKey>,
    capacity: usize,
}

impl BlockPreviewCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, key: &BlockPreviewKey) -> Option<RenderedImage> {
        let entry = self.entries.get(key)?.clone();
        self.touch(key);
        Some(entry)
    }

    fn insert(&mut self, key: BlockPreviewKey, value: RenderedImage) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), value);
            self.touch(&key);
            return;
        }

        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }

        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }

    fn touch(&mut self, key: &BlockPreviewKey) {
        if let Some(idx) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(idx);
        }
        self.order.push_back(key.clone());
    }
}

fn rgba_to_color(pixel: Rgba<u8>) -> Option<Color> {
    let [r, g, b, a] = pixel.0;
    if a == 0 {
        return None;
    }

    let alpha = a as u16;
    let blend = |channel: u8| ((channel as u16 * alpha) / 255) as u8;
    Some(Color::Rgb(blend(r), blend(g), blend(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn image_renders_to_half_block_lines() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        image.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
        image.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
        image.put_pixel(1, 1, Rgba([255, 255, 0, 255]));

        let lines = rgba_to_lines(&image);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 2);
        assert_eq!(lines[0].spans[0].content.as_ref(), "▀");
        assert_eq!(lines[0].spans[1].content.as_ref(), "▀");
    }

    #[test]
    fn image_height_can_be_capped_by_viewport_limit() {
        let mut image = RgbaImage::new(10, 40);
        for y in 0..40 {
            for x in 0..10 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let path = std::env::temp_dir().join(format!(
            "lazydeck-image-limit-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        image.save(&path).expect("save temp image");

        let rendered = LuaImage::new(path.clone(), None, None)
            .render_block_preview(20, Some(5))
            .expect("render preview");

        assert!(rendered.height <= 5);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn small_image_is_not_upscaled_to_fill_viewport() {
        let mut image = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let path = std::env::temp_dir().join(format!(
            "lazydeck-image-small-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        image.save(&path).expect("save temp image");

        let rendered = LuaImage::new(path.clone(), None, None)
            .render_block_preview(40, Some(20))
            .expect("render preview");

        assert!(rendered.width <= 4);
        assert!(rendered.height <= 2);

        std::fs::remove_file(path).ok();
    }
}
