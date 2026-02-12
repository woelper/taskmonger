use eframe::egui;
use egui::containers::menu::MenuConfig;
use egui::{color_picker, Button, Color32, Key, Layout, RichText};
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

use std::cmp::{max, min};

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

trait ReadableText {
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

fn mix_colors(c1: Color32, c2: Color32) -> Color32 {
    Color32::from_rgb(
        ((c1.r() as u16 + c2.r() as u16) / 2) as u8,
        ((c1.g() as u16 + c2.g() as u16) / 2) as u8,
        ((c1.b() as u16 + c2.b() as u16) / 2) as u8,
    )
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

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    #[serde(default)]
    dark_mode: bool,
    #[serde(default)]
    markdown_view_enabled: bool,
    mark_as_background: bool,
}

#[derive(Serialize, Deserialize)]
struct BuffMonster {
    buffer: String,
    #[serde(default)]
    tags: HashMap<String, [u8; 3]>,
    #[serde(default)]
    tagged_ranges: Vec<TaggedRange>,
    settings: Settings,
    #[serde(skip)]
    selection: Range<usize>,
    #[serde(skip)]
    markdown_cache: HashMap<String, egui_commonmark::CommonMarkCache>,
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
            settings: Default::default(),
            selection: Default::default(),
            markdown_cache: HashMap::new(),
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
        let selection = self.selection.clone();

        for tr in self.tagged_ranges.iter_mut() {
            if tr.tag_name == tag_name {
                if tr.range.intersects(&selection) {
                    tr.range = tr.range.union(&selection);
                    return;
                }
            }
        }

        // Just add the range
        self.tagged_ranges
            .push(TaggedRange::new(tag_name.to_string(), selection));

        let _ = self.save_to_disk();
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
        if self.settings.dark_mode {
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
                        let theme_icon = if self.settings.dark_mode { SUN } else { MOON };
                        if ui
                            .button(theme_icon)
                            .on_hover_text("Toggle theme")
                            .clicked()
                        {
                            self.settings.dark_mode = !self.settings.dark_mode;
                            let _ = self.save_to_disk();
                        }

                        if ui
                            .button(FILE_MD)
                            .on_hover_text("Toggle markdown view")
                            .clicked()
                        {
                            self.settings.markdown_view_enabled =
                                !self.settings.markdown_view_enabled;
                            let _ = self.save_to_disk();
                        }
                    });
                });
                ui.separator();

                // Tag adding
                if ui.button("Add tag").clicked() {
                    ctx.memory_mut(|w| w.data.insert_temp("tag".into(), "".to_string()));
                }

                let tag = ctx.memory(|r| r.data.get_temp::<String>("tag".into()));

                if let Some(tag) = tag {
                    egui::Modal::new("Tags".into()).show(ctx, |ui| {
                        ui.set_width(200.0);
                        ui.heading("Add tag");
                        let mut tag_name = tag.clone();
                        let text_edit = ui.text_edit_singleline(&mut tag_name);

                        if text_edit.changed() {
                            ctx.memory_mut(|w| w.data.insert_temp("tag".into(), tag_name.clone()));
                        }
                        ui.memory_mut(|w| w.request_focus(text_edit.id));

                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                            }

                            if ui.button("Add").clicked() {
                                self.add_tag(tag_name.clone());
                                ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                            }

                            if ui.button("Add and assign").clicked() {
                                self.apply_tag_to_selection(&tag);
                                self.add_tag(tag_name);
                                ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                            }
                        });
                    });
                }

                egui::ScrollArea::vertical()
                    .id_salt("tags")
                    .max_height(150.0)
                    .min_scrolled_width(222.)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (tag, c) in self.tags.clone() {
                                let color = to_color32(c);
                                let button = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}", tag))
                                            .color(color.readable_text_color()),
                                    )
                                    .fill(color),
                                );

                                let p = egui::Popup::from_toggle_button_response(&button);
                                p.show(|ui| {
                                    let mut srgba = Color32::from_rgb(c[0], c[1], c[2]);

                                    if !self.selection.is_empty() {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("Assign to selection")
                                                        .color(srgba.readable_text_color()),
                                                )
                                                .fill(srgba),
                                            )
                                            .clicked()
                                        {
                                            self.apply_tag_to_selection(&tag);
                                        }
                                    } else {
                                        ui.label("Select something to assign this tag.");
                                    }
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
                            }
                        });
                    });

                ui.separator();
                ui.label("Tagged ranges:");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut delete_tr: Option<TaggedRange> = None;

                    dnd(ui, "drag_drop").show_vec(
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

        // Markdown view panel (conditional, on the right side of text edit)
        if self.settings.markdown_view_enabled {
            egui::SidePanel::right("markdown_view_panel")
                .resizable(true)
                .default_width(300.0)
                .min_width(200.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Sort tagged ranges by their position in the buffer

                        for tr in &self.tagged_ranges {
                            if tr.range.end <= self.buffer.len() {
                                let text = &self.buffer[tr.range.clone()];

                                ui.group(|ui| {
                                    // Show tag name header with color
                                    if let Some(col) = self.tags.get(&tr.tag_name) {
                                        let color = to_color32(*col);
                                        ui.label(
                                            egui::RichText::new(&tr.tag_name).color(color).strong(),
                                        );
                                    } else {
                                        ui.label(egui::RichText::new(&tr.tag_name).strong());
                                    }

                                    ui.separator();

                                    // Get or create cache for this tagged range
                                    let cache_key = format!(
                                        "{}:{}-{}",
                                        tr.tag_name, tr.range.start, tr.range.end
                                    );
                                    let cache = self
                                        .markdown_cache
                                        .entry(cache_key)
                                        .or_insert_with(egui_commonmark::CommonMarkCache::default);

                                    // Render markdown
                                    egui_commonmark::CommonMarkViewer::new().show(ui, cache, text);
                                });
                                ui.add_space(10.0);
                            }
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut tagged_ranges = self.tagged_ranges.clone();
            let tags = self.tags.clone();

            //  make a default colormap for all chars
            let mut colormap: HashMap<usize, Color32> = Default::default();
            // go though all ranges. If color exists, mix it.
            for tr in &mut tagged_ranges {
                if let Some(col) = tags.get(&tr.tag_name) {
                    for i in &mut tr.range {
                        let x = Color32::from_rgb(col[0], col[1], col[2]);
                        colormap
                            .entry(i)
                            .and_modify(|c| {
                                *c = mix_colors(*c, x);
                            })
                            .or_insert(x);
                    }
                }
            }

            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                let text = text.as_str();
                let mut layout_job = egui::text::LayoutJob::default();
                layout_job.wrap.max_width = wrap_width;

                let default_color = ui.style().visuals.text_color();
                let font_id = egui::FontId::monospace(14.0);

                // TODO: if it is faster, collapse ranges so we need fewer layoutjobs
                // TODO: expose this as setting later
                let background = self.settings.mark_as_background;

                for (i, c) in text.chars().enumerate() {
                    let selected = self.selection.contains(&i);
                    let selected_color = ui.visuals().selection.bg_fill;

                    if let Some(col) = colormap.get(&i) {
                        layout_job.append(
                            &c.to_string(),
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: if background {
                                    if selected {
                                        ui.visuals().selection.stroke.color
                                    } else {
                                        default_color.clone()
                                    }
                                } else {
                                    if selected {
                                        ui.visuals().selection.stroke.color
                                    } else {
                                        col.clone()
                                    }
                                },
                                background: if selected {
                                    selected_color
                                } else {
                                    if background {
                                        col.clone()
                                    } else {
                                        Color32::from_white_alpha(0)
                                    }
                                },
                                ..Default::default()
                            },
                        );
                    } else {
                        // default text
                        layout_job.append(
                            &c.to_string(),
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: if selected {
                                    ui.visuals().selection.stroke.color
                                } else {
                                    default_color.clone()
                                },
                                background: if selected {
                                    selected_color
                                } else {
                                    Color32::from_white_alpha(0)
                                },
                                ..Default::default()
                            },
                        );
                    }
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

            let selection_len = self.selection.len() as i32;

            if let Some(cursor_range) = output.state.cursor.char_range() {
                self.selection = cursor_range.as_sorted_char_range();
            }
            if output.response.changed() {
                println!("len {selection_len}");
                let mut shift: i32 = 0;

                if let Some(range) = output.cursor_range {
                    println!("Cursor range {:?}", range);

                    let keys_down = ctx.input(|i| i.keys_down.clone());
                    let delete = keys_down.iter().nth(0) == Some(&Key::Backspace);

                    if !keys_down.is_empty() {
                        println!("key down {:?}", keys_down);

                        // No selection
                        if selection_len == 0 {
                            println!("Single range Cursor");
                            if delete {
                                shift -= 1;
                            } else {
                                shift += 1;
                            }
                        } else {
                            // let selection_len = range.as_sorted_char_range().len() as i32;
                            println!("Cursor range {:?}, len {selection_len}", range);
                            if delete {
                                shift -= selection_len;
                            } else {
                                shift -= selection_len - 1;
                            }
                        }

                        println!("shift {:?}", shift);

                        for tr in &mut self.tagged_ranges {
                            if tr.range.start > range.primary.index {
                                tr.range.start = (tr.range.start as i32 + shift).abs() as usize;
                            }

                            if tr.range.end > range.primary.index {
                                tr.range.end = (tr.range.end as i32 + shift).abs() as usize;
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
