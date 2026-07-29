mod ansi;
mod backend;
mod command_canvas;
mod template;

use std::{
    collections::HashMap,
    env, fs,
    fs::OpenOptions,
    io::{self, LineWriter, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    backend::Backend,
    crossterm::{
        execute,
        terminal::{enable_raw_mode, EnterAlternateScreen},
    },
    layout::{Alignment, Constraint, Layout, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use ratatui_3dmesh::{
    render::PreparedMesh, ColorMode, Mesh, Mesh3dConfig, Mesh3dState, Mesh3dWidget, ProjectionMode,
    TextureFilter, Vec3,
};

use ansi::AnsiOptimizer;
use backend::{GroupedCrosstermBackend, SharedFrameBackend};
use template::{
    HorizontalAlignment, HorizontalPosition, ModelTemplate, SystemTemplate, TextureFilterChoice,
    VerticalAlignment, WidgetTemplate,
};

const DEFAULT_FPS: f32 = 15.0;
const MAX_FPS: f32 = 60.0;
const DEFAULT_MAX_TEXTURE_DIMENSION: u32 = 512;
const FPS_SAMPLE_TIME: Duration = Duration::from_millis(500);
const THEME_REFRESH: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
struct Theme {
    text: Color,
    accent: Color,
    accent_muted: Color,
    accent_dim: Color,
    model: Color,
}

#[derive(Debug, Default)]
struct IniDocument {
    values: HashMap<(String, String), String>,
}

#[derive(Debug)]
struct SystemSnapshot {
    title: String,
    groups: Vec<InfoGroup>,
}

#[derive(Debug)]
struct InfoGroup {
    title: &'static str,
    rows: Vec<(String, String)>,
}

#[derive(Debug)]
struct RuntimeSettings {
    template: WidgetTemplate,
    frames_per_second: f32,
    show_fps: bool,
    animate_model: bool,
}

struct RenderContext<'frame, 'mesh> {
    model: Option<&'frame PreparedMesh<'mesh>>,
    config: &'frame Mesh3dConfig,
    state: Option<&'frame mut Mesh3dState>,
    snapshot: Option<&'frame SystemSnapshot>,
    theme: &'frame Theme,
    fps: Option<f32>,
    settings: &'frame RuntimeSettings,
}

#[derive(Debug, Default)]
struct FpsCounter {
    window_started: Option<Instant>,
    frames: u32,
    displayed: Option<f32>,
}

impl FpsCounter {
    fn record_frame(&mut self, now: Instant) {
        let Some(started) = self.window_started else {
            self.window_started = Some(now);
            return;
        };
        self.frames += 1;
        let elapsed = now.duration_since(started);
        if elapsed >= FPS_SAMPLE_TIME {
            self.displayed = Some(self.frames as f32 / elapsed.as_secs_f32());
            self.frames = 0;
            self.window_started = Some(now);
        }
    }

    fn displayed(&self) -> Option<f32> {
        self.displayed
    }
}

impl RuntimeSettings {
    fn load() -> Result<Self, io::Error> {
        let template = WidgetTemplate::load()
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        let frames_per_second = env::var_os("DESKTOP_TUI_FPS")
            .and_then(|value| value.into_string().ok())
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|fps| fps.is_finite() && *fps >= 1.0)
            .unwrap_or(DEFAULT_FPS)
            .min(MAX_FPS);
        let show_fps = env::var_os("DESKTOP_TUI_SHOW_FPS")
            .and_then(|value| value.into_string().ok())
            .is_some_and(|value| parse_bool(&value));
        let animate_model = env::var("DESKTOP_TUI_ANIMATE_MODEL")
            .ok()
            .is_none_or(|value| parse_bool(&value));

        Ok(Self {
            template,
            frames_per_second,
            show_fps,
            animate_model,
        })
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if command_canvas::requested() {
        return command_canvas::run();
    }

    env::remove_var("NO_COLOR");
    env::set_var("TERM", "xterm-256color");
    env::set_var("COLORTERM", "truecolor");

    let settings = RuntimeSettings::load()?;
    let model = load_model(&settings.template)?;
    let snapshot = settings
        .template
        .system
        .enabled
        .then(SystemSnapshot::collect);

    if let Some(path) = env::var_os("DESKTOP_TUI_SHARED_FRAME") {
        let size = shared_frame_size();
        let backend = SharedFrameBackend::new(Path::new(&path), size)?;
        let mut terminal = Terminal::new(backend)?;
        terminal.hide_cursor()?;
        return run(&mut terminal, model, snapshot.as_ref(), &settings);
    }

    let mut output = io::stdout();
    enable_raw_mode()?;
    execute!(output, EnterAlternateScreen)?;
    let backend = GroupedCrosstermBackend::new(AnsiOptimizer::new(output));
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run(&mut terminal, model, snapshot.as_ref(), &settings);
    ratatui::restore();
    result
}

fn shared_frame_size() -> Size {
    fn dimension(name: &str, default: u16) -> u16 {
        env::var_os(name)
            .and_then(|value| value.into_string().ok())
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
    }

    Size::new(
        dimension("DESKTOP_TUI_FRAME_WIDTH", 160),
        dimension("DESKTOP_TUI_FRAME_HEIGHT", 48),
    )
}

fn load_model(template: &WidgetTemplate) -> Result<Option<Mesh>, Box<dyn std::error::Error>> {
    let Some(path) = template.model_path() else {
        return Ok(None);
    };
    let mut mesh = Mesh::load(path)?;
    mesh.limit_texture_size(max_texture_dimension());
    Ok(Some(mesh))
}

fn run<B: Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    mut model: Option<Mesh>,
    snapshot: Option<&SystemSnapshot>,
    settings: &RuntimeSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = model
        .as_ref()
        .map(|mesh| model_state(mesh, &settings.template.model, settings.animate_model));
    let prepared = model.as_mut().map(PreparedMesh::new_compact);

    let mut theme = Theme::load();
    let mut config = mesh_config(&theme, &settings.template.model);
    let active_fps = if prepared.is_some() && !settings.animate_model {
        1.0
    } else {
        settings.frames_per_second
    };
    let frame_time = frame_time(active_fps);
    let mut last_frame = Instant::now();
    let mut last_theme_refresh = Instant::now();
    let mut fps_counter = FpsCounter::default();
    let mut frame_log = frame_timing_log();
    let mut frame_index = 0_u64;

    loop {
        let frame_started = Instant::now();
        let now = Instant::now();
        let frame_gap = now.duration_since(last_frame);
        if let Some(state) = &mut state {
            state.tick(frame_gap.as_secs_f32(), &config);
        }
        last_frame = now;

        let theme_started = Instant::now();
        if now.duration_since(last_theme_refresh) >= THEME_REFRESH {
            let refreshed = Theme::load();
            if refreshed != theme {
                theme = refreshed;
                config = mesh_config(&theme, &settings.template.model);
                terminal.clear()?;
            }
            last_theme_refresh = now;
        }
        let theme_elapsed = theme_started.elapsed();

        let displayed_fps = fps_counter.displayed();
        let draw_started = Instant::now();
        terminal.draw(|frame| {
            render(
                frame,
                RenderContext {
                    model: prepared.as_ref(),
                    config: &config,
                    state: state.as_mut(),
                    snapshot,
                    theme: &theme,
                    fps: displayed_fps,
                    settings,
                },
            );
            stabilize_terminal_colors(frame.buffer_mut());
        })?;
        let draw_finished = Instant::now();
        let draw_elapsed = draw_finished.duration_since(draw_started);
        fps_counter.record_frame(draw_finished);

        let elapsed = last_frame.elapsed();
        let sleep_requested = frame_time.saturating_sub(elapsed);
        if let Some(log) = &mut frame_log {
            let _ = writeln!(
                log,
                "{frame_index},{:.3},{:.3},{:.3},{:.3},{:.4},{:.1}",
                millis(frame_gap),
                millis(theme_elapsed),
                millis(draw_elapsed),
                millis(frame_started.elapsed()),
                state
                    .as_ref()
                    .map_or(0.0, |state| state.animation_time_seconds),
                fps_counter.displayed().unwrap_or(0.0),
            );
        }
        frame_index += 1;
        if !sleep_requested.is_zero() {
            thread::sleep(sleep_requested);
        }
    }
}

fn frame_timing_log() -> Option<LineWriter<fs::File>> {
    env::var_os("DESKTOP_TUI_FRAME_LOG")?;
    let runtime_directory = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    let path = runtime_directory.join("desktop-tui-frame-timing.csv");
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .ok()?;
    let mut writer = LineWriter::new(file);
    writeln!(
        writer,
        "frame,gap_ms,theme_ms,draw_ms,work_ms,animation_seconds,fps"
    )
    .ok()?;
    Some(writer)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn stabilize_terminal_colors(buffer: &mut ratatui::buffer::Buffer) {
    for cell in &mut buffer.content {
        if let Color::Rgb(red, green, blue) = cell.fg {
            cell.fg = Color::Rgb(
                stabilize_color_channel(red),
                stabilize_color_channel(green),
                stabilize_color_channel(blue),
            );
        }
    }
}

const fn stabilize_color_channel(channel: u8) -> u8 {
    let rounded = ((channel as u16 + 4) / 8) * 8;
    if rounded > u8::MAX as u16 {
        u8::MAX
    } else {
        rounded as u8
    }
}

fn render(frame: &mut Frame, context: RenderContext<'_, '_>) {
    let margin = context
        .settings
        .template
        .outer_margin
        .min(frame.area().height.saturating_sub(1) / 2);
    let [_, content, _] = Layout::vertical([
        Constraint::Length(margin),
        Constraint::Min(1),
        Constraint::Length(margin),
    ])
    .areas(frame.area());

    if let (Some(model), Some(state)) = (context.model, context.state) {
        render_mesh(frame, content, model, context.config, state);
    }

    if let Some(snapshot) = context.snapshot {
        let area = system_area(content, &context.settings.template.system);
        render_identity(
            frame,
            area,
            snapshot,
            context.theme,
            &context.settings.template.system,
            &context.settings.template,
        );
    }
    if context.settings.show_fps {
        render_fps(frame, context.fps, context.theme);
    }
}

fn render_fps(frame: &mut Frame, fps: Option<f32>, theme: &Theme) {
    let label = fps.map_or_else(|| "FPS  --.-".to_owned(), |fps| format!("FPS {fps:5.1}"));
    let width = label.len() as u16;
    let frame_area = frame.area();
    if frame_area.width < width {
        return;
    }
    let area = Rect::new(
        frame_area.x + frame_area.width - width,
        frame_area.y,
        width,
        1,
    );
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.accent_dim)),
        area,
    );
}

fn render_mesh(
    frame: &mut Frame,
    area: Rect,
    mesh: &PreparedMesh<'_>,
    config: &Mesh3dConfig,
    state: &mut Mesh3dState,
) {
    let [_, stage, _] = Layout::horizontal([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    frame.render_stateful_widget(
        Mesh3dWidget::new_prepared(mesh).with_config_ref(config),
        stage,
        state,
    );
}

fn system_area(area: Rect, config: &SystemTemplate) -> Rect {
    let width = (u32::from(area.width) * u32::from(config.width_percent) / 100)
        .max(1)
        .min(u32::from(area.width)) as u16;
    let x = match config.horizontal_position {
        HorizontalPosition::Left => area.x,
        HorizontalPosition::Center => area.x + area.width.saturating_sub(width) / 2,
        HorizontalPosition::Right => area.x + area.width.saturating_sub(width),
    };
    Rect::new(x, area.y, width, area.height)
}

fn render_identity(
    frame: &mut Frame,
    area: Rect,
    snapshot: &SystemSnapshot,
    theme: &Theme,
    config: &SystemTemplate,
    template: &WidgetTemplate,
) {
    if area.width < 20 || area.height < 8 {
        return;
    }

    let mut lines = vec![
        Line::styled(
            &snapshot.title,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];

    for (rendered_groups, group) in snapshot
        .groups
        .iter()
        .filter(|group| !group.rows.is_empty() && template.includes_section(group.title))
        .enumerate()
    {
        let group_height = group.rows.len() + 1 + usize::from(rendered_groups > 0);
        if lines.len() + group_height > area.height as usize {
            break;
        }

        if rendered_groups > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(
            group.title,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        for (label, value) in &group.rows {
            lines.push(info_line(label, value, theme));
        }
    }

    let content_height = (lines.len() as u16).min(area.height);
    let y = match config.vertical_alignment {
        VerticalAlignment::Top => area.y,
        VerticalAlignment::Center => area.y + area.height.saturating_sub(content_height) / 2,
        VerticalAlignment::Bottom => area.y + area.height.saturating_sub(content_height),
    };
    let centered_area = Rect {
        x: area.x,
        y,
        width: area.width,
        height: content_height,
    };
    frame.render_widget(
        Paragraph::new(lines).alignment(match config.horizontal_alignment {
            HorizontalAlignment::Left => Alignment::Left,
            HorizontalAlignment::Center => Alignment::Center,
            HorizontalAlignment::Right => Alignment::Right,
        }),
        centered_area,
    );
}

fn info_line<'a>(label: &'a str, value: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(label, Style::default().fg(theme.accent_muted)),
        Span::styled("  ·  ", Style::default().fg(theme.accent_dim)),
        Span::styled(value, Style::default().fg(theme.text)),
    ])
}

fn mesh_config(theme: &Theme, model: &ModelTemplate) -> Mesh3dConfig {
    Mesh3dConfig::quality()
        .auto_fit(true)
        .scale(model.scale)
        .show_hints(false)
        .show_help_overlay(false)
        .projection(ProjectionMode::Orthographic)
        .color_mode(ColorMode::Texture)
        .texture_filter(match model.texture_filter {
            TextureFilterChoice::Nearest => TextureFilter::Nearest,
            TextureFilterChoice::Bilinear => TextureFilter::Bilinear,
        })
        .texture_lighting(model.texture_lighting)
        .color_brightness(model.color_brightness)
        .backface_culling(model.backface_culling)
        .flip_horizontal(model.flip_horizontal)
        .cell_aspect_ratio(0.5)
        .light_direction(model.light_direction)
        .lighting(model.diffuse_light, model.ambient_light)
        .foreground_style(Style::default().fg(theme.model))
        .background_style(None)
}

fn frame_time(fps: f32) -> Duration {
    Duration::from_secs_f32(fps.recip())
}

fn max_texture_dimension() -> u32 {
    env::var_os("DESKTOP_TUI_MAX_TEXTURE_DIMENSION")
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_MAX_TEXTURE_DIMENSION)
}

fn model_state(mesh: &Mesh, config: &ModelTemplate, animate: bool) -> Mesh3dState {
    let animation = (!mesh.animations.is_empty())
        .then_some(config.animation_index.min(mesh.animations.len() - 1));
    let radians = config.rotation_degrees.map(f32::to_radians);

    Mesh3dState {
        rotation: Vec3::new(radians[0], radians[1], radians[2]),
        pan: Vec3::new(config.pan[0], config.pan[1], config.pan[2]),
        zoom: 1.0,
        auto_spin_enabled: false,
        selected_animation: animation,
        animation_speed: config.animation_speed,
        animation_playing: animate && animation.is_some(),
        animation_looping: true,
        animation_loop_blend_seconds: config.animation_loop_blend_seconds,
        ..Mesh3dState::default()
    }
}

impl Theme {
    fn load() -> Self {
        Self::from_active_kde_scheme().unwrap_or_else(Self::fallback)
    }

    fn from_active_kde_scheme() -> Option<Self> {
        let scheme_name = active_color_scheme()?;
        let scheme_path = color_scheme_path(&scheme_name)?;
        let scheme = IniDocument::load(&scheme_path)?;

        let text = scheme
            .color("Colors:View", "ForegroundNormal")
            .unwrap_or(Color::Rgb(230, 228, 240));
        let text_muted = scheme
            .color("Colors:Window", "ForegroundNormal")
            .or_else(|| scheme.color("Colors:View", "ForegroundInactive"))
            .unwrap_or(Color::Rgb(171, 170, 181));
        let accent = scheme
            .color("Colors:Selection", "BackgroundNormal")
            .or_else(|| scheme.color("Colors:View", "DecorationFocus"))
            .unwrap_or(Color::Rgb(192, 196, 238));
        let accent_muted = scheme
            .color("Colors:Complementary", "ForegroundNormal")
            .unwrap_or(text_muted);
        let accent_dim = scheme
            .color("Colors:View", "DecorationHover")
            .unwrap_or(Color::Rgb(88, 92, 130));
        let model = accent;

        Some(Self {
            text,
            accent,
            accent_muted,
            accent_dim,
            model,
        })
    }

    fn fallback() -> Self {
        Self {
            text: Color::Rgb(222, 216, 211),
            accent: Color::Rgb(181, 34, 62),
            accent_muted: Color::Rgb(125, 40, 58),
            accent_dim: Color::Rgb(80, 24, 38),
            model: Color::Rgb(205, 155, 160),
        }
    }
}

impl IniDocument {
    fn load(path: &Path) -> Option<Self> {
        let contents = fs::read_to_string(path).ok()?;
        let mut document = Self::default();
        let mut section = String::new();

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                section = name.to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            document.values.insert(
                (section.clone(), key.trim().to_owned()),
                value.trim().to_owned(),
            );
        }

        Some(document)
    }

    fn value(&self, section: &str, key: &str) -> Option<&str> {
        self.values
            .get(&(section.to_owned(), key.to_owned()))
            .map(String::as_str)
    }

    fn color(&self, section: &str, key: &str) -> Option<Color> {
        parse_color(self.value(section, key)?)
    }
}

fn active_color_scheme() -> Option<String> {
    let path = config_home()?.join("kdeglobals");
    IniDocument::load(&path)?
        .value("General", "ColorScheme")
        .map(str::to_owned)
}

fn color_scheme_path(scheme_name: &str) -> Option<PathBuf> {
    let safe_name = Path::new(scheme_name).file_name()?.to_str()?;
    let file_name = if safe_name.ends_with(".colors") {
        safe_name.to_owned()
    } else {
        format!("{safe_name}.colors")
    };

    [
        data_home()?.join("color-schemes"),
        PathBuf::from("/usr/share/color-schemes"),
    ]
    .into_iter()
    .map(|directory| directory.join(&file_name))
    .find(|path| path.is_file())
}

fn parse_color(value: &str) -> Option<Color> {
    if let Some(hex) = value.strip_prefix('#') {
        let rgb = match hex.len() {
            6 => hex,
            8 => &hex[2..],
            _ => return None,
        };
        return Some(Color::Rgb(
            u8::from_str_radix(&rgb[0..2], 16).ok()?,
            u8::from_str_radix(&rgb[2..4], 16).ok()?,
            u8::from_str_radix(&rgb[4..6], 16).ok()?,
        ));
    }

    let channels = value
        .split(',')
        .map(str::trim)
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (channels.len() == 3).then(|| Color::Rgb(channels[0], channels[1], channels[2]))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir()?.join(".config")))
}

fn data_home() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(home_dir()?.join(".local/share")))
}

impl SystemSnapshot {
    fn collect() -> Self {
        let title = format!("{}@{}", username(), hostname());
        let rows = fastfetch_rows();

        let mut system = Vec::new();
        let mut desktop = Vec::new();
        let mut hardware = Vec::new();
        let mut storage = Vec::new();
        let mut network = Vec::new();

        for (label, value) in rows {
            match label.as_str() {
                "OS" | "Host" | "Kernel" | "Uptime" | "Packages" | "Shell" => {
                    system.push((label, value));
                }
                "DE" | "WM" => {
                    desktop.push((friendly_label(&label), value));
                }
                label if label.starts_with("Display") => {
                    desktop.push((friendly_label(label), value));
                }
                "CPU" | "GPU" | "Memory" => hardware.push((label, value)),
                label if label.starts_with("Disk") && storage.len() < 2 => {
                    storage.push((friendly_label(label), value));
                }
                label if label.starts_with("Local IP") => {
                    network.push((friendly_label(label), value));
                }
                _ => {}
            }
        }

        if system.is_empty() {
            system.push(("Status".into(), "system information unavailable".into()));
        }

        Self {
            title,
            groups: vec![
                InfoGroup {
                    title: "SYSTEM",
                    rows: system,
                },
                InfoGroup {
                    title: "DESKTOP",
                    rows: desktop,
                },
                InfoGroup {
                    title: "HARDWARE",
                    rows: hardware,
                },
                InfoGroup {
                    title: "STORAGE",
                    rows: storage,
                },
                InfoGroup {
                    title: "NETWORK",
                    rows: network,
                },
            ],
        }
    }
}

fn fastfetch_rows() -> Vec<(String, String)> {
    const STRUCTURE: &str =
        "OS:Host:Kernel:Uptime:Packages:Shell:Display:DE:WM:CPU:GPU:Memory:Disk:LocalIp";

    let output = Command::new("fastfetch")
        .args(["--logo", "none", "--pipe", "--structure", STRUCTURE])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once(": "))
        .map(|(label, value)| {
            let value = if label == "Shell" {
                shell_name().unwrap_or_else(|| value.to_owned())
            } else {
                value.to_owned()
            };
            (label.to_owned(), value)
        })
        .collect()
}

fn friendly_label(label: &str) -> String {
    if label.starts_with("Display") {
        "Display".into()
    } else if label.starts_with("Disk") {
        label
            .split_once('(')
            .and_then(|(_, path)| path.strip_suffix(')'))
            .map_or_else(|| "Disk".into(), |path| format!("Disk {path}"))
    } else if label.starts_with("Local IP") {
        "Local IP".into()
    } else {
        match label {
            "DE" => "Desktop".into(),
            "WM" => "Session".into(),
            other => other.into(),
        }
    }
}

fn username() -> String {
    env::var("DESKTOP_TUI_USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "user".into())
}

fn hostname() -> String {
    env::var("DESKTOP_TUI_HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| command_text("hostname"))
        .unwrap_or_else(|| "localhost".into())
}

fn shell_name() -> Option<String> {
    env::var("SHELL")
        .ok()
        .and_then(|path| PathBuf::from(path).file_name()?.to_str().map(str::to_owned))
}

fn command_text(program: &str) -> Option<String> {
    let output = Command::new(program).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_counter_reports_completed_frame_intervals() {
        let start = Instant::now();
        let mut counter = FpsCounter::default();

        counter.record_frame(start);
        for interval in 1_u64..=10 {
            counter.record_frame(start + Duration::from_millis(interval * 50));
        }

        let fps = counter.displayed().expect("an FPS sample");
        assert!((fps - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn terminal_color_stabilization_has_small_bounded_error() {
        for channel in u8::MIN..=u8::MAX {
            assert!(channel.abs_diff(stabilize_color_channel(channel)) <= 4);
        }
        assert_eq!(stabilize_color_channel(0), 0);
        assert_eq!(stabilize_color_channel(255), 255);
    }
}
