use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use eframe::egui::{self, Key, TextureHandle, Vec2};

fn main() -> Result<()> {
    let initial_image = std::env::args().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Minimal Image Viewer",
        options,
        Box::new(move |cc| {
            let mut app = ViewerApp::default();
            if let Some(path) = initial_image.clone() {
                app.open_image(&path, &cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow!("failed to start UI: {err}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserLayout {
    Icons,
    List,
}

impl Default for BrowserLayout {
    fn default() -> Self {
        Self::Icons
    }
}

#[derive(Default)]
struct ViewerApp {
    current_dir: Option<PathBuf>,
    images: Vec<PathBuf>,
    current_index: usize,
    texture: Option<TextureHandle>,
    thumbnails: HashMap<PathBuf, TextureHandle>,
    show_browser: bool,
    browser_layout: BrowserLayout,
    zoom: f32,
    pan: Vec2,
    fit_to_window: bool,
    error: Option<String>,
}

impl ViewerApp {
    fn open_image(&mut self, selected: &Path, ctx: &egui::Context) {
        if let Err(err) = self.try_open_image(selected, ctx) {
            self.error = Some(err.to_string());
        } else {
            self.error = None;
        }
    }

    fn try_open_image(&mut self, selected: &Path, ctx: &egui::Context) -> Result<()> {
        let selected = fs::canonicalize(selected)
            .with_context(|| format!("cannot access {}", selected.display()))?;
        let dir = selected
            .parent()
            .ok_or_else(|| anyhow!("selected path has no parent"))?
            .to_path_buf();

        if self.current_dir.as_ref() != Some(&dir) {
            self.thumbnails.clear();
        }

        self.images = list_images(&dir)?;
        let index = self
            .images
            .iter()
            .position(|path| path == &selected)
            .ok_or_else(|| anyhow!("selected image is not in allowed extension list"))?;

        self.current_dir = Some(dir);
        self.current_index = index;
        self.load_current_texture(ctx)
    }

    fn load_current_texture(&mut self, ctx: &egui::Context) -> Result<()> {
        let path = self
            .images
            .get(self.current_index)
            .ok_or_else(|| anyhow!("no image selected"))?
            .clone();

        let dyn_img = decode_image(&path)?;
        let rgba = dyn_img.to_rgba8();
        let width = usize::try_from(rgba.width()).context("width conversion failed")?;
        let height = usize::try_from(rgba.height()).context("height conversion failed")?;

        let color = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba.as_raw());
        let name = format!("image:{}", path.display());
        self.texture = Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR));

        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.fit_to_window = true;
        Ok(())
    }

    fn select_index(&mut self, index: usize, ctx: &egui::Context) {
        self.current_index = index;
        if let Err(err) = self.load_current_texture(ctx) {
            self.error = Some(err.to_string());
        } else {
            self.error = None;
        }
    }

    fn select_relative(&mut self, delta: isize, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }

        let len = self.images.len() as isize;
        let current = self.current_index as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.select_index(next, ctx);
    }

    fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.05, 20.0);
        self.fit_to_window = false;
    }

    fn thumbnail_texture(&mut self, path: &Path, ctx: &egui::Context) -> Option<TextureHandle> {
        if let Some(texture) = self.thumbnails.get(path) {
            return Some(texture.clone());
        }

        let thumbnail = decode_image(path).ok()?.thumbnail(160, 160).to_rgba8();
        let width = usize::try_from(thumbnail.width()).ok()?;
        let height = usize::try_from(thumbnail.height()).ok()?;
        let color = egui::ColorImage::from_rgba_unmultiplied([width, height], thumbnail.as_raw());
        let name = format!("thumb:{}", path.display());
        let texture = ctx.load_texture(name, color, egui::TextureOptions::LINEAR);
        self.thumbnails.insert(path.to_path_buf(), texture.clone());
        Some(texture)
    }

    fn render_folder_browser(&mut self, ctx: &egui::Context) {
        let mut selected_index = None;

        egui::Window::new("Folder Browser")
            .open(&mut self.show_browser)
            .default_size([900.0, 640.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Layout:");
                    ui.selectable_value(&mut self.browser_layout, BrowserLayout::Icons, "Icon mode");
                    ui.selectable_value(&mut self.browser_layout, BrowserLayout::List, "List mode");
                });
                ui.separator();

                if self.images.is_empty() {
                    ui.label("No supported images in this folder.");
                    return;
                }

                match self.browser_layout {
                    BrowserLayout::Icons => {
                        let paths = self.images.clone();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (idx, path) in paths.iter().enumerate() {
                                    ui.group(|ui| {
                                        if let Some(texture) = self.thumbnail_texture(path, ctx) {
                                            if ui
                                                .add(egui::ImageButton::new((
                                                    texture.id(),
                                                    egui::vec2(128.0, 128.0),
                                                )))
                                                .clicked()
                                            {
                                                selected_index = Some(idx);
                                            }
                                        } else if ui.button("[preview unavailable]").clicked() {
                                            selected_index = Some(idx);
                                        }

                                        let name = path
                                            .file_name()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_else(|| path.display().to_string());
                                        ui.label(name);
                                    });
                                }
                            });
                        });
                    }
                    BrowserLayout::List => {
                        let paths = self.images.clone();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (idx, path) in paths.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    if let Some(texture) = self.thumbnail_texture(path, ctx) {
                                        ui.add(egui::Image::new((texture.id(), egui::vec2(64.0, 64.0))));
                                    } else {
                                        ui.label("[no thumb]");
                                    }

                                    let name = path
                                        .file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| path.display().to_string());

                                    let label = if idx == self.current_index {
                                        format!("> {name}")
                                    } else {
                                        name
                                    };

                                    if ui.button(label).clicked() {
                                        selected_index = Some(idx);
                                    }
                                });
                                ui.separator();
                            }
                        });
                    }
                }
            });

        if let Some(idx) = selected_index {
            self.select_index(idx, ctx);
            self.show_browser = false;
        }
    }

    fn render_image_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(texture) = self.texture.clone() {
            let available = ui.available_size();
            let (canvas_rect, response) = ui.allocate_exact_size(available, egui::Sense::drag());

            if response.hovered() {
                let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll.abs() > f32::EPSILON {
                    let factor = (scroll / 300.0).exp();
                    self.set_zoom(self.zoom * factor);
                }
            }

            if response.dragged() {
                self.fit_to_window = false;
                self.pan += ctx.input(|i| i.pointer.delta());
            }

            let image_size = texture.size_vec2();
            if self.fit_to_window {
                let fit = (canvas_rect.width() / image_size.x)
                    .min(canvas_rect.height() / image_size.y)
                    .max(0.01);
                self.zoom = fit;
                self.pan = Vec2::ZERO;
            }

            let displayed = image_size * self.zoom;
            let top_left = canvas_rect.center() - displayed * 0.5 + self.pan;
            let image_rect = egui::Rect::from_min_size(top_left, displayed);

            let painter = ui.painter_at(canvas_rect);
            painter.rect_filled(canvas_rect, 0.0, egui::Color32::BLACK);
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::new(0.0, 0.0), egui::Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Open a PNG, JPEG, or JXL image.");
            });
        }
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            self.select_relative(1, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            self.select_relative(-1, ctx);
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "jxl"])
                        .pick_file()
                    {
                        self.open_image(&path, ctx);
                    }
                }

                if ui.button("Prev").clicked() {
                    self.select_relative(-1, ctx);
                }
                if ui.button("Next").clicked() {
                    self.select_relative(1, ctx);
                }

                if ui.button("Zoom +").clicked() {
                    self.set_zoom(self.zoom * 1.25);
                }
                if ui.button("Zoom -").clicked() {
                    self.set_zoom(self.zoom / 1.25);
                }
                if ui.button("Fit").clicked() {
                    self.fit_to_window = true;
                }

                if ui.button("Browse Folder").clicked() {
                    self.show_browser = true;
                }

                ui.separator();
                ui.label("Wheel: zoom, Drag: pan");

                if let Some(current) = self.images.get(self.current_index) {
                    ui.separator();
                    ui.label(current.display().to_string());
                }
            });

            if let Some(err) = &self.error {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

        egui::SidePanel::left("files")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Folder Images");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut clicked_index = None;
                    for (idx, path) in self.images.iter().enumerate() {
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.display().to_string());
                        let selected = idx == self.current_index;
                        if ui.selectable_label(selected, name).clicked() {
                            clicked_index = Some(idx);
                        }
                    }
                    if let Some(idx) = clicked_index {
                        self.select_index(idx, ctx);
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_image_canvas(ui, ctx);
        });

        if self.show_browser {
            self.render_folder_browser(ctx);
        }
    }
}

fn list_images(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_supported_extension(&path) {
            items.push(path);
        }
    }

    items.sort_by(|a, b| {
        let an = a
            .file_name()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let bn = b
            .file_name()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        an.cmp(&bn)
    });

    Ok(items)
}

fn is_supported_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "jxl")
}

fn decode_image(path: &Path) -> Result<image::DynamicImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if ext == "jxl" {
        decode_jxl_with_djxl(path)
    } else {
        image::open(path).with_context(|| format!("cannot decode {}", path.display()))
    }
}

fn decode_jxl_with_djxl(path: &Path) -> Result<image::DynamicImage> {
    let temp_dir = tempfile::tempdir().context("cannot create temp dir")?;
    let converted = temp_dir.path().join("decoded.png");

    let status = Command::new("djxl")
        .arg(path)
        .arg(&converted)
        .status()
        .context("failed to execute djxl (install libjxl-tools)")?;

    if !status.success() {
        return Err(anyhow!("djxl failed on {}", path.display()));
    }

    image::open(&converted)
        .with_context(|| format!("cannot decode converted image for {}", path.display()))
}
