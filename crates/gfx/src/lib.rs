//! Renderer-independent geometry plus a small software canvas.
//!
//! The host simulator and headless screenshots use this canvas directly. The
//! webOS build can implement the same drawing calls with GLES2 text/quad
//! batches without changing screen code.

use tiny_skia::{
    Color as SkColor, FillRule, FilterQuality, Paint, PathBuilder, Pattern, Pixmap, Rect as SkRect,
    SpreadMode, Stroke as SkStroke, Transform,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub const fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }

    pub const fn center(self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    pub fn inset(self, amount: i32) -> Self {
        Self {
            x: self.x.saturating_add(amount),
            y: self.y.saturating_add(amount),
            width: (self.width - amount * 2).max(0),
            height: (self.height - amount * 2).max(0),
        }
    }

    pub fn offset(self, x: i32, y: i32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..self
        }
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    pub fn as_skia(self) -> Option<SkRect> {
        SkRect::from_xywh(
            self.x as f32,
            self.y as f32,
            self.width.max(0) as f32,
            self.height.max(0) as f32,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const BLACK: Self = Self::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    pub fn mix(self, other: Self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Self {
            r: blend(self.r, other.r),
            g: blend(self.g, other.g),
            b: blend(self.b, other.b),
            a: blend(self.a, other.a),
        }
    }

    pub fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    fn skia(self) -> SkColor {
        SkColor::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

/// A decoded RGBA image ready to be drawn by the software canvas or uploaded as
/// a GLES2 texture. Pixels are premultiplied, matching `tiny-skia`.
#[derive(Clone)]
pub struct Image {
    pixmap: Pixmap,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Image")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish()
    }
}

impl Image {
    /// Build an image from straight (non-premultiplied) RGBA bytes.
    pub fn from_rgba(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self, String> {
        let expected = width as usize * height as usize * 4;
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err("image dimensions do not match the pixel buffer".into());
        }
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * alpha) / 255) as u8;
            }
        }
        let size = tiny_skia::IntSize::from_wh(width, height)
            .ok_or_else(|| "image is too large".to_string())?;
        let pixmap = Pixmap::from_vec(pixels, size)
            .ok_or_else(|| "could not allocate the image".to_string())?;
        Ok(Self { pixmap })
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub size: f32,
    pub color: Color,
    pub weight: FontWeight,
}

impl TextStyle {
    pub const fn new(size: f32, color: Color) -> Self {
        Self {
            size,
            color,
            weight: FontWeight::Regular,
        }
    }

    pub const fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub const fn semibold(self) -> Self {
        self.weight(FontWeight::SemiBold)
    }

    pub const fn bold(self) -> Self {
        self.weight(FontWeight::Bold)
    }

    pub const fn medium(self) -> Self {
        self.weight(FontWeight::Medium)
    }
}

struct Fonts {
    regular: fontdue::Font,
    medium: fontdue::Font,
    semibold: fontdue::Font,
    bold: fontdue::Font,
}

impl Fonts {
    fn load() -> Result<Self, String> {
        fn load(bytes: &'static [u8], name: &str) -> Result<fontdue::Font, String> {
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
                .map_err(|error| format!("could not load {name}: {error}"))
        }
        Ok(Self {
            regular: load(
                include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
                "Inter Regular",
            )?,
            medium: load(
                include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
                "Inter Medium",
            )?,
            semibold: load(
                include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
                "Inter SemiBold",
            )?,
            bold: load(
                include_bytes!("../../../assets/fonts/Inter-Bold.ttf"),
                "Inter Bold",
            )?,
        })
    }

    fn get(&self, weight: FontWeight) -> &fontdue::Font {
        match weight {
            FontWeight::Regular => &self.regular,
            FontWeight::Medium => &self.medium,
            FontWeight::SemiBold => &self.semibold,
            FontWeight::Bold => &self.bold,
        }
    }
}

fn round_rect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let width = rect.width.max(0) as f32;
    let height = rect.height.max(0) as f32;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let x = rect.x as f32;
    let y = rect.y as f32;
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    if radius <= 0.0 {
        let mut builder = PathBuilder::new();
        builder.push_rect(SkRect::from_xywh(x, y, width, height)?);
        return builder.finish();
    }
    let k = 0.552_284_8 * radius;
    let right = x + width;
    let bottom = y + height;
    let mut builder = PathBuilder::new();
    builder.move_to(x + radius, y);
    builder.line_to(right - radius, y);
    builder.cubic_to(
        right - radius + k,
        y,
        right,
        y + radius - k,
        right,
        y + radius,
    );
    builder.line_to(right, bottom - radius);
    builder.cubic_to(
        right,
        bottom - radius + k,
        right - radius + k,
        bottom,
        right - radius,
        bottom,
    );
    builder.line_to(x + radius, bottom);
    builder.cubic_to(
        x + radius - k,
        bottom,
        x,
        bottom - radius + k,
        x,
        bottom - radius,
    );
    builder.line_to(x, y + radius);
    builder.cubic_to(x, y + radius - k, x + radius - k, y, x + radius, y);
    builder.finish()
}

pub struct Canvas {
    pixmap: Pixmap,
    fonts: Fonts,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let pixmap = Pixmap::new(width.max(1), height.max(1))
            .ok_or_else(|| "could not allocate the canvas".to_string())?;
        Ok(Self {
            pixmap,
            fonts: Fonts::load()?,
        })
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Premultiplied RGBA pixels, ready for a texture upload.
    pub fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }

    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        self.pixmap
            .save_png(path.as_ref())
            .map_err(|error| error.to_string())
    }

    pub fn clear(&mut self, color: Color) {
        self.pixmap.fill(color.skia());
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        if rect.width <= 0 || rect.height <= 0 || color.a == 0 {
            return;
        }
        let Some(rect) = rect.as_skia() else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(color.skia());
        paint.anti_alias = false;
        self.pixmap
            .fill_rect(rect, &paint, Transform::identity(), None);
    }

    pub fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        if rect.width <= 0 || rect.height <= 0 || color.a == 0 {
            return;
        }
        let Some(path) = round_rect_path(rect, radius) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(color.skia());
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub fn stroke_round_rect(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        if rect.width <= 0 || rect.height <= 0 || color.a == 0 {
            return;
        }
        let Some(path) = round_rect_path(rect, radius) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(color.skia());
        paint.anti_alias = true;
        let stroke = SkStroke {
            width: width.max(1.0),
            ..Default::default()
        };
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    pub fn fill_triangle(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        if color.a == 0 {
            return;
        }
        let mut builder = PathBuilder::new();
        builder.move_to(a.0 as f32, a.1 as f32);
        builder.line_to(b.0 as f32, b.1 as f32);
        builder.line_to(c.0 as f32, c.1 as f32);
        builder.close();
        let Some(path) = builder.finish() else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(color.skia());
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub fn fill_circle(&mut self, center: (i32, i32), radius: f32, color: Color) {
        let rect = Rect::new(
            center.0 - radius as i32,
            center.1 - radius as i32,
            (radius * 2.0) as i32,
            (radius * 2.0) as i32,
        );
        self.fill_round_rect(rect, radius, color);
    }

    pub fn fill_vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        for offset in 0..rect.height {
            let amount = if rect.height <= 1 {
                0.0
            } else {
                offset as f32 / (rect.height - 1) as f32
            };
            self.fill_rect(
                Rect::new(rect.x, rect.y + offset, rect.width, 1),
                top.mix(bottom, amount),
            );
        }
    }

    /// Vertical gradient clipped to a rounded rectangle.
    pub fn fill_round_vertical_gradient(
        &mut self,
        rect: Rect,
        radius: f32,
        top: Color,
        bottom: Color,
    ) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let radius = radius
            .max(0.0)
            .min(rect.width as f32 / 2.0)
            .min(rect.height as f32 / 2.0);
        for offset in 0..rect.height {
            let amount = if rect.height <= 1 {
                0.0
            } else {
                offset as f32 / (rect.height - 1) as f32
            };
            let y = offset as f32;
            let distance = if y < radius {
                radius - y
            } else if y > rect.height as f32 - radius {
                y - (rect.height as f32 - radius)
            } else {
                0.0
            };
            let inset = if distance > 0.0 && radius > 0.0 {
                (radius - (radius * radius - distance * distance).max(0.0).sqrt()).ceil() as i32
            } else {
                0
            };
            self.fill_rect(
                Rect::new(
                    rect.x + inset,
                    rect.y + offset,
                    (rect.width - inset * 2).max(0),
                    1,
                ),
                top.mix(bottom, amount),
            );
        }
    }

    /// Draw an image so it covers `rect`, cropping the longer axis, clipped to
    /// a rounded rectangle. The source is sampled bilinearly by tiny-skia.
    pub fn draw_image_cover(&mut self, rect: Rect, radius: f32, image: &Image) {
        if rect.width <= 0 || rect.height <= 0 || image.width() == 0 || image.height() == 0 {
            return;
        }
        let Some(path) = round_rect_path(rect, radius) else {
            return;
        };
        let scale = (rect.width as f32 / image.width() as f32)
            .max(rect.height as f32 / image.height() as f32);
        let width = image.width() as f32 * scale;
        let height = image.height() as f32 * scale;
        let x = rect.x as f32 + (rect.width as f32 - width) / 2.0;
        let y = rect.y as f32 + (rect.height as f32 - height) / 2.0;
        let transform = Transform::from_translate(x, y).pre_scale(scale, scale);
        let pattern = Pattern::new(
            image.pixmap.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            1.0,
            transform,
        );
        let paint = Paint {
            shader: pattern,
            anti_alias: true,
            ..Default::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub fn fill_horizontal_gradient(&mut self, rect: Rect, left: Color, right: Color) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        for offset in 0..rect.width {
            let amount = if rect.width <= 1 {
                0.0
            } else {
                offset as f32 / (rect.width - 1) as f32
            };
            self.fill_rect(
                Rect::new(rect.x + offset, rect.y, 1, rect.height),
                left.mix(right, amount),
            );
        }
    }

    /// Horizontal gradient clipped to a rounded rectangle.
    pub fn fill_round_horizontal_gradient(
        &mut self,
        rect: Rect,
        radius: f32,
        left: Color,
        right: Color,
    ) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let radius = radius
            .max(0.0)
            .min(rect.width as f32 / 2.0)
            .min(rect.height as f32 / 2.0);
        for offset in 0..rect.width {
            let amount = if rect.width <= 1 {
                0.0
            } else {
                offset as f32 / (rect.width - 1) as f32
            };
            let x = offset as f32;
            let distance = if x < radius {
                radius - x
            } else if x > rect.width as f32 - radius {
                x - (rect.width as f32 - radius)
            } else {
                0.0
            };
            let inset = if distance > 0.0 && radius > 0.0 {
                (radius - (radius * radius - distance * distance).max(0.0).sqrt()).ceil() as i32
            } else {
                0
            };
            self.fill_rect(
                Rect::new(
                    rect.x + offset,
                    rect.y + inset,
                    1,
                    (rect.height - inset * 2).max(0),
                ),
                left.mix(right, amount),
            );
        }
    }

    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, width: f32, color: Color) {
        let mut paint = Paint::default();
        paint.set_color(color.skia());
        paint.anti_alias = true;
        let stroke = SkStroke {
            width: width.max(1.0),
            ..Default::default()
        };
        let mut builder = PathBuilder::new();
        builder.move_to(x1 as f32, y1 as f32);
        builder.line_to(x2 as f32, y2 as f32);
        if let Some(path) = builder.finish() {
            self.pixmap
                .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    pub fn text(&mut self, text: &str, x: i32, baseline: i32, style: TextStyle) -> f32 {
        let font = self.fonts.get(style.weight);
        let mut pen = x as f32;
        for character in text.chars() {
            if character == '\n' {
                continue;
            }
            let (metrics, bitmap) = font.rasterize(character, style.size);
            if metrics.width > 0 && metrics.height > 0 {
                let left = (pen + metrics.xmin as f32).round() as i32;
                let top =
                    (baseline as f32 - metrics.ymin as f32 - metrics.height as f32).round() as i32;
                Self::blend_alpha(
                    &mut self.pixmap,
                    left,
                    top,
                    metrics.width,
                    metrics.height,
                    &bitmap,
                    style.color,
                );
            }
            pen += metrics.advance_width;
        }
        pen - x as f32
    }

    pub fn text_centered(&mut self, text: &str, center_x: i32, baseline: i32, style: TextStyle) {
        let width = self.measure_text(text, style);
        self.text(
            text,
            (center_x as f32 - width / 2.0).round() as i32,
            baseline,
            style,
        );
    }

    pub fn text_right(&mut self, text: &str, right: i32, baseline: i32, style: TextStyle) {
        let width = self.measure_text(text, style);
        self.text(text, (right as f32 - width).round() as i32, baseline, style);
    }

    pub fn measure_text(&self, text: &str, style: TextStyle) -> f32 {
        let font = self.fonts.get(style.weight);
        text.chars()
            .map(|character| font.metrics(character, style.size).advance_width)
            .sum()
    }

    /// Truncate with an ellipsis so cards never bleed outside their bounds.
    pub fn clipped(&self, text: &str, max_width: f32, style: TextStyle) -> String {
        if self.measure_text(text, style) <= max_width {
            return text.to_string();
        }
        let ellipsis = "…";
        let ellipsis_width = self.measure_text(ellipsis, style);
        let mut output = String::new();
        let mut width = 0.0;
        for character in text.chars() {
            let character_width = self.measure_text(&character.to_string(), style);
            if width + character_width + ellipsis_width > max_width {
                break;
            }
            output.push(character);
            width += character_width;
        }
        output.push('…');
        output
    }

    pub fn wrapped(
        &self,
        text: &str,
        max_width: f32,
        max_lines: usize,
        style: TextStyle,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if self.measure_text(&candidate, style) <= max_width {
                current = candidate;
            } else {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    if lines.len() == max_lines {
                        break;
                    }
                }
                current = word.to_string();
            }
        }
        if lines.len() < max_lines && !current.is_empty() {
            lines.push(current);
        }
        if lines.len() > max_lines {
            lines.truncate(max_lines);
        }
        if lines.len() == max_lines {
            let last = lines.last_mut().unwrap();
            if self.measure_text(last, style) > max_width {
                *last = self.clipped(last, max_width, style);
            }
            // If the original text continues beyond the visible words, mark the
            // final line. This is a display hint, not a full text layout engine.
            let shown: String = lines.join(" ");
            if shown.len() < text.len() {
                let last = lines.last_mut().unwrap();
                if !last.ends_with('…') {
                    last.push('…');
                }
            }
        }
        lines
    }

    fn blend_alpha(
        pixmap: &mut Pixmap,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        alpha: &[u8],
        color: Color,
    ) {
        if color.a == 0 {
            return;
        }
        let pixmap_width = pixmap.width();
        let canvas_width = pixmap_width as i32;
        let canvas_height = pixmap.height() as i32;
        let data = pixmap.data_mut();
        for row in 0..height {
            let py = y + row as i32;
            if py < 0 || py >= canvas_height {
                continue;
            }
            for column in 0..width {
                let px = x + column as i32;
                if px < 0 || px >= canvas_width {
                    continue;
                }
                let coverage = alpha[row * width + column] as f32 / 255.0;
                let source_alpha = coverage * color.a as f32 / 255.0;
                if source_alpha <= 0.0 {
                    continue;
                }
                let index = ((py as u32 * pixmap_width + px as u32) * 4) as usize;
                let destination_alpha = data[index + 3] as f32 / 255.0;
                let out_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
                if out_alpha <= 0.0 {
                    continue;
                }
                for channel in 0..3 {
                    let source = match channel {
                        0 => color.r,
                        1 => color.g,
                        _ => color.b,
                    } as f32
                        / 255.0;
                    let destination = data[index + channel] as f32 / 255.0;
                    let value = (source * source_alpha
                        + destination * destination_alpha * (1.0 - source_alpha))
                        / out_alpha;
                    data[index + channel] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
                data[index + 3] = (out_alpha * 255.0).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insets_never_go_negative() {
        let rect = Rect::new(10, 20, 30, 40);
        assert_eq!(rect.inset(5), Rect::new(15, 25, 20, 30));
        assert_eq!(rect.inset(99).width, 0);
        assert_eq!(rect.inset(99).height, 0);
    }

    #[test]
    fn hit_testing_is_half_open() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(rect.contains(0, 0));
        assert!(rect.contains(9, 9));
        assert!(!rect.contains(10, 9));
    }

    #[test]
    fn canvas_draws_and_saves_png() {
        let mut canvas = Canvas::new(320, 180).unwrap();
        canvas.clear(Color::rgb(8, 10, 16));
        canvas.fill_round_rect(Rect::new(20, 20, 120, 60), 12.0, Color::rgb(40, 44, 60));
        canvas.text(
            "ArchivePlayer",
            24,
            100,
            TextStyle::new(28.0, Color::WHITE).bold(),
        );
        assert_eq!(canvas.width(), 320);
        let path = std::env::temp_dir().join("archiveplayer-tv-gfx-test.png");
        canvas.save_png(&path).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn draws_a_cover_image_inside_a_rounded_rect() {
        let mut canvas = Canvas::new(160, 120).unwrap();
        canvas.clear(Color::BLACK);
        let image = Image::from_rgba(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        )
        .unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
        canvas.draw_image_cover(Rect::new(10, 10, 140, 100), 8.0, &image);
        let index = ((60 * 160 + 80) * 4) as usize;
        assert_eq!(&canvas.pixels()[index..index + 4], &[255, 0, 0, 255]);
        assert!(Image::from_rgba(2, 2, vec![0; 3]).is_err());
    }

    #[test]
    fn text_measurement_and_clipping_are_bounded() {
        let mut canvas = Canvas::new(100, 40).unwrap();
        canvas.clear(Color::BLACK);
        let style = TextStyle::new(18.0, Color::WHITE);
        assert!(canvas.measure_text("Hello", style) > 0.0);
        let clipped = canvas.clipped("A very long title that does not fit", 80.0, style);
        assert!(clipped.ends_with('…'));
        assert!(canvas.measure_text(&clipped, style) <= 82.0);
    }
}
