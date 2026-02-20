use std::cmp::{max, min};
use std::ops::Range;

use egui::Color32;

pub trait RangeExt {
    fn intersects(&self, other: &Self) -> bool;
    fn union(&self, other: &Self) -> Self;
}

impl RangeExt for Range<usize> {
    // Check if ranges overlap
    fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    // Returns the convex hull (the smallest range containing both)
    fn union(&self, other: &Self) -> Self {
        min(self.start, other.start)..max(self.end, other.end)
    }
}

pub fn random_color(num_existing: usize) -> [u8; 3] {
    let c = colorous::WARM.eval_rational(num_existing, 20);
    [c.r, c.g, c.b]
}


pub fn random_color_of(num: usize, total: usize) -> [u8; 3] {
    let c = colorous::RAINBOW.eval_rational(num, total);
    // let c = colorous::WARM.eval_rational(num, total);
    [c.r, c.g, c.b]
}

pub fn to_color32(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

pub trait ReadableText {
    /// Returns a grayscale color that is readable against `self` as a background.
    fn readable_text_color(&self) -> Color32;
}

impl ReadableText for Color32 {
    fn readable_text_color(&self) -> Color32 {
        // Relative luminance using sRGB coefficients
        let luminance = 0.299 * self.r() as f32 + 0.587 * self.g() as f32 + 0.114 * self.b() as f32;
        if luminance > 150.0 {
            Color32::from_gray(30)
        } else {
            Color32::from_gray(230)
        }
    }
}

/// Extracts a markdown list prefix from a line and returns the continuation prefix for the next line.
/// Handles: `- `, `* `, `+ `, `- [ ] `, `- [x] `, `1. `, etc. Preserves indentation.
pub fn extract_list_prefix(line: &str) -> Option<String> {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];

    // Checkbox patterns (most specific, check first)
    for marker in ["-", "*", "+"] {
        for checkbox in ["[ ] ", "[x] ", "[X] "] {
            let pattern = format!("{} {}", marker, checkbox);
            if trimmed.starts_with(&pattern) {
                // Always continue with unchecked checkbox
                return Some(format!("{}{} [ ] ", indent, marker));
            }
        }
    }

    // Unordered list
    for marker in ["-", "*", "+"] {
        let pattern = format!("{} ", marker);
        if trimmed.starts_with(&pattern) {
            return Some(format!("{}{}", indent, pattern));
        }
    }

    // Ordered list (e.g., "1. ", "42. ")
    let digit_end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
    if digit_end > 0 && trimmed[digit_end..].starts_with(". ") {
        if let Ok(num) = trimmed[..digit_end].parse::<usize>() {
            return Some(format!("{}{}. ", indent, num + 1));
        }
    }

    None
}

pub fn mix_colors(c1: Color32, c2: Color32) -> Color32 {
    Color32::from_rgb(
        ((c1.r() as u16 + c2.r() as u16) / 2) as u8,
        ((c1.g() as u16 + c2.g() as u16) / 2) as u8,
        ((c1.b() as u16 + c2.b() as u16) / 2) as u8,
    )
}
