use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use eframe::egui::{self, Key, TextureHandle};

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

#[derive(Default)]
struct ViewerApp {
    current_dir: Option<PathBuf>,
    images: Vec<PathBuf>,
    current_index: usize,
    texture: Option<TextureHandle>,
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
        Ok(())
    }

    fn select_relative(&mut self, delta: isize, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }

        let len = self.images.len() as isize;
        let current = self.current_index as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.current_index = next;
        if let Err(err) = self.load_current_texture(ctx) {
            self.error = Some(err.to_string());
        } else {
            self.error = None;
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

                if let Some(current) = self.images.get(self.current_index) {
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
                        self.current_index = idx;
                        if let Err(err) = self.load_current_texture(ctx) {
                            self.error = Some(err.to_string());
                        } else {
                            self.error = None;
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                let available = ui.available_size();
                let image_size = texture.size_vec2();
                let scale = (available.x / image_size.x)
                    .min(available.y / image_size.y)
                    .max(0.01);
                let desired = image_size * scale;

                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new((texture.id(), desired)));
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a PNG, JPEG, or JXL image.");
                });
            }
        });
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
