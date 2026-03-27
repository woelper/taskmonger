use crate::tools::{mix_colors, random_color, random_color_of, to_color32, ReadableText};
use eframe::egui;
use egui::containers::menu::MenuConfig;
use egui::{color_picker, Button, Color32, Key, Layout};
use egui_dnd::dnd;
use egui_phosphor::regular::*;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
mod tools;
use egui::containers::menu::SubMenuButton;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, Default)]
struct Filter {
    text: String,
    active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct Task {
    buffer: String,
    tag_names: Vec<String>,
    #[serde(default)]
    created: chrono::NaiveDateTime,
    #[serde(default)]
    modified: chrono::NaiveDateTime,
    #[serde(default)]
    hide: bool,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    relates_to: Vec<String>,
}

impl Task {
    fn new(buffer: String) -> Self {
        Self {
            buffer,
            tag_names: Vec::new(),
            created: chrono::Utc::now().naive_local(),
            modified: chrono::Utc::now().naive_local(),
            hide: false,
            completed: false,
            relates_to: Vec::new(),
        }
    }

    fn mark(&mut self) {
        self.modified = chrono::Utc::now().naive_local();
    }

    /// Returns the mixed color for this task based on its tags
    fn color(&self, tags: &HashMap<String, [u8; 3]>) -> Option<Color32> {
        let mut result: Option<Color32> = None;
        for tag_name in &self.tag_names {
            if let Some(col) = tags.get(tag_name) {
                let c = to_color32(*col);
                result = Some(match result {
                    Some(existing) => mix_colors(existing, c),
                    None => c,
                });
            }
        }
        result
    }

    /// First line preview, truncated
    fn preview(&self, max_chars: usize) -> String {
        self.buffer
            .chars()
            .take_while(|c| c != &'\n')
            .take(max_chars)
            .collect()
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
struct Taskmonger {
    tasks: Vec<Task>,
    #[serde(default)]
    tags: HashMap<String, [u8; 3]>,
    settings: Settings,
    #[serde(skip)]
    markdown_cache: HashMap<String, egui_commonmark::CommonMarkCache>,
    #[serde(default)]
    filter: Filter,
}

impl Default for Taskmonger {
    fn default() -> Self {
        Self {
            tasks: vec![Task::new(format!(
                "Welcome to {}! \n\nJust start typing here and tag your things.",
                env!("CARGO_PKG_NAME")
            ))],
            tags: Default::default(),
            settings: Default::default(),
            markdown_cache: HashMap::new(),
            filter: Filter::default(),
        }
    }
}

impl Taskmonger {
    fn save_path() -> PathBuf {
        PathBuf::from("taskmonger_state.json")
    }

    fn save_to_disk(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        // Backup all task buffers as plain text
        let backup: String = self
            .tasks
            .iter()
            .map(|t| t.buffer.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");
        fs::write("backup.txt", &backup)?;
        fs::write(Self::save_path(), json)?;
        debug!("Saved state to {}", Self::save_path().display());
        Ok(())
    }

    fn load_from_disk() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::save_path();
        if path.exists() {
            let json = fs::read_to_string(&path)?;
            let app: Self = serde_json::from_str(&json)?;
            debug!("Loaded state from {}", path.display());
            Ok(app)
        } else {
            Err("Save file does not exist".into())
        }
    }

    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::load_from_disk().unwrap_or_else(|e| {
            debug!("No saved state found ({}), starting fresh", e);
            let mut def = Self::default();
            if PathBuf::from("backup.txt").exists() {
                let mut buf: String = Default::default();
                if let Ok(mut f) = File::open(PathBuf::from("backup.txt")) {
                    _ = f.read_to_string(&mut buf);
                    if !buf.is_empty() {
                        debug!("Recovered backup");
                        def.tasks = vec![Task::new(buf)];
                    }
                }
            }
            def
        })
    }

    fn add_tag(&mut self, name: String) {
        let name = name.trim().to_string();
        self.tags.insert(name, random_color(self.tags.len()));
        let _ = self.save_to_disk();
    }

    fn delete_tag(&mut self, tag_name: &str) {
        self.tags.remove(tag_name);
        // Remove this tag from all tasks
        for task in &mut self.tasks {
            task.tag_names.retain(|t| t != tag_name);
        }
        let _ = self.save_to_disk();
    }

    /// Ensure there's always at least one task
    fn ensure_default_task(&mut self) {
        if self.tasks.is_empty() {
            self.tasks.push(Task::new(String::new()));
        }
    }
}

impl eframe::App for Taskmonger {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_default_task();

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

                        let button = ui.add(egui::Button::new(GEAR));
                        let p = egui::Popup::from_toggle_button_response(&button);
                        p.show(|ui| {
                            ui.vertical_centered_justified(|ui| {
                                if ui.button("Assign palette colors").clicked() {
                                    let num_tags = self.tags.len();
                                    for (i, t) in self.tags.iter_mut().enumerate() {
                                        *t.1 = random_color_of(i, num_tags);
                                    }
                                }
                                ui.checkbox(
                                    &mut self.settings.mark_as_background,
                                    "Mark text background",
                                );
                            });
                        });

                        if ui.text_edit_singleline(&mut self.filter.text).changed() {}
                    });
                });
                ui.separator();

                let tag = ctx.memory(|r| r.data.get_temp::<String>("tag".into()));

                if let Some(tag) = tag {
                    egui::Modal::new("Tags".into()).show(ctx, |ui| {
                        ui.set_width(200.0);
                        ui.heading("Add tag");
                        let mut tag_name = tag.clone();

                        ui.vertical_centered_justified(|ui| {
                            let text_edit = ui.text_edit_singleline(&mut tag_name);
                            if text_edit.changed() {
                                ctx.memory_mut(|w| {
                                    w.data.insert_temp("tag".into(), tag_name.clone())
                                });
                            }
                            ui.memory_mut(|w| w.request_focus(text_edit.id));
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Close").clicked() {
                                ctx.memory_mut(|w| w.data.remove_temp::<String>("tag".into()));
                            }

                            if ui.button("Add").clicked() {
                                self.add_tag(tag_name.clone());
                            }

                            if ui.button("Add & close").clicked() {
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
                                        egui::RichText::new(tag.to_string())
                                            .color(color.readable_text_color()),
                                    )
                                    .fill(color),
                                );

                                let p = egui::Popup::from_toggle_button_response(&button);
                                p.show(|ui| {
                                    let mut srgba = Color32::from_rgb(c[0], c[1], c[2]);

                                    ui.vertical_centered_justified(|ui| {
                                        let button = Button::new(format!("Color {ARROW_RIGHT}"))
                                            .fill(srgba.gamma_multiply(0.3));
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
                                                        *t = [srgba.r(), srgba.g(), srgba.b()];
                                                    }
                                                }
                                            });
                                        if ui.button("Rand col").clicked() {
                                            if let Some(t) = self.tags.get_mut(&tag) {
                                                *t = random_color(
                                                    rand::random_range(0..40) as usize
                                                );
                                            }
                                        }

                                        if ui.button(TRASH).clicked() {
                                            self.delete_tag(&tag);
                                        }
                                    });
                                });
                            }
                        });
                    });

                // Tag adding
                ui.vertical_centered_justified(|ui| {
                    if ui.button("Add tag").clicked() {
                        ctx.memory_mut(|w| w.data.insert_temp("tag".into(), "".to_string()));
                    }
                });

                ui.separator();
                ui.label("Tasks:");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut delete_idx: Option<usize> = None;

                    dnd(ui, "task_drag_drop").show_vec(
                        &mut self.tasks,
                        |ui, task, handle, state| {
                            if task.completed {
                                return;
                            }
                            ui.horizontal(|ui| {
                                handle.ui(ui, |ui| {
                                    if state.dragged {
                                        ui.label("-");
                                    } else {
                                        ui.label(DOTS_SIX_VERTICAL);
                                    }
                                });

                                let preview = task.preview(30);

                                // Show with mixed tag color if any
                                if let Some(color) = task.color(&self.tags) {
                                    let tag_labels: String = task.tag_names.join(", ");
                                    if tag_labels.is_empty() {
                                        ui.label(egui::RichText::new(&preview).color(color));
                                    } else {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}: {}",
                                                tag_labels, preview
                                            ))
                                            .color(color),
                                        );
                                    }
                                } else {
                                    ui.label(&preview);
                                }

                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button(TRASH).clicked() {
                                                delete_idx = Some(state.index);
                                            }
                                            let icon = if task.hide { EYE_CLOSED } else { EYE };
                                            if ui.small_button(icon).clicked() {
                                                task.hide = !task.hide;
                                            }
                                        },
                                    );
                                });
                            });
                        },
                    );
                    if let Some(idx) = delete_idx {
                        self.tasks.remove(idx);
                        self.ensure_default_task();
                        let _ = self.save_to_disk();
                    }
                });

                // Completed tasks section
                let completed_count = self.tasks.iter().filter(|t| t.completed).count();
                if completed_count > 0 {
                    let id = ui.make_persistent_id("completed_tasks");
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ctx, id, false,
                    )
                    .show_header(ui, |ui| {
                        ui.label(format!("Completed ({})", completed_count));
                    })
                    .body(|ui| {
                        let mut restore_idx: Option<usize> = None;
                        for (i, task) in self.tasks.iter().enumerate() {
                            if !task.completed {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                let preview = task.preview(30);
                                let label = if let Some(color) = task.color(&self.tags) {
                                    egui::RichText::new(&preview).color(color).strikethrough()
                                } else {
                                    egui::RichText::new(&preview).strikethrough()
                                };
                                ui.label(label);
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .small_button(ARROW_COUNTER_CLOCKWISE)
                                        .on_hover_text("Restore")
                                        .clicked()
                                    {
                                        restore_idx = Some(i);
                                    }
                                });
                            });
                        }
                        if let Some(idx) = restore_idx {
                            self.tasks[idx].completed = false;
                            self.tasks[idx].mark();
                            let _ = self.save_to_disk();
                        }
                    });
                }
            });

        // Markdown view panel
        if self.settings.markdown_view_enabled {
            egui::SidePanel::right("markdown_view_panel")
                .resizable(true)
                .default_width(300.0)
                .min_width(200.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, task) in self.tasks.iter().enumerate() {
                            if task.hide || task.completed {
                                continue;
                            }
                            ui.group(|ui| {
                                // Show tag names header with color
                                if let Some(color) = task.color(&self.tags) {
                                    let label = if task.tag_names.is_empty() {
                                        "Untagged".to_string()
                                    } else {
                                        task.tag_names.join(", ")
                                    };
                                    ui.label(egui::RichText::new(label).color(color).strong());
                                } else {
                                    ui.label(egui::RichText::new("Untagged").strong());
                                }
                                ui.separator();

                                let cache_key = format!("task_{}", i);
                                let cache = self.markdown_cache.entry(cache_key).or_default();
                                egui_commonmark::CommonMarkViewer::new().show(
                                    ui,
                                    cache,
                                    &task.buffer,
                                );
                            });
                            ui.add_space(10.0);
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let tags_clone = self.tags.clone();
            let mut any_changed = false;

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut new_task_after: Option<usize> = None;

                for (i, task) in self.tasks.iter_mut().enumerate() {
                    if task.hide || task.completed {
                        continue;
                    }

                    // Filter: skip tasks that don't match
                    if !self.filter.text.is_empty()
                        && !task
                            .buffer
                            .to_lowercase()
                            .contains(&self.filter.text.to_lowercase())
                        && !task
                            .tag_names
                            .iter()
                            .any(|t| t.to_lowercase().contains(&self.filter.text.to_lowercase()))
                    {
                        continue;
                    }

                    let task_color = task.color(&tags_clone);

                    // Reserve a background shape slot before the text edit
                    let bg_idx = ui.painter().add(egui::Shape::Noop);

                    let text_edit = egui::TextEdit::multiline(&mut task.buffer)
                        .id_salt(format!("task_edit_{}", i))
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .lock_focus(false)
                        .frame(false)
                        .font(egui::TextStyle::Monospace);

                    let output = text_edit.show(ui);

                    // Paint background behind text
                    let mult = if i % 2 == 0 { 0.12 } else { 0.20 };
                    if let Some(color) = task_color {
                        let mult = if output.response.hovered() { mult + 0.05 } else { mult };
                        let bg = color.gamma_multiply(mult);
                        ui.painter().set(
                            bg_idx,
                            egui::Shape::rect_filled(output.response.rect, 0.0, bg),
                        );
                    }

                    if output.response.changed() {
                        task.mark();
                        any_changed = true;

                        // Markdown list continuation
                        if let Some(range) = output.cursor_range {
                            let keys_down = ctx.input(|i| i.keys_down.clone());
                            let enter = keys_down.contains(&Key::Enter);

                            if enter {
                                let cursor_char_pos = range.primary.index;
                                let cursor_byte_pos = task
                                    .buffer
                                    .char_indices()
                                    .nth(cursor_char_pos)
                                    .map(|(i, _)| i)
                                    .unwrap_or(task.buffer.len());

                                if cursor_byte_pos > 0
                                    && task.buffer.as_bytes()[cursor_byte_pos - 1] == b'\n'
                                {
                                    let before_newline = cursor_byte_pos - 1;
                                    let line_start = task.buffer[..before_newline]
                                        .rfind('\n')
                                        .map(|i| i + 1)
                                        .unwrap_or(0);
                                    let previous_line =
                                        task.buffer[line_start..before_newline].to_string();

                                    if let Some(prefix) = tools::extract_list_prefix(&previous_line)
                                    {
                                        let prefix_char_len = prefix.chars().count();
                                        task.buffer.insert_str(cursor_byte_pos, &prefix);

                                        let new_cursor_pos = cursor_char_pos + prefix_char_len;
                                        let mut state = output.state.clone();
                                        state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::one(
                                                egui::text::CCursor::new(new_cursor_pos),
                                            ),
                                        ));
                                        state.store(ctx, output.response.id);
                                    }
                                }
                            }
                        }
                    }

                    // Checkbox hover and toggle
                    if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                        if output.response.hovered() {
                            let cursor = output.galley.cursor_from_pos(pos - output.galley_pos);
                            let char_idx = cursor.index;

                            if let Some((_middle_idx, is_checked)) =
                                tools::find_checkbox_at(&task.buffer, char_idx)
                            {
                                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);

                                if ctx.input(|i| i.pointer.primary_clicked()) {
                                    let byte_idx = task
                                        .buffer
                                        .char_indices()
                                        .nth(_middle_idx)
                                        .map(|(i, _)| i)
                                        .unwrap();
                                    let new_char = if is_checked { " " } else { "x" };
                                    task.buffer.replace_range(byte_idx..byte_idx + 1, new_char);
                                    task.mark();
                                    any_changed = true;
                                }
                            }
                        }
                    }

                    // Context menu for task
                    let task_tags = task.tag_names.clone();
                    output.response.context_menu(|ui| {
                        // Assign tags
                        for (tag, c) in &tags_clone {
                            let color = to_color32(*c);
                            let already_assigned = task_tags.contains(tag);
                            let label = if already_assigned {
                                format!("{CHECK} {}", tag)
                            } else {
                                tag.to_string()
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(label)
                                            .color(color.readable_text_color()),
                                    )
                                    .fill(color),
                                )
                                .clicked()
                            {
                                if already_assigned {
                                    task.tag_names.retain(|t| t != tag);
                                } else {
                                    task.tag_names.push(tag.clone());
                                }
                                any_changed = true;
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button(format!("{CHECK} Mark completed")).clicked() {
                            task.completed = true;
                            task.mark();
                            any_changed = true;
                            ui.close();
                        }
                        if ui.button(format!("{PLUS} New task after this")).clicked() {
                            new_task_after = Some(i);
                            ui.close();
                        }
                    });

                }

                // Add new task button
                ui.vertical_centered_justified(|ui| {
                    if ui.button(format!("{PLUS} New task")).clicked() {
                        new_task_after = Some(self.tasks.len().saturating_sub(1));
                    }
                });

                if let Some(idx) = new_task_after {
                    self.tasks.insert(idx + 1, Task::new(String::new()));
                    any_changed = true;
                }
            });

            if any_changed {
                let _ = self.save_to_disk();
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let icon_rgba = image::load_from_memory(include_bytes!("../icon.png"))
        .expect("Failed to load icon")
        .to_rgba8();
    let (width, height) = icon_rgba.dimensions();
    let icon_data = egui::IconData {
        rgba: icon_rgba.into_raw(),
        width,
        height,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("Taskmonger")
            .with_icon(icon_data),
        ..Default::default()
    };

    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "IBMPlexSans".to_owned(),
        egui::FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "IBMPlexMono".to_owned(),
        egui::FontData::from_static(include_bytes!("../fonts/IBMPlexMono-Regular.ttf")).into(),
    );

    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "IBMPlexSans".to_owned());
    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .insert(0, "IBMPlexMono".to_owned());

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    eframe::run_native(
        "Taskmonger",
        native_options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(Taskmonger::new(cc)))
        }),
    )
}
