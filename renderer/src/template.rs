use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const DEFAULT_TEMPLATE_ID: &str = "model-system";

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetTemplate {
    pub name: String,
    pub description: String,
    pub outer_margin: u16,
    pub model: ModelTemplate,
    pub system: SystemTemplate,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelTemplate {
    pub enabled: bool,
    pub asset: String,
    pub scale: f32,
    pub pan: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub flip_horizontal: bool,
    pub texture_filter: TextureFilterChoice,
    pub texture_lighting: bool,
    pub color_brightness: f32,
    pub backface_culling: bool,
    pub light_direction: [f32; 3],
    pub diffuse_light: f32,
    pub ambient_light: f32,
    pub animation_index: usize,
    pub animation_speed: f32,
    pub animation_loop_blend_seconds: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SystemTemplate {
    pub enabled: bool,
    pub width_percent: u16,
    pub horizontal_position: HorizontalPosition,
    pub horizontal_alignment: HorizontalAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TextureFilterChoice {
    #[default]
    Nearest,
    Bilinear,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HorizontalPosition {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HorizontalAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerticalAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

impl Default for WidgetTemplate {
    fn default() -> Self {
        Self {
            name: "System information".into(),
            description: String::new(),
            outer_margin: 2,
            model: ModelTemplate::default(),
            system: SystemTemplate::default(),
        }
    }
}

impl Default for ModelTemplate {
    fn default() -> Self {
        Self {
            enabled: false,
            asset: String::new(),
            scale: 1.0,
            pan: [0.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            flip_horizontal: false,
            texture_filter: TextureFilterChoice::Nearest,
            texture_lighting: true,
            color_brightness: 1.0,
            backface_culling: true,
            light_direction: [0.55, 0.35, 1.0],
            diffuse_light: 0.65,
            ambient_light: 0.35,
            animation_index: 0,
            animation_speed: 1.0,
            animation_loop_blend_seconds: 0.35,
        }
    }
}

impl Default for SystemTemplate {
    fn default() -> Self {
        Self {
            enabled: true,
            width_percent: 70,
            horizontal_position: HorizontalPosition::Center,
            horizontal_alignment: HorizontalAlignment::Center,
            vertical_alignment: VerticalAlignment::Center,
            sections: Vec::new(),
        }
    }
}

impl WidgetTemplate {
    pub fn load() -> Result<Self, String> {
        let path = template_path()?;
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("unable to read template {}: {error}", path.display()))?;
        let mut template: Self = serde_json::from_str(&contents)
            .map_err(|error| format!("invalid template {}: {error}", path.display()))?;
        template.normalize();
        Ok(template)
    }

    pub fn model_path(&self) -> Option<PathBuf> {
        if !self.model.enabled {
            return None;
        }

        if let Some(path) = env::var_os("DESKTOP_TUI_MODEL_PATH").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path));
        }
        if self.model.asset.is_empty() {
            return None;
        }

        let path = PathBuf::from(&self.model.asset);
        if path.is_absolute() {
            Some(path)
        } else {
            Some(asset_directory().join(path))
        }
    }

    pub fn includes_section(&self, section: &str) -> bool {
        self.system.sections.is_empty()
            || self
                .system
                .sections
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(section))
    }

    fn normalize(&mut self) {
        self.outer_margin = self.outer_margin.min(20);
        self.system.width_percent = self.system.width_percent.clamp(10, 100);
        self.model.scale = finite_or(self.model.scale, 1.0).clamp(0.05, 100.0);
        self.model.pan = self.model.pan.map(|value| finite_or(value, 0.0));
        self.model.rotation_degrees = self
            .model
            .rotation_degrees
            .map(|value| finite_or(value, 0.0));
        self.model.color_brightness = finite_or(self.model.color_brightness, 1.0).clamp(0.0, 4.0);
        self.model.light_direction = self
            .model
            .light_direction
            .map(|value| finite_or(value, 0.0));
        self.model.diffuse_light = finite_or(self.model.diffuse_light, 0.65).clamp(0.0, 1.0);
        self.model.ambient_light = finite_or(self.model.ambient_light, 0.35).clamp(0.0, 1.0);
        self.model.animation_speed = finite_or(self.model.animation_speed, 1.0).clamp(0.0, 10.0);
        self.model.animation_loop_blend_seconds =
            finite_or(self.model.animation_loop_blend_seconds, 0.35).clamp(0.0, 10.0);
        self.system
            .sections
            .retain(|section| !section.trim().is_empty());
    }
}

fn template_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("DESKTOP_TUI_TEMPLATE_FILE").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    let id = env::var("DESKTOP_TUI_TEMPLATE")
        .ok()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| DEFAULT_TEMPLATE_ID.into());
    let file_name = Path::new(&id)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == id)
        .ok_or_else(|| format!("invalid template id {id:?}"))?;

    Ok(template_directory().join(format!("{file_name}.json")))
}

fn template_directory() -> PathBuf {
    env::var_os("DESKTOP_TUI_TEMPLATE_DIR")
        .map(PathBuf::from)
        .or_else(|| executable_sibling("templates"))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates"))
}

fn asset_directory() -> PathBuf {
    env::var_os("DESKTOP_TUI_ASSET_DIR")
        .map(PathBuf::from)
        .or_else(|| executable_sibling("assets"))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn executable_sibling(name: &str) -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .parent()
        .map(|directory| directory.join(name))
        .filter(|directory| directory.is_dir())
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_defaults_are_safe_without_a_model() {
        let template: WidgetTemplate = serde_json::from_str("{}").expect("default template");
        assert!(!template.model.enabled);
        assert!(template.system.enabled);
        assert_eq!(template.system.width_percent, 70);
    }

    #[test]
    fn template_normalization_bounds_user_values() {
        let mut template: WidgetTemplate = serde_json::from_str(
            r#"{
                "outer_margin": 99,
                "model": {"scale": -2.0, "ambient_light": 3.0},
                "system": {"width_percent": 4}
            }"#,
        )
        .expect("template");

        template.normalize();

        assert_eq!(template.outer_margin, 20);
        assert_eq!(template.model.scale, 0.05);
        assert_eq!(template.model.ambient_light, 1.0);
        assert_eq!(template.system.width_percent, 10);
    }

    #[test]
    fn section_filter_is_case_insensitive() {
        let mut template = WidgetTemplate::default();
        template.system.sections = vec!["hardware".into()];
        assert!(template.includes_section("HARDWARE"));
        assert!(!template.includes_section("SYSTEM"));
    }
}
