use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, VecDeque},
    env, fs,
    hash::{Hash, Hasher},
    io::Write,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Instant, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::FilterType,
    DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
};
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Protocol {
    Kitty,
    Iip,
}

static PROTOCOL: OnceLock<Option<Protocol>> = OnceLock::new();
static SHOWN_IMAGES: Mutex<Vec<ShownImage>> = Mutex::new(Vec::new());
static PREPARED_IMAGES: OnceLock<Mutex<PreparedImageCache>> = OnceLock::new();
static TMUX_PASSTHROUGH_READY: OnceLock<()> = OnceLock::new();

pub fn protocol() -> Option<Protocol> {
    ensure_tmux_passthrough();
    *PROTOCOL.get_or_init(detect_protocol)
}

#[allow(dead_code)]
pub fn clear<W: Write + ?Sized>(w: &mut W) -> Result<bool> {
    let shown = SHOWN_IMAGES
        .lock()
        .expect("native image mutex poisoned")
        .drain(..)
        .collect::<Vec<_>>();
    if !shown.is_empty() {
        for image in &shown {
            clear_shown_image(w, image)?;
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn render<W: Write + ?Sized>(w: &mut W, path: &Path, area: Rect) -> Result<bool> {
    if area.is_empty() {
        return Ok(false);
    }

    let Some(protocol) = protocol() else {
        return Ok(false);
    };

    let next = ShownImage {
        path: path.to_path_buf(),
        area,
        protocol,
        kitty_id: (protocol == Protocol::Kitty).then(|| {
            let (max_width_px, max_height_px) = max_pixels(area);
            kitty_image_id(path, max_width_px, max_height_px)
        }),
    };
    let mut shown = SHOWN_IMAGES.lock().expect("native image mutex poisoned");
    if shown.contains(&next) {
        return Ok(true);
    }

    match protocol {
        Protocol::Iip => render_iip(w, path, area)?,
        Protocol::Kitty => render_kitty(
            w,
            path,
            area,
            next.kitty_id
                .expect("kitty image id must exist for kitty protocol"),
        )?,
    }

    shown.push(next);
    Ok(true)
}

pub fn measure_cell_area(
    path: &Path,
    max_area: Rect,
    max_width_cells: Option<u16>,
    max_height_cells: Option<u16>,
) -> Result<Rect> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("failed to open image '{}'", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("failed to inspect image '{}'", path.display()))?;
    let (src_w, src_h) = reader.into_dimensions()?;
    let (cw, ch) = cell_pixel_size().unwrap_or((8, 16));
    let max_w_px = (max_width_cells
        .unwrap_or(max_area.width)
        .min(max_area.width) as u32
        * cw as u32)
        .max(1);
    let max_h_px = (max_height_cells
        .unwrap_or(max_area.height)
        .min(max_area.height) as u32
        * ch as u32)
        .max(1);
    let scale = ((max_w_px as f32 / src_w as f32).min(max_h_px as f32 / src_h as f32)).min(1.0);
    let scaled_w = ((src_w as f32 * scale).round().max(1.0)) as u32;
    let scaled_h = ((src_h as f32 * scale).round().max(1.0)) as u32;

    Ok(Rect::new(
        0,
        0,
        ((scaled_w + cw as u32 - 1) / cw as u32) as u16,
        ((scaled_h + ch as u32 - 1) / ch as u32) as u16,
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ShownImage {
    path: PathBuf,
    area: Rect,
    protocol: Protocol,
    kitty_id: Option<u32>,
}

fn clear_shown_image<W: Write + ?Sized>(w: &mut W, shown: &ShownImage) -> Result<()> {
    match shown.protocol {
        Protocol::Kitty => {
            erase_area(w, shown.area)?;
            if let Some(id) = shown.kitty_id {
                write_wrapped_escape(w, &format!("\x1b_Gq=2,a=d,d=I,i={id}\x1b\\"))
            } else {
                Ok(())
            }
        }
        Protocol::Iip => erase_area(w, shown.area),
    }
}

fn detect_protocol() -> Option<Protocol> {
    let env = detected_env();

    if env.kitty_window_id.is_some() {
        return Some(Protocol::Kitty);
    }

    match env.term_program.as_deref() {
        Some("iTerm.app") => return Some(Protocol::Iip),
        Some("WezTerm") => return Some(Protocol::Iip),
        Some("vscode") => return Some(Protocol::Iip),
        _ => {}
    }

    let term = env.term.unwrap_or_default();
    if term.contains("xterm-kitty") {
        return Some(Protocol::Kitty);
    }

    None
}

#[derive(Default)]
struct DetectedEnv {
    term: Option<String>,
    term_program: Option<String>,
    kitty_window_id: Option<String>,
}

fn detected_env() -> DetectedEnv {
    let mut detected = DetectedEnv {
        term: env::var("TERM").ok(),
        term_program: env::var("TERM_PROGRAM").ok(),
        kitty_window_id: env::var("KITTY_WINDOW_ID").ok(),
    };

    if env::var_os("TMUX").is_none() {
        return detected;
    }

    if let Ok(output) = Command::new("tmux").arg("show-environment").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some((key, value)) = line.trim().split_once('=') else {
                continue;
            };

            match key {
                "TERM" => detected.term = Some(value.to_owned()),
                "TERM_PROGRAM" => detected.term_program = Some(value.to_owned()),
                "KITTY_WINDOW_ID" => detected.kitty_window_id = Some(value.to_owned()),
                _ => {}
            }
        }
    }

    detected
}

fn ensure_tmux_passthrough() {
    if env::var_os("TMUX").is_none() {
        return;
    }

    let _ = TMUX_PASSTHROUGH_READY.get_or_init(|| {
        let _ = Command::new("tmux")
            .args(["set", "-p", "allow-passthrough", "on"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

fn render_iip<W: Write + ?Sized>(w: &mut W, path: &Path, area: Rect) -> Result<()> {
    let prepared = prepared_image(path, area, Protocol::Iip)?;
    let PreparedImageKind::Iip {
        width,
        height,
        binary_len,
        payload,
    } = &prepared.kind
    else {
        anyhow::bail!("invalid prepared image kind for iip");
    };

    move_to(w, area.x, area.y)?;
    let payload = format!(
        "\x1b]1337;File=inline=1;size={};width={}px;height={}px;doNotMoveCursor=1:{}\x07",
        binary_len, width, height, payload
    );
    write_wrapped_escape(w, &payload)?;
    Ok(())
}

fn render_kitty<W: Write + ?Sized>(
    w: &mut W,
    path: &Path,
    area: Rect,
    image_id: u32,
) -> Result<()> {
    let prepared = prepared_image(path, area, Protocol::Kitty)?;
    let PreparedImageKind::Kitty {
        width,
        height,
        format,
        payload,
    } = &prepared.kind
    else {
        anyhow::bail!("invalid prepared image kind for kitty");
    };
    let chunks = kitty_chunks_from_base64(*format, *width, *height, payload, image_id);
    let placement = kitty_place(area, image_id);

    move_to(w, area.x, area.y)?;
    for chunk in chunks {
        write_wrapped_escape(w, &chunk)?;
    }
    write_all(w, &placement)?;
    Ok(())
}

fn prepared_image(path: &Path, area: Rect, protocol: Protocol) -> Result<PreparedImage> {
    let started = Instant::now();
    let (max_width_px, max_height_px) = max_pixels(area);
    let key = PreparedImageKey {
        path: path.to_path_buf(),
        protocol,
        max_width_px,
        max_height_px,
    };

    let cache = PREPARED_IMAGES.get_or_init(|| Mutex::new(PreparedImageCache::new(8)));
    if let Some(prepared) = cache
        .lock()
        .expect("prepared image cache mutex poisoned")
        .get(&key)
    {
        return Ok(prepared);
    }

    let memory_cache_elapsed = started.elapsed();
    if let Some(prepared) = load_prepared_image_from_disk(&key)? {
        let mut cache = cache.lock().expect("prepared image cache mutex poisoned");
        cache.insert(key, prepared.clone());
        let elapsed = started.elapsed();
        if elapsed.as_millis() >= 50 {
            tracing::info!(
                path = %path.display(),
                ?protocol,
                max_width_px,
                max_height_px,
                memory_cache_ms = memory_cache_elapsed.as_millis(),
                total_ms = elapsed.as_millis(),
                "loaded prepared native image from disk cache"
            );
        }
        return Ok(prepared);
    }

    let disk_cache_elapsed = started.elapsed();
    let preview_source = ffmpeg_preview_source(path, key.max_width_px, key.max_height_px)?;
    let ffmpeg_elapsed = started.elapsed();
    let source_path = preview_source.as_deref().unwrap_or(path);
    let img = downscale_for_pixels(source_path, key.max_width_px, key.max_height_px)?;
    let downscale_elapsed = started.elapsed();
    let prepared = match protocol {
        Protocol::Iip => PreparedImage::from_inline_image(encode_inline_image(img)?),
        Protocol::Kitty => PreparedImage::from_kitty_image(encode_kitty_payload(img)?),
    };
    let encode_elapsed = started.elapsed();

    save_prepared_image_to_disk(&key, &prepared)?;
    let mut cache = cache.lock().expect("prepared image cache mutex poisoned");
    cache.insert(key, prepared.clone());

    let total_elapsed = started.elapsed();
    if total_elapsed.as_millis() >= 100 {
        tracing::info!(
            path = %path.display(),
            ?protocol,
            max_width_px,
            max_height_px,
            disk_cache_ms = disk_cache_elapsed.as_millis(),
            ffmpeg_ms = ffmpeg_elapsed.as_millis(),
            ffmpeg_used = preview_source.is_some(),
            downscale_ms = downscale_elapsed.as_millis(),
            encode_ms = encode_elapsed.as_millis(),
            total_ms = total_elapsed.as_millis(),
            "prepared native image"
        );
    }

    Ok(prepared)
}

fn ffmpeg_preview_source(path: &Path, max_w: u32, max_h: u32) -> Result<Option<PathBuf>> {
    let Some(cache_path) = ffmpeg_preview_cache_path(path, max_w, max_h)? else {
        return Ok(None);
    };
    if cache_path.is_file() {
        return Ok(Some(cache_path));
    }

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = cache_path.with_extension("jpg.tmp");
    let scale = format!(
        "scale='min({max_w},iw)':'min({max_h},ih)':force_original_aspect_ratio=decrease:flags=fast_bilinear,format=rgb24"
    );
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf"])
        .arg(scale)
        .args(["-q:v", "6", "-f", "image2"])
        .arg(&tmp_path)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            fs::rename(&tmp_path, &cache_path)?;
            Ok(Some(cache_path))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!(
                path = %path.display(),
                status = ?output.status.code(),
                stderr = %stderr.trim(),
                "ffmpeg image preview source failed; falling back to image crate"
            );
            let _ = fs::remove_file(tmp_path);
            Ok(None)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn ffmpeg_preview_cache_path(path: &Path, max_w: u32, max_h: u32) -> Result<Option<PathBuf>> {
    let meta = fs::metadata(path)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let source = format!(
        "ffmpeg-v1|{}|{}|{}|{}|{}|q6",
        path.display(),
        meta.len(),
        modified,
        max_w,
        max_h
    );
    let name = STANDARD.encode(source).replace(['/', '+', '='], "_") + ".jpg";
    Ok(Some(prepared_cache_dir()?.join("ffmpeg-preview-sources").join(name)))
}

fn downscale_for_pixels(path: &Path, max_w: u32, max_h: u32) -> Result<DynamicImage> {
    let mut img = ImageReader::open(path)
        .with_context(|| format!("failed to open image '{}'", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode image '{}'", path.display()))?;

    if img.width() > max_w || img.height() > max_h {
        img = img.resize(max_w, max_h, FilterType::Triangle);
    }
    Ok(img)
}

fn max_pixels(area: Rect) -> (u32, u32) {
    match cell_pixel_size() {
        Some((cw, ch)) => (
            (area.width as u32 * cw as u32).max(1),
            (area.height as u32 * ch as u32).max(1),
        ),
        None => (
            (area.width as u32 * 8).max(1),
            (area.height as u32 * 16).max(1),
        ),
    }
}

fn encode_png_image(img: DynamicImage) -> Result<EncodedImage> {
    let (width, height) = img.dimensions();
    let mut bytes = Vec::new();
    if img.color().has_alpha() {
        PngEncoder::new(&mut bytes).write_image(
            &img.into_rgba8(),
            width,
            height,
            ExtendedColorType::Rgba8,
        )?;
    } else {
        PngEncoder::new(&mut bytes).write_image(
            &img.into_rgb8(),
            width,
            height,
            ExtendedColorType::Rgb8,
        )?;
    }

    Ok(EncodedImage {
        width,
        height,
        binary_len: bytes.len(),
        base64: STANDARD.encode(bytes),
    })
}

fn encode_inline_image(img: DynamicImage) -> Result<EncodedImage> {
    if img.color().has_alpha() {
        return encode_png_image(img);
    }

    let (width, height) = img.dimensions();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 75).encode_image(&img)?;

    Ok(EncodedImage {
        width,
        height,
        binary_len: bytes.len(),
        base64: STANDARD.encode(bytes),
    })
}

fn encode_kitty_payload(img: DynamicImage) -> Result<EncodedKittyImage> {
    let encoded = encode_png_image(img)?;

    Ok(EncodedKittyImage {
        width: encoded.width,
        height: encoded.height,
        format: 100,
        base64: encoded.base64,
    })
}

fn kitty_chunks_from_base64(
    format: u8,
    width: u32,
    height: u32,
    b64: &str,
    image_id: u32,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut parts = b64.as_bytes().chunks(4096).peekable();
    if let Some(first) = parts.next() {
        chunks.push(format!(
            "\x1b_Gq=2,a=T,C=1,U=1,f={format},s={width},v={height},i={image_id},m={};{}\x1b\\",
            u8::from(parts.peek().is_some()),
            std::str::from_utf8(first).unwrap_or_default()
        ));
    }
    while let Some(chunk) = parts.next() {
        chunks.push(format!(
            "\x1b_Gm={};{}\x1b\\",
            u8::from(parts.peek().is_some()),
            std::str::from_utf8(chunk).unwrap_or_default()
        ));
    }
    chunks
}

fn kitty_place(area: Rect, image_id: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let (r, g, b) = (
        (image_id >> 16) & 0xff,
        (image_id >> 8) & 0xff,
        image_id & 0xff,
    );
    let _ = write!(buf, "\x1b[38;2;{r};{g};{b}m");

    for y in 0..area.height {
        let _ = write!(buf, "\x1b[{};{}H", area.y + y + 1, area.x + 1);
        for x in 0..area.width {
            let _ = write!(buf, "\u{10EEEE}");
            let _ = write!(
                buf,
                "{}{}",
                DIACRITICS.get(y as usize).copied().unwrap_or(DIACRITICS[0]),
                DIACRITICS.get(x as usize).copied().unwrap_or(DIACRITICS[0])
            );
        }
    }
    buf
}

fn kitty_image_id(path: &Path, max_width_px: u32, max_height_px: u32) -> u32 {
    let mut hasher = DefaultHasher::new();
    std::process::id().hash(&mut hasher);
    path.hash(&mut hasher);
    max_width_px.hash(&mut hasher);
    max_height_px.hash(&mut hasher);
    (hasher.finish() as u32) & 0x00ff_ffff
}

fn erase_area<W: Write + ?Sized>(w: &mut W, area: Rect) -> Result<()> {
    let spaces = " ".repeat(area.width as usize);
    for y in area.y..area.y + area.height {
        move_to(w, area.x, y)?;
        write_all(w, spaces.as_bytes())?;
    }
    Ok(())
}

fn move_to<W: Write + ?Sized>(w: &mut W, x: u16, y: u16) -> Result<()> {
    write_all(w, format!("\x1b[{};{}H", y + 1, x + 1).as_bytes())
}

fn write_wrapped_escape<W: Write + ?Sized>(w: &mut W, sequence: &str) -> Result<()> {
    if env::var_os("TMUX").is_some() {
        write_all(w, b"\x1bPtmux;")?;
        let escaped = sequence.replace('\x1b', "\x1b\x1b");
        write_all(w, escaped.as_bytes())?;
        write_all(w, b"\x1b\\")?;
        return Ok(());
    }
    write_all(w, sequence.as_bytes())
}

fn write_all<W: Write + ?Sized>(w: &mut W, bytes: &[u8]) -> Result<()> {
    w.write_all(bytes)?;
    Ok(())
}

pub(crate) fn cell_pixel_size() -> Option<(u16, u16)> {
    let fd = std::io::stdout().as_raw_fd();
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut winsize) };
    if result != 0
        || winsize.ws_col == 0
        || winsize.ws_row == 0
        || winsize.ws_xpixel == 0
        || winsize.ws_ypixel == 0
    {
        return None;
    }

    Some((
        winsize.ws_xpixel / winsize.ws_col,
        winsize.ws_ypixel / winsize.ws_row,
    ))
}

struct EncodedImage {
    width: u32,
    height: u32,
    binary_len: usize,
    base64: String,
}

struct EncodedKittyImage {
    width: u32,
    height: u32,
    format: u8,
    base64: String,
}

#[derive(Clone)]
struct PreparedImage {
    kind: PreparedImageKind,
}

impl PreparedImage {
    fn from_inline_image(image: EncodedImage) -> Self {
        Self {
            kind: PreparedImageKind::Iip {
                width: image.width,
                height: image.height,
                binary_len: image.binary_len,
                payload: image.base64,
            },
        }
    }

    fn from_kitty_image(image: EncodedKittyImage) -> Self {
        Self {
            kind: PreparedImageKind::Kitty {
                width: image.width,
                height: image.height,
                format: image.format,
                payload: image.base64,
            },
        }
    }
}

#[derive(Clone)]
enum PreparedImageKind {
    Iip {
        width: u32,
        height: u32,
        binary_len: usize,
        payload: String,
    },
    Kitty {
        width: u32,
        height: u32,
        format: u8,
        payload: String,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PreparedImageKey {
    path: PathBuf,
    protocol: Protocol,
    max_width_px: u32,
    max_height_px: u32,
}

struct PreparedImageCache {
    entries: HashMap<PreparedImageKey, PreparedImage>,
    order: VecDeque<PreparedImageKey>,
    capacity: usize,
}

fn load_prepared_image_from_disk(key: &PreparedImageKey) -> Result<Option<PreparedImage>> {
    let path = prepared_cache_path(key)?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };

    parse_prepared_image(&bytes).map(Some)
}

fn save_prepared_image_to_disk(key: &PreparedImageKey, prepared: &PreparedImage) -> Result<()> {
    let path = prepared_cache_path(key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serialize_prepared_image(prepared))?;
    Ok(())
}

fn prepared_cache_path(key: &PreparedImageKey) -> Result<PathBuf> {
    Ok(prepared_cache_dir()?.join(prepared_cache_key(key)?))
}

fn prepared_cache_dir() -> Result<PathBuf> {
    let home = env::var("HOME")?;
    Ok(Path::new(&home)
        .join(".cache/lazydeck")
        .join("prepared-images"))
}

fn prepared_cache_key(key: &PreparedImageKey) -> Result<String> {
    let meta = fs::metadata(&key.path)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let source = format!(
        "v3|{}|{}|{}|{}|{}|{}",
        key.path.display(),
        meta.len(),
        modified,
        match key.protocol {
            Protocol::Kitty => "kitty",
            Protocol::Iip => "iip",
        },
        key.max_width_px,
        key.max_height_px
    );
    Ok(STANDARD.encode(source).replace(['/', '+', '='], "_"))
}

fn serialize_prepared_image(prepared: &PreparedImage) -> Vec<u8> {
    match &prepared.kind {
        PreparedImageKind::Iip {
            width,
            height,
            binary_len,
            payload,
        } => format!("IIP\n{width}\n{height}\n{binary_len}\n{payload}").into_bytes(),
        PreparedImageKind::Kitty {
            width,
            height,
            format,
            payload,
        } => {
            let mut bytes = format!("KITTY\n{width}\n{height}\n{format}\n").into_bytes();
            bytes.extend_from_slice(payload.as_bytes());
            bytes
        }
    }
}

fn parse_prepared_image(bytes: &[u8]) -> Result<PreparedImage> {
    if let Some(rest) = bytes.strip_prefix(b"KITTY\n") {
        let text = String::from_utf8(rest.to_vec())?;
        let mut parts = text.splitn(4, '\n');
        let width = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing kitty width"))?
            .parse()?;
        let height = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing kitty height"))?
            .parse()?;
        let format = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing kitty format"))?
            .parse()?;
        let payload = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing kitty payload"))?
            .to_owned();

        return Ok(PreparedImage {
            kind: PreparedImageKind::Kitty {
                width,
                height,
                format,
                payload,
            },
        });
    }

    if let Some(rest) = bytes.strip_prefix(b"IIP\n") {
        let text = String::from_utf8(rest.to_vec())?;
        let mut parts = text.splitn(4, '\n');
        let width = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing width"))?
            .parse()?;
        let height = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing height"))?
            .parse()?;
        let binary_len = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing binary_len"))?
            .parse()?;
        let payload = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("corrupt prepared image cache: missing payload"))?
            .to_owned();

        return Ok(PreparedImage {
            kind: PreparedImageKind::Iip {
                width,
                height,
                binary_len,
                payload,
            },
        });
    }

    Err(anyhow::anyhow!(
        "corrupt prepared image cache: unknown format"
    ))
}

impl PreparedImageCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, key: &PreparedImageKey) -> Option<PreparedImage> {
        let entry = self.entries.get(key)?.clone();
        self.touch(key);
        Some(entry)
    }

    fn insert(&mut self, key: PreparedImageKey, value: PreparedImage) {
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

    fn touch(&mut self, key: &PreparedImageKey) {
        if let Some(idx) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(idx);
        }
        self.order.push_back(key.clone());
    }
}

const DIACRITICS: &[char] = &[
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
    '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}', '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}',
    '\u{0485}', '\u{0486}', '\u{0487}', '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}',
    '\u{0598}', '\u{0599}', '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}',
    '\u{05A8}', '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}', '\u{0658}',
    '\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}', '\u{06D7}', '\u{06D8}',
    '\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}', '\u{06E0}', '\u{06E1}', '\u{06E2}',
    '\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}', '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}',
    '\u{0735}', '\u{0736}', '\u{073A}', '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}',
    '\u{0745}', '\u{0747}', '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}',
    '\u{07EF}', '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}', '\u{0822}',
];

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kitty_payload_uses_png_transfer_format() {
        let mut image = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let encoded = encode_kitty_payload(DynamicImage::ImageRgba8(image)).expect("encode kitty");
        let bytes = STANDARD.decode(encoded.base64).expect("decode payload");

        assert_eq!(encoded.format, 100);
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn small_image_native_area_stays_small() {
        let mut image = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                image.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let path = std::env::temp_dir().join(format!(
            "lazydeck-native-area-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        image.save(&path).expect("save temp image");

        let rect =
            measure_cell_area(&path, Rect::new(0, 0, 40, 20), None, None).expect("measure area");
        assert!(rect.width <= 2);
        assert!(rect.height <= 1);

        std::fs::remove_file(path).ok();
    }
}
