use eframe::egui;
use egui::containers::menu::MenuConfig;
use egui::{color_picker, Button, Color32, Key, Layout};
use egui_dnd::dnd;
use egui_phosphor::regular::*;
use palette::{Hsl, IntoColor, Srgb};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::ops::Range;
use std::path::PathBuf;

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

fn to_color32(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct TaggedRange {
    tag_name: String,
    range: Range<usize>,
}

impl TaggedRange {
    fn new(tag_name: String, range: Range<usize>) -> Self {
        Self { tag_name, range }
    }
}

#[derive(Serialize, Deserialize)]
struct BuffMonster {
    buffer: String,
    #[serde(default)]
    tags: HashMap<String, [u8; 3]>,
    #[serde(default)]
    tagged_ranges: Vec<TaggedRange>,
    // next_id: usize,
    dark_mode: bool,
    #[serde(skip)]
    selection: Range<usize>,
}

impl Default for BuffMonster {
    fn default() -> Self {
        Self {
            buffer: format!(
                "Welcome to {}! \n\nJust start typing here and tag your things.",
                env!("CARGO_PKG_NAME")
            )
            .to_string(),
            tags: Default::default(),
            tagged_ranges: Vec::new(),
            dark_mode: true, // Default to dark mode
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
        fs::write("backup.txt", &self.buffer)?;
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
            eprintln!("No saved state found ({}), starting fresh", e);

            let mut def = Self::default();
            if PathBuf::from("backup.txt").exists() {
                let mut buf: String = Default::default();
                if let Ok(mut f) = File::open(PathBuf::from("backup.txt")) {
                    _ = f.read_to_string(&mut buf);
                    if !buf.is_empty() {
                        eprintln!("Recovered backup");
                        def.buffer = buf;
                    }
                }
            }
            def
        })
    }

    fn add_tag(&mut self, name: String) {
        let name = name.trim().to_string();
        self.tags.insert(name, random_color());
        let _ = self.save_to_disk();
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
                let tagged_range = TaggedRange::new(tag_name.to_string(), range);
                self.tagged_ranges.push(tagged_range);
            }

            let _ = self.save_to_disk();
        }
    }

    fn delete_tagged_range(&mut self, range: &TaggedRange) {
        self.tagged_ranges.retain(|t| t != range);
        let _ = self.save_to_disk();
    }

    fn delete_tag(&mut self, tag_name: &str) {
        self.tags.remove(tag_name);
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

                // Tag adding
                if ui.button("Add tag").clicked() {
                    // ctx.memory_mut(|w| w.data.insert_temp("tag_open".into(), true));
                    ctx.memory_mut(|w| w.data.insert_temp("tag".into(), "".to_string()));
                }

                let tag = ctx.memory(|r| r.data.get_temp::<String>("tag".into()));

                if let Some(tag) = tag {
                    egui::Modal::new("Tags".into()).show(ctx, |ui| {
                        ui.set_width(200.0);
                        ui.heading("Add tag");
                        let mut tag_name = tag.clone();
                        if ui.text_edit_singleline(&mut tag_name).changed() {
                            ctx.memory_mut(|w| w.data.insert_temp("tag".into(), tag_name.clone()));
                        }

                        if ui.button("Add").clicked() {
                            self.add_tag(tag_name);
                            ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                        }

                        if ui.button("Cancel").clicked() {
                            ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                        }

                        ui.add_space(32.0);
                    });
                }

                // ui.allocate_space(dezsired_size)

                egui::ScrollArea::vertical()
                    .id_salt("tags")
                    .max_height(150.0)
                    .min_scrolled_width(222.)
                    .show(ui, |ui| {
                        for (tag, c) in self.tags.clone() {
                            ui.horizontal_wrapped(|ui| {
                                let color = to_color32(c);
                                let button = ui.add(egui::Button::new(
                                    egui::RichText::new(format!("{}", tag)).color(color),
                                ));

                                let p = egui::Popup::from_toggle_button_response(&button);
                                p.show(|ui| {
                                    if !self.selection.is_empty() {
                                        if ui.button("Assign to selection").clicked() {
                                            self.apply_tag_to_selection(&tag);
                                        }
                                    } else {
                                        ui.label("Select something to assign this tag.");
                                    }
                                    let mut srgba = Color32::from_rgb(c[0], c[1], c[2]);
                                    let button = Button::new(format!("Color {ARROW_RIGHT}"))
                                        .fill(srgba.gamma_multiply(0.3));
                                    use egui::containers::menu::SubMenuButton;
                                    SubMenuButton::from_button(button)
                                        .config(MenuConfig::new().close_behavior(
                                            egui::PopupCloseBehavior::CloseOnClickOutside,
                                        ))
                                        .ui(ui, |ui| {
                                            ui.spacing_mut().slider_width = 200.0;
                                            if color_picker::color_picker_color32(
                                                ui,
                                                &mut srgba,
                                                color_picker::Alpha::Opaque,
                                            ) {
                                                if let Some(t) = self.tags.get_mut(&tag) {
                                                    t[0] = srgba.r();
                                                    t[1] = srgba.g();
                                                    t[2] = srgba.b();
                                                }
                                            }
                                        });

                                    if ui.button(TRASH).clicked() {
                                        self.delete_tag(&tag);
                                    }
                                });
                            });
                        }
                    });

                ui.separator();
                ui.label("Tagged ranges:");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut delete_tr: Option<TaggedRange> = None;

                    dnd(ui, "dnd_example").show_vec(
                        &mut self.tagged_ranges,
                        |ui, item, handle, state| {
                            ui.horizontal(|ui| {
                                handle.ui(ui, |ui| {
                                    if state.dragged {
                                        ui.label("-");
                                    } else {
                                        ui.label(DOTS_SIX_VERTICAL);
                                    }
                                });

                                let preview: String = self
                                    .buffer
                                    .chars()
                                    .skip(item.range.start)
                                    .take(item.range.end - item.range.start)
                                    .take_while(|c| c != &'\n')
                                    .take(30)
                                    .collect();

                                if let Some(col) = &self.tags.get(&item.tag_name) {
                                    let color = to_color32(**col);
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}: {}",
                                            item.tag_name, preview
                                        ))
                                        .color(color),
                                    );
                                } else {
                                    ui.label(format!("{}: {}", item.tag_name, preview));
                                }
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            // TODO: add button to scroll to this range
                                            if ui.small_button(TRASH).clicked() {
                                                delete_tr = Some(item.clone());
                                            }
                                        },
                                    );
                                });
                            });
                        },
                    );
                    if let Some(r) = delete_tr {
                        self.delete_tagged_range(&r);
                    };
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
                            .get(&tr.tag_name)
                            .map(|tag| to_color32(*tag))
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

    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    eframe::run_native(
        "BuffMonster",
        native_options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(BuffMonster::new(cc)))
        }),
    )
}
