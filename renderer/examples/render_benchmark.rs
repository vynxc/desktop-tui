use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::hash_map::DefaultHasher,
    env,
    hash::{Hash, Hasher},
    path::Path,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};
use ratatui_3dmesh::{
    render::render_prepared_mesh,
    render::{render_prepared_mesh_profiled, PreparedMesh, RenderProfile},
    ColorMode, Mesh, Mesh3dConfig, Mesh3dState, Mesh3dWidget, ProjectionMode, TextureFilter, Vec3,
};

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES_ALLOCATED: AtomicU64 = AtomicU64::new(0);
static BYTES_DEALLOCATED: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_SIZE_BUCKETS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES_ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
            allocation_bucket(layout.size()).fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES_DEALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, old, new_size) };
        if !new_ptr.is_null() {
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES_DEALLOCATED.fetch_add(old.size() as u64, Ordering::Relaxed);
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES_ALLOCATED.fetch_add(new_size as u64, Ordering::Relaxed);
            allocation_bucket(new_size).fetch_add(1, Ordering::Relaxed);
        }
        new_ptr
    }
}

#[derive(Clone, Copy)]
struct AllocationSnapshot {
    allocations: u64,
    deallocations: u64,
    bytes_allocated: u64,
    bytes_deallocated: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let asset = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("renderer/assets/fox.glb"));
    let width = parse_arg(2, 180);
    let height = parse_arg(3, 52);
    let frames = parse_arg(4, 120);

    eprintln!("cpu: {}", cpu_model());
    eprintln!("logical cpu affinity: {}", cpu_affinity());

    reset_allocations();
    let load_cpu = process_cpu_time();
    let load_cycles = read_cycles();
    let load_started = Instant::now();
    let mut mesh = Mesh::load(&asset)?;
    let load_elapsed = load_started.elapsed();
    let load_cycles = read_cycles().saturating_sub(load_cycles);
    let load_cpu = process_cpu_time().saturating_sub(load_cpu);
    let load_allocations = allocation_snapshot();
    eprintln!(
        "load: {:.2}ms wall, {:.2}ms cpu, {load_cycles} reference cycles, {} allocations ({:.1} MiB)",
        millis(load_elapsed),
        millis(load_cpu),
        load_allocations.allocations,
        mib(load_allocations.bytes_allocated),
    );

    if let Some(limit) = env::var("DESKTOP_TUI_BENCH_TEXTURE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    {
        reset_allocations();
        let resize_started = Instant::now();
        let resized = mesh.limit_texture_size(limit);
        let resize_elapsed = resize_started.elapsed();
        let resize_allocations = allocation_snapshot();
        eprintln!(
            "texture resize: {resized} textures within {limit}px, {:.2}ms, {} allocations ({:.1} MiB)",
            millis(resize_elapsed),
            resize_allocations.allocations,
            mib(resize_allocations.bytes_allocated),
        );
    }
    if env_flag("DESKTOP_TUI_BENCH_FLAT_NORMALS") {
        mesh.normals.clear();
        mesh.bind_normals.clear();
        for node in &mut mesh.animation_nodes {
            node.normal_ranges.clear();
        }
        for skin in &mut mesh.skins {
            skin.normal_range = None;
        }
        for face in &mut mesh.faces {
            face.normal_indices.clear();
            face.normal = None;
        }
        eprintln!("using geometry-derived face normals");
    }
    let texture_bytes = mesh
        .textures
        .iter()
        .map(|texture| texture.rgba.len())
        .sum::<usize>();
    eprintln!(
        "mesh: {} vertices, {} normals, {} UVs, {} vertex colors, {} faces, {} materials, {} textures, {} animation nodes, {} skins, {:.1} MiB texture data",
        mesh.vertices.len(),
        mesh.normals.len(),
        mesh.tex_coords.len(),
        mesh.vertex_colors.len(),
        mesh.faces.len(),
        mesh.materials.len(),
        mesh.textures.len(),
        mesh.animation_nodes.len(),
        mesh.skins.len(),
        texture_bytes as f64 / (1024.0 * 1024.0)
    );
    let non_white_vertex_colors = mesh
        .vertex_colors
        .iter()
        .filter(|&&color| color != [1.0; 4])
        .count();
    let wrapped_uvs = mesh
        .tex_coords
        .iter()
        .filter(|uv| !(uv.u >= 0.0 && uv.u < 1.0 && uv.v >= 0.0 && uv.v < 1.0))
        .count();
    eprintln!(
        "mesh attributes: {non_white_vertex_colors} non-white vertex colors, {wrapped_uvs} UVs outside repeat fast path"
    );
    for (index, clip) in mesh.animations.iter().enumerate() {
        let keyframes = clip
            .channels
            .iter()
            .map(|channel| channel.sampler.inputs.len())
            .sum::<usize>();
        let largest_channel = clip
            .channels
            .iter()
            .map(|channel| channel.sampler.inputs.len())
            .max()
            .unwrap_or(0);
        eprintln!(
            "animation {index}: name={:?}, duration={:.3}s, channels={}, keyframes={keyframes}, largest_channel={largest_channel}",
            clip.name,
            clip.duration_seconds,
            clip.channels.len(),
        );
    }
    let animation_vertex_ranges = mesh
        .animation_nodes
        .iter()
        .flat_map(|node| &node.vertex_ranges)
        .collect::<Vec<_>>();
    eprintln!(
        "animation ownership: {} vertex ranges, largest={} vertices",
        animation_vertex_ranges.len(),
        animation_vertex_ranges
            .iter()
            .map(|range| range.len)
            .max()
            .unwrap_or(0),
    );
    for (index, material) in mesh.materials.iter().enumerate() {
        eprintln!(
            "material {index}: name={:?}, diffuse={:?}, alpha={:?}/{:.3}, double_sided={}, diffuse_texture={:?}, emissive={:?}, emissive_texture={:?}",
            material.name,
            material.diffuse,
            material.alpha_mode,
            material.base_color_alpha,
            material.double_sided,
            material.diffuse_texture.as_ref().and_then(|texture| texture.index),
            material.emissive,
            material.emissive_texture.as_ref().and_then(|texture| texture.index),
        );
    }
    for (index, texture) in mesh.textures.iter().enumerate() {
        eprintln!(
            "texture {index}: {}x{} ({:.1} MiB)",
            texture.width,
            texture.height,
            texture.rgba.len() as f64 / (1024.0 * 1024.0)
        );
    }
    reset_allocations();
    let prepare_started = Instant::now();
    let prepared = PreparedMesh::new_compact(&mut mesh);
    let prepare_elapsed = prepare_started.elapsed();
    let prepare_allocations = allocation_snapshot();
    eprintln!(
        "topology cache: {:.2}ms, {} allocations ({:.1} MiB)",
        millis(prepare_elapsed),
        prepare_allocations.allocations,
        mib(prepare_allocations.bytes_allocated),
    );
    eprintln!("target: {width}x{height}, {frames} frames");
    eprintln!("memory after load: {}", memory_status());

    let cull = !env_flag("DESKTOP_TUI_BENCH_NO_CULL");
    let nearest = env_flag("DESKTOP_TUI_BENCH_NEAREST");
    let static_mesh = env_flag("DESKTOP_TUI_BENCH_STATIC");
    let normalize = !env_flag("DESKTOP_TUI_BENCH_NO_NORMALIZE");
    let loop_blend_seconds = env::var("DESKTOP_TUI_BENCH_LOOP_BLEND")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .unwrap_or(0.0);
    let color_mode = match env::var("DESKTOP_TUI_BENCH_COLOR_MODE").ok().as_deref() {
        Some("off") => ColorMode::Off,
        Some("material") => ColorMode::Material,
        Some("lighting") => ColorMode::Lighting,
        Some("auto") => ColorMode::Auto,
        _ => ColorMode::Texture,
    };
    eprintln!(
        "options: backface_culling={cull}, nearest_filter={nearest}, \
         static_mesh={static_mesh}, normalize={normalize}, color_mode={color_mode:?}, \
         loop_blend={loop_blend_seconds:.3}s"
    );
    let config = Mesh3dConfig::quality()
        .auto_fit(true)
        .scale(1.35)
        .show_hints(false)
        .show_help_overlay(false)
        .projection(ProjectionMode::Orthographic)
        .color_mode(color_mode)
        .texture_lighting(true)
        .color_brightness(1.0)
        .backface_culling(cull)
        .texture_filter(if nearest {
            TextureFilter::Nearest
        } else {
            TextureFilter::Bilinear
        })
        .normalize(normalize)
        .flip_horizontal(false)
        .cell_aspect_ratio(0.5)
        .light_direction([0.55, 0.35, 1.0])
        .lighting(0.65, 0.35)
        .background_style(None);
    let mut state = Mesh3dState {
        rotation: Vec3::new(0.0, 25.0_f32.to_radians(), 0.0),
        pan: Vec3::new(0.0, -0.12, 0.0),
        selected_animation: (!static_mesh && !prepared.mesh().animations.is_empty()).then_some(0),
        animation_playing: !static_mesh && !prepared.mesh().animations.is_empty(),
        animation_looping: true,
        animation_loop_blend_seconds: loop_blend_seconds,
        ..Mesh3dState::default()
    };
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    for _ in 0..2 {
        state.tick(0.05, &config);
        terminal.draw(|frame| {
            frame.render_stateful_widget(
                Mesh3dWidget::new_prepared(&prepared).with_config_ref(&config),
                frame.area(),
                &mut state,
            );
        })?;
    }
    let measured_initial_state = state;

    reset_allocations();
    let cpu_started = process_cpu_time();
    let cycles_started = read_cycles();
    let started = Instant::now();
    for _ in 0..frames {
        state.tick(0.05, &config);
        terminal.draw(|frame| {
            frame.render_stateful_widget(
                Mesh3dWidget::new_prepared(&prepared).with_config_ref(&config),
                frame.area(),
                &mut state,
            );
        })?;
    }
    let elapsed = started.elapsed();
    let cycles = read_cycles().saturating_sub(cycles_started);
    let cpu_elapsed = process_cpu_time().saturating_sub(cpu_started);
    let allocations = allocation_snapshot();
    let frame_checksum = buffer_checksum(terminal.backend().buffer());
    let frame_count = u64::from(frames);
    eprintln!(
        "total: {:.3}s wall, {:.3}s cpu, {:.1}% of one core",
        elapsed.as_secs_f64(),
        cpu_elapsed.as_secs_f64(),
        100.0 * cpu_elapsed.as_secs_f64() / elapsed.as_secs_f64(),
    );
    eprintln!(
        "frame: {:.2}ms wall, {:.2}ms cpu, {:.1} frames/s",
        millis(elapsed) / f64::from(frames),
        millis(cpu_elapsed) / f64::from(frames),
        f64::from(frames) / elapsed.as_secs_f64(),
    );
    eprintln!(
        "cycles: {cycles} total, {} reference cycles/frame, {:.2} cycles/target-cell",
        cycles / frame_count.max(1),
        cycles as f64 / f64::from(frames) / (f64::from(width) * f64::from(height)),
    );
    eprintln!(
        "heap: {} allocs/frame ({:.1} KiB/frame), {} deallocs/frame ({:.1} KiB/frame)",
        allocations.allocations / frame_count.max(1),
        kib(allocations.bytes_allocated) / f64::from(frames),
        allocations.deallocations / frame_count.max(1),
        kib(allocations.bytes_deallocated) / f64::from(frames),
    );
    eprintln!("output checksum: {frame_checksum:016x}");
    if let Some(path) = env::var_os("DESKTOP_TUI_BENCH_DUMP_PPM") {
        let path = PathBuf::from(path);
        dump_buffer_ppm(terminal.backend().buffer(), &path)?;
        eprintln!("buffer image: {}", path.display());
    }
    eprintln!("memory after benchmark: {}", memory_status());

    let mut direct_state = measured_initial_state;
    let area = Rect::new(0, 0, width, height);
    let mut direct_buffer = Buffer::empty(area);
    reset_allocations();
    let direct_cpu_started = process_cpu_time();
    let direct_cycles_started = read_cycles();
    let direct_started = Instant::now();
    for _ in 0..frames {
        direct_state.tick(0.05, &config);
        direct_buffer.reset();
        render_prepared_mesh(&prepared, area, &mut direct_buffer, &direct_state, &config);
    }
    let direct_elapsed = direct_started.elapsed();
    let direct_cpu = process_cpu_time().saturating_sub(direct_cpu_started);
    let direct_cycles = read_cycles().saturating_sub(direct_cycles_started);
    let direct_allocations = allocation_snapshot();
    let direct_checksum = buffer_checksum(&direct_buffer);
    let direct_ms_per_frame = millis(direct_elapsed) / f64::from(frames);
    eprintln!(
        "direct renderer: {:.2}ms wall, {:.2}ms cpu, {:.1} frames/s, {} reference cycles/frame",
        direct_ms_per_frame,
        millis(direct_cpu) / f64::from(frames),
        f64::from(frames) / direct_elapsed.as_secs_f64(),
        direct_cycles / frame_count.max(1),
    );
    eprintln!(
        "direct heap: {} allocs/frame ({:.1} KiB/frame), checksum {direct_checksum:016x}",
        direct_allocations.allocations / frame_count.max(1),
        kib(direct_allocations.bytes_allocated) / f64::from(frames),
    );
    eprintln!(
        "Ratatui terminal overhead: {:.2}ms/frame",
        (millis(elapsed) - millis(direct_elapsed)) / f64::from(frames),
    );

    let profile_frames = env::var("DESKTOP_TUI_BENCH_PROFILE_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5)
        .max(1);
    let mut profile_buffer = Buffer::empty(area);
    let mut combined = RenderProfile::default();
    reset_allocations();
    let profile_started = Instant::now();
    for _ in 0..profile_frames {
        state.tick(0.05, &config);
        let profile =
            render_prepared_mesh_profiled(&prepared, area, &mut profile_buffer, &state, &config);
        add_profile(&mut combined, profile);
    }
    let profile_wall = profile_started.elapsed();
    let profile_allocations = allocation_snapshot();
    let profile_checksum = buffer_checksum(&profile_buffer);
    eprintln!(
        "profile: {profile_frames} frames, {:.2}ms/frame instrumented, checksum {profile_checksum:016x}",
        millis(profile_wall) / f64::from(profile_frames),
    );
    eprintln!(
        "  direct-render heap: {} allocs/frame ({:.1} KiB/frame)",
        profile_allocations.allocations / u64::from(profile_frames),
        kib(profile_allocations.bytes_allocated) / f64::from(profile_frames),
    );
    eprintln!(
        "  allocation sizes/frame: <=16={} <=32={} <=64={} <=128={} <=256={} <=512={} <=1K={} <=4K={} >4K={}",
        ALLOCATION_SIZE_BUCKETS[0].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[1].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[2].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[3].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[4].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[5].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[6].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[7].load(Ordering::Relaxed) / u64::from(profile_frames),
        ALLOCATION_SIZE_BUCKETS[8].load(Ordering::Relaxed) / u64::from(profile_frames),
    );
    print_profile(&combined, profile_frames, width, height);
    if profile_checksum != frame_checksum {
        eprintln!(
            "checksum note: profile uses later animation timestamps; use DESKTOP_TUI_BENCH_STATIC=1 for exact checksum comparison"
        );
    }
    if let Some(trace_frames) = env::var("DESKTOP_TUI_BENCH_TRACE_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|frames| *frames > 0)
    {
        let step_seconds = env::var("DESKTOP_TUI_BENCH_STEP_SECONDS")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|step| step.is_finite() && *step > 0.0)
            .unwrap_or(0.05);
        let duration = state
            .selected_animation
            .and_then(|index| prepared.mesh().animations.get(index))
            .map(|clip| clip.duration_seconds)
            .unwrap_or(0.0);
        let loop_duration = duration - loop_blend_seconds.min(duration * 0.5).max(0.0);
        let mut trace_state = measured_initial_state;
        let mut trace_buffer = Buffer::empty(area);
        let mut previous_trace_buffer = trace_buffer.clone();
        eprintln!(
            "frame trace: {trace_frames} frames, step={step_seconds:.4}s, animation_duration={duration:.4}s, loop_duration={loop_duration:.4}s"
        );
        for frame_index in 0..trace_frames {
            trace_state.tick(step_seconds, &config);
            let wall_started = Instant::now();
            let profile = render_prepared_mesh_profiled(
                &prepared,
                area,
                &mut trace_buffer,
                &trace_state,
                &config,
            );
            let wall = wall_started.elapsed();
            let changed_cells = trace_buffer
                .content()
                .iter()
                .zip(previous_trace_buffer.content())
                .filter(|(current, previous)| current != previous)
                .count();
            previous_trace_buffer.clone_from(&trace_buffer);
            let playback = if loop_duration > f32::EPSILON {
                trace_state.animation_time_seconds.rem_euclid(loop_duration)
            } else {
                trace_state.animation_time_seconds
            };
            eprintln!(
                "trace frame={frame_index:04} playback={playback:.4}s wall={:.3}ms total={:.3}ms animation={:.3}ms projection={:.3}ms depth={:.3}ms raster={:.3}ms cells_written={} changed_cells={changed_cells}",
                millis(wall),
                millis(profile.total),
                millis(profile.animation),
                millis(profile.projection),
                millis(profile.depth_buffer),
                millis(profile.face_rendering),
                profile.cells_written,
            );
        }
    }
    if env_flag("DESKTOP_TUI_BENCH_REQUIRE_BUDGET") && millis(elapsed) / f64::from(frames) > 8.0 {
        return Err("8ms/frame performance budget exceeded".into());
    }
    if env_flag("DESKTOP_TUI_BENCH_REQUIRE_RENDER_BUDGET") && direct_ms_per_frame > 8.0 {
        return Err("8ms/frame direct-renderer performance budget exceeded".into());
    }
    if env_flag("DESKTOP_TUI_BENCH_REQUIRE_CHECKSUM") && profile_checksum != frame_checksum {
        return Err("profiled and normal render checksums differ".into());
    }
    eprintln!(
        "budget: total {:.2}ms/frame {}, renderer {:.2}ms/frame {} 8.00ms target",
        millis(elapsed) / f64::from(frames),
        if millis(elapsed) / f64::from(frames) <= 8.0 {
            "meets"
        } else {
            "exceeds"
        },
        direct_ms_per_frame,
        if direct_ms_per_frame <= 8.0 {
            "meets"
        } else {
            "exceeds"
        },
    );
    std::thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn parse_arg(index: usize, fallback: u16) -> u16 {
    env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_flag(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| value != "0" && value != "false")
}

fn reset_allocations() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES_ALLOCATED.store(0, Ordering::Relaxed);
    BYTES_DEALLOCATED.store(0, Ordering::Relaxed);
    for bucket in &ALLOCATION_SIZE_BUCKETS {
        bucket.store(0, Ordering::Relaxed);
    }
}

fn allocation_snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
        bytes_allocated: BYTES_ALLOCATED.load(Ordering::Relaxed),
        bytes_deallocated: BYTES_DEALLOCATED.load(Ordering::Relaxed),
    }
}

fn allocation_bucket(size: usize) -> &'static AtomicU64 {
    let index = match size {
        0..=16 => 0,
        17..=32 => 1,
        33..=64 => 2,
        65..=128 => 3,
        129..=256 => 4,
        257..=512 => 5,
        513..=1024 => 6,
        1025..=4096 => 7,
        _ => 8,
    };
    &ALLOCATION_SIZE_BUCKETS[index]
}

fn process_cpu_time() -> Duration {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct Timespec {
            seconds: i64,
            nanoseconds: i64,
        }
        unsafe extern "C" {
            fn clock_gettime(clock_id: i32, timespec: *mut Timespec) -> i32;
        }
        const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
        let mut timespec = Timespec {
            seconds: 0,
            nanoseconds: 0,
        };
        if unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut timespec) } == 0 {
            return Duration::new(timespec.seconds.max(0) as u64, timespec.nanoseconds as u32);
        }
    }
    Duration::ZERO
}

#[cfg(target_arch = "x86_64")]
fn read_cycles() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        let cycles = core::arch::x86_64::_rdtsc();
        core::arch::x86_64::_mm_lfence();
        cycles
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn read_cycles() -> u64 {
    0
}

fn buffer_checksum(buffer: &Buffer) -> u64 {
    let mut hasher = DefaultHasher::new();
    buffer.area().hash(&mut hasher);
    for cell in buffer.content() {
        cell.symbol().hash(&mut hasher);
        cell.fg.hash(&mut hasher);
        cell.bg.hash(&mut hasher);
        cell.modifier.hash(&mut hasher);
        cell.diff_option.hash(&mut hasher);
    }
    hasher.finish()
}

fn dump_buffer_ppm(buffer: &Buffer, path: &Path) -> std::io::Result<()> {
    const CELL_HEIGHT: usize = 2;
    let width = usize::from(buffer.area().width);
    let height = usize::from(buffer.area().height);
    let mut ppm = format!("P6\n{width} {}\n255\n", height * CELL_HEIGHT).into_bytes();
    ppm.reserve(width * height * CELL_HEIGHT * 3);
    for y in 0..height {
        for _ in 0..CELL_HEIGHT {
            for x in 0..width {
                let cell = &buffer[(buffer.area().x + x as u16, buffer.area().y + y as u16)];
                let rgb = if cell.symbol().trim().is_empty() {
                    color_rgb(cell.bg).unwrap_or([0, 0, 0])
                } else {
                    color_rgb(cell.fg).unwrap_or([255, 255, 255])
                };
                ppm.extend_from_slice(&rgb);
            }
        }
    }
    std::fs::write(path, ppm)
}

fn color_rgb(color: ratatui::style::Color) -> Option<[u8; 3]> {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => Some([r, g, b]),
        ratatui::style::Color::Black => Some([0, 0, 0]),
        ratatui::style::Color::White => Some([255, 255, 255]),
        _ => None,
    }
}

fn add_profile(total: &mut RenderProfile, frame: RenderProfile) {
    total.total += frame.total;
    total.animation += frame.animation;
    total.projection += frame.projection;
    total.depth_buffer += frame.depth_buffer;
    total.face_rendering += frame.face_rendering;
    total.faces_considered += frame.faces_considered;
    total.blend_faces += frame.blend_faces;
    total.triangles_considered += frame.triangles_considered;
    total.triangles_culled += frame.triangles_culled;
    total.triangles_degenerate_or_offscreen += frame.triangles_degenerate_or_offscreen;
    total.raster_bbox_cells += frame.raster_bbox_cells;
    total.raster_visited_cells += frame.raster_visited_cells;
    total.raster_inside_cells += frame.raster_inside_cells;
    total.depth_rejected_cells += frame.depth_rejected_cells;
    total.shade_calls += frame.shade_calls;
    total.shade_discarded_cells += frame.shade_discarded_cells;
    total.coplanar_rejected_cells += frame.coplanar_rejected_cells;
    total.cells_written += frame.cells_written;
}

fn print_profile(profile: &RenderProfile, frames: u16, width: u16, height: u16) {
    let count = f64::from(frames);
    let total_ms = millis(profile.total) / count;
    eprintln!(
        "  phases: animation={:.2}ms ({:.1}%), projection={:.2}ms ({:.1}%), depth_buffer={:.2}ms ({:.1}%), faces+raster={:.2}ms ({:.1}%)",
        millis(profile.animation) / count,
        percent(profile.animation, profile.total),
        millis(profile.projection) / count,
        percent(profile.projection, profile.total),
        millis(profile.depth_buffer) / count,
        percent(profile.depth_buffer, profile.total),
        millis(profile.face_rendering) / count,
        percent(profile.face_rendering, profile.total),
    );
    eprintln!(
        "  geometry/frame: faces={:.0}, blend={:.0}, triangles={:.0}, culled={:.0}, degenerate/offscreen={:.0}",
        profile.faces_considered as f64 / count,
        profile.blend_faces as f64 / count,
        profile.triangles_considered as f64 / count,
        profile.triangles_culled as f64 / count,
        profile.triangles_degenerate_or_offscreen as f64 / count,
    );
    eprintln!(
        "  raster/frame: bbox={:.0}, visited={:.0}, inside={:.0}, depth_reject={:.0}, shade={:.0}, shade_discard={:.0}, coplanar_reject={:.0}, writes={:.0}",
        profile.raster_bbox_cells as f64 / count,
        profile.raster_visited_cells as f64 / count,
        profile.raster_inside_cells as f64 / count,
        profile.depth_rejected_cells as f64 / count,
        profile.shade_calls as f64 / count,
        profile.shade_discarded_cells as f64 / count,
        profile.coplanar_rejected_cells as f64 / count,
        profile.cells_written as f64 / count,
    );
    eprintln!(
        "  rates: bbox overdraw={:.2}x target, visited/bbox={:.1}%, inside/visited={:.1}%, depth rejection={:.1}%, writes/shades={:.1}%",
        profile.raster_bbox_cells as f64 / count / (f64::from(width) * f64::from(height)),
        ratio(profile.raster_visited_cells, profile.raster_bbox_cells),
        ratio(profile.raster_inside_cells, profile.raster_visited_cells),
        ratio(profile.depth_rejected_cells, profile.raster_inside_cells),
        ratio(profile.cells_written, profile.shade_calls),
    );
    eprintln!("  accounted phase total: {total_ms:.2}ms/frame");
}

fn percent(part: Duration, total: Duration) -> f64 {
    100.0 * part.as_secs_f64() / total.as_secs_f64().max(f64::EPSILON)
}

fn ratio(part: u64, total: u64) -> f64 {
    100.0 * part as f64 / (total.max(1) as f64)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn kib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn memory_status() -> String {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return "unavailable".to_string();
    };
    let rss = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .unwrap_or("VmRSS: unavailable");
    let high_water = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .unwrap_or("VmHWM: unavailable");
    format!("{}, {}", rss.trim(), high_water.trim())
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|cpuinfo| {
            cpuinfo
                .lines()
                .find_map(|line| line.strip_prefix("model name\t: "))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn cpu_affinity() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Cpus_allowed_list:\t"))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}
