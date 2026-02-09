use eframe::egui;
use egui::{vec2, Key, Sense, TextBuffer};
use palette::{Hsl, IntoColor, Srgb};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::ops::Range;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tag {
    name: String,
    color: [u8; 3],
}

impl Tag {
    fn new(name: String) -> Self {
        Self {
            name,
            color: Self::random_color(),
        }
    }

    fn random_color() -> [u8; 3] {
        let mut rng = rand::rng();

        // Generate color in HSL space for better perceptual distribution
        let hue = rng.random_range(0.0..360.0);
        let saturation = rng.random_range(0.5..0.8); // Medium to high saturation
        let lightness = rng.random_range(0.5..0.7); // Medium lightness for readability

        let hsl = Hsl::new(hue, saturation, lightness);
        let rgb: Srgb = hsl.into_color();

        [
            (rgb.red * 255.0) as u8,
            (rgb.green * 255.0) as u8,
            (rgb.blue * 255.0) as u8,
        ]
    }

    fn to_color32(&self) -> egui::Color32 {
        egui::Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct TaggedRange {
    tag_name: String,
    range: Range<usize>,
}

impl TaggedRange {
    fn new(tag_name: String, range: Range<usize>, _id: usize) -> Self {
        Self { tag_name, range }
    }
}

#[derive(Serialize, Deserialize)]
struct BuffMonster {
    buffer: String,
    tags: Vec<Tag>,
    tagged_ranges: Vec<TaggedRange>,
    next_id: usize,
    dark_mode: bool,
    #[serde(skip)]
    new_tag_name: String,
    #[serde(skip)]
    selection: Range<usize>,
}

impl Default for BuffMonster {
    fn default() -> Self {
        Self {
            buffer: "Welcome to BuffMonster!\n\nCreate tags and apply them to text ranges.\n"
                .to_string(),
            tags: vec![],
            tagged_ranges: Vec::new(),
            next_id: 0,
            dark_mode: true, // Default to dark mode
            new_tag_name: String::new(),
            selection: Default::default(),
        }
    }
}

impl BuffMonster {
    fn save_path() -> PathBuf {
        // Save in the current directory for simplicity
        // Could use dirs crate for a proper config directory
        PathBuf::from("buffmonster_state.json")
    }

    fn save_to_disk(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(Self::save_path(), json)?;
        println!("Saved state to {}", Self::save_path().display());
        Ok(())
    }

    fn load_from_disk() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::save_path();
        if path.exists() {
            let json = fs::read_to_string(&path)?;
            let mut app: Self = serde_json::from_str(&json)?;
            println!("Loaded state from {}", path.display());
            // Clean up any invalid ranges that might have been saved
            app.clean_invalid_ranges();
            Ok(app)
        } else {
            Err("Save file does not exist".into())
        }
    }

    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Try to load from disk, fallback to default
        Self::load_from_disk().unwrap_or_else(|e| {
            println!("No saved state found ({}), starting fresh", e);
            Self::default()
        })
    }

    fn add_tag(&mut self, name: String) {
        let name = name.trim().to_string();
        if !name.is_empty() && !self.tags.iter().any(|t| t.name == name) {
            self.tags.push(Tag::new(name));
            let _ = self.save_to_disk();
        }
    }

    fn get_tag(&self, name: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.name == name)
    }

    fn apply_tag_to_selection(&mut self, tag_name: &str) {
        let range = if self.selection.start < self.selection.end {
            self.selection.start..self.selection.end
        } else {
            self.selection.end..self.selection.start
        };

        if range.start < range.end && range.end <= self.buffer.len() {
            // Check if this range is fully enclosed in an existing tag of the same name
            let is_enclosed = self.tagged_ranges.iter().any(|tr| {
                tr.tag_name == tag_name
                    && tr.range.start <= range.start
                    && tr.range.end >= range.end
            });

            if is_enclosed {
                // Already fully tagged, don't create a duplicate
                return;
            }

            // Check for overlapping ranges with the same tag and merge them
            let mut merged = false;
            for tr in &mut self.tagged_ranges {
                if tr.tag_name == tag_name {
                    // Check if ranges overlap or are adjacent
                    let overlaps = (range.start <= tr.range.end && range.end >= tr.range.start)
                        || (range.start == tr.range.end || range.end == tr.range.start);

                    if overlaps {
                        // Merge by extending the existing range
                        tr.range.start = tr.range.start.min(range.start);
                        tr.range.end = tr.range.end.max(range.end);
                        merged = true;
                        break;
                    }
                }
            }

            if !merged {
                // No overlap found, create a new tagged range
                let tagged_range = TaggedRange::new(tag_name.to_string(), range, self.next_id);
                self.next_id += 1;
                self.tagged_ranges.push(tagged_range);
            }

            let _ = self.save_to_disk();
        }
    }

    fn delete_tagged_range(&mut self, range: &TaggedRange) {
        // self.tagged_ranges
        //     .retain(|t| t.tag_name != range.tag_name && t.range != range.range);
        
        self.tagged_ranges
            .retain(|t| t != range);
        let _ = self.save_to_disk();
    }

    fn delete_tag(&mut self, tag_name: &str) {
        self.tags.retain(|t| t.name != tag_name);
        self.tagged_ranges.retain(|tr| tr.tag_name != tag_name);
        let _ = self.save_to_disk();
    }

    fn clean_invalid_ranges(&mut self) {
        let buffer_len = self.buffer.len();
        // Remove ranges that are completely out of bounds or invalid
        self.tagged_ranges.retain(|tr| {
            tr.range.start < buffer_len
                && tr.range.end <= buffer_len
                && tr.range.start < tr.range.end
        });
        // Clamp ranges that extend beyond the buffer
        for tr in &mut self.tagged_ranges {
            if tr.range.end > buffer_len {
                tr.range.end = buffer_len;
            }
            if tr.range.start > buffer_len {
                tr.range.start = buffer_len;
            }
        }
    }
}

impl eframe::App for BuffMonster {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply the theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        egui::SidePanel::right("tags_panel")
            .min_width(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Tags");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let theme_icon = if self.dark_mode { "☀" } else { "🌙" };
                        if ui
                            .button(theme_icon)
                            .on_hover_text("Toggle theme")
                            .clicked()
                        {
                            self.dark_mode = !self.dark_mode;
                            let _ = self.save_to_disk();
                        }
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_tag_name);
                    if !self.new_tag_name.is_empty() {
                        if ui.button("+").clicked() {
                            self.add_tag(self.new_tag_name.clone());
                            self.new_tag_name.clear();
                        }
                    } else {
                        ui.label("Add a tag");
                    }
                });

                ui.separator();

                let tags_clone = self.tags.clone();
                egui::ScrollArea::vertical()
                    .id_salt("tags")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for tag in tags_clone.iter() {
                            ui.horizontal(|ui| {
                                let color = tag.to_color32();
                                let button = egui::Button::new(
                                    egui::RichText::new(format!("{}", tag.name)).color(color),
                                );
                                if ui.add(button).clicked() {
                                    self.apply_tag_to_selection(&tag.name);
                                }
                                if ui.small_button("x").clicked() {
                                    self.delete_tag(&tag.name);
                                }
                            });
                        }
                    });

                ui.separator();
                ui.label("Tagged ranges:");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for tr in self.tagged_ranges.clone().iter() {
                        ui.group(|ui| {
                            ui.allocate_at_least(vec2(ui.available_width(), 0.), Sense::empty());

                            let preview: String = self
                                .buffer
                                .chars()
                                .skip(tr.range.start)
                                .take(tr.range.end - tr.range.start)
                                .take(30)
                                .collect();

                            if let Some(tag) = self.get_tag(&tr.tag_name) {
                                let color = tag.to_color32();
                                ui.label(
                                    egui::RichText::new(format!("{}: {}", tr.tag_name, preview))
                                        .color(color),
                                );
                            } else {
                                ui.label(format!("{}: {}", tr.tag_name, preview));
                            }

                            #[cfg(debug_assertions)]
                            {
                                ui.label(format!("{}: {:?}", tr.tag_name, tr.range));
                            }

                            ui.horizontal(|ui| {
                                // if ui.small_button("Select").clicked() {
                                //     self.selection = tr.range.clone();
                                // }
                                if ui.small_button("Delete").clicked() {
                                    self.delete_tagged_range(&tr);
                                }
                            });
                        });
                        ui.add_space(5.0);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let tagged_ranges = self.tagged_ranges.clone();
            let tags = self.tags.clone();

            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                let text = text.as_str();
                let mut layout_job = egui::text::LayoutJob::default();
                layout_job.wrap.max_width = wrap_width;

                let mut last_pos = 0;
                let default_color = ui.style().visuals.text_color();
                let font_id = egui::FontId::monospace(14.0);

                let mut sorted_ranges = tagged_ranges.clone();
                sorted_ranges.sort_by_key(|tr| tr.range.start);

                for tr in sorted_ranges.iter() {
                    // Skip ranges that overlap with already-rendered text
                    if tr.range.start < last_pos {
                        continue;
                    }

                    if tr.range.start > last_pos && last_pos < text.len() {
                        let end = tr.range.start.min(text.len());
                        layout_job.append(
                            &text[last_pos..end],
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: default_color,
                                ..Default::default()
                            },
                        );
                    }

                    if tr.range.end <= text.len() {
                        let background_color = tags
                            .iter()
                            .find(|tag| tag.name == tr.tag_name)
                            .map(|tag| tag.to_color32())
                            .unwrap_or(egui::Color32::TRANSPARENT)
                            .gamma_multiply(0.2);

                        layout_job.append(
                            &text[tr.range.clone()],
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: default_color,
                                background: background_color,
                                ..Default::default()
                            },
                        );
                        last_pos = tr.range.end;
                    }
                }

                if last_pos < text.len() {
                    layout_job.append(
                        &text[last_pos..],
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: default_color,
                            ..Default::default()
                        },
                    );
                }

                ui.fonts_mut(|f| f.layout_job(layout_job))
            };

            let output = egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.buffer)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .frame(false)
                        .font(egui::TextStyle::Monospace)
                        .layouter(&mut layouter)
                        .show(ui)
                })
                .inner;

            if let Some(cursor_range) = output.cursor_range {
                self.selection = cursor_range.primary.index..cursor_range.secondary.index;
            }

            if output.response.changed() {
                if let Some(range) = output.cursor_range {
                    let keys_down = ctx.input(|i| i.keys_down.clone());

                    if !keys_down.is_empty() {
                        println!("key down");

                        if let Some(single) = range.single() {
                            println!("Cursor at {}", single.index);

                            for tr in &mut self.tagged_ranges {
                                if tr.range.contains(&single.index) {
                                    println!("need to replace {:?}", range);
                                    if keys_down.iter().nth(0) == Some(&Key::Backspace) {
                                        tr.range.end -= 1;
                                    } else {
                                        tr.range.end += 1;
                                    }
                                }
                            }
                        } else {
                            // multiple chars selected
                            let sel_start = range.primary.index.min(range.secondary.index);
                            let sel_end = range.primary.index.max(range.secondary.index);
                            let selected_range = sel_start..sel_end;
                            // Find and update tagged ranges that match or overlap the selection
                            for tr in &mut self.tagged_ranges {
                                // Check if the tagged range matches the selection exactly
                                if tr.range == selected_range {
                                    println!("Updating tagged range {:?} for selection", tr.range);

                                    if keys_down.iter().nth(0) == Some(&Key::Backspace) {
                                        tr.range = sel_start..sel_start;
                                    } else {
                                        tr.range = sel_start..(sel_start + 1);
                                    }
                                } else if tr.range.contains(&sel_start)
                                    && tr.range.contains(&(sel_end.saturating_sub(1)))
                                {
                                    // Tagged range contains the selection - adjust the end
                                    let selection_len = sel_end - sel_start;
                                    println!(
                                        "Adjusting tagged range {:?} that contains selection",
                                        tr.range
                                    );

                                    if keys_down.iter().nth(0) == Some(&Key::Backspace) {
                                        // Selection deleted, no replacement
                                        tr.range.end = tr.range.end.saturating_sub(selection_len);
                                    } else {
                                        // Selection replaced with 1 character
                                        tr.range.end =
                                            tr.range.end.saturating_sub(selection_len - 1);
                                    }
                                }
                            }
                        }
                    }
                }

                // Clean up invalid ranges and auto-save on text changes
                self.clean_invalid_ranges();
                let _ = self.save_to_disk();
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("BuffMonster"),
        ..Default::default()
    };

    eframe::run_native(
        "BuffMonster",
        native_options,
        Box::new(|cc| Ok(Box::new(BuffMonster::new(cc)))),
    )
}
