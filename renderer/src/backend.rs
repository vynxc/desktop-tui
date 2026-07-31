use std::{
    fs::{File, OpenOptions},
    io,
    io::Write,
    os::unix::fs::FileExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use ratatui::{
    backend::{Backend, ClearType, CrosstermBackend, WindowSize},
    buffer::Cell,
    crossterm::{
        cursor::MoveTo,
        queue,
        style::{
            Attribute as CrosstermAttribute, Color as CrosstermColor, Print, SetAttribute,
            SetBackgroundColor, SetForegroundColor,
        },
    },
    layout::{Position, Size},
    style::{Color, Modifier},
};

use ratatui::prelude::IntoCrossterm;

const SHARED_FRAME_MAGIC: &[u8; 8] = b"DTUI001\0";
const SHARED_FRAME_HEADER_SIZE: usize = 64;
const SHARED_FRAME_CELL_SIZE: usize = 8;
const SHARED_FRAME_GENERATION_OFFSET: usize = 32;

#[derive(Clone)]
struct OwnedCell {
    x: u16,
    y: u16,
    cell: Cell,
}

#[derive(Clone, Copy)]
struct RgbCell {
    x: u16,
    y: u16,
    symbol: u8,
    red: u8,
    green: u8,
    blue: u8,
}

impl RgbCell {
    const fn color_key(self) -> u32 {
        u32::from_be_bytes([0, self.red, self.green, self.blue])
    }
}

/// Emits the animated RGB cells grouped by exact color.
///
/// A row-major true-color stream changes SGR color for almost every textured
/// cell. Moving the cursor costs fewer bytes than another RGB escape, so
/// grouping equal colors substantially reduces PTY traffic while producing
/// the exact same terminal image.
pub(crate) struct GroupedCrosstermBackend<W: Write> {
    inner: CrosstermBackend<W>,
    rgb_cells: Vec<RgbCell>,
    fallback_cells: Vec<OwnedCell>,
}

impl<W: Write> GroupedCrosstermBackend<W> {
    pub(crate) const fn new(writer: W) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            rgb_cells: Vec::new(),
            fallback_cells: Vec::new(),
        }
    }
}

impl<W: Write> Backend for GroupedCrosstermBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.rgb_cells.clear();
        self.fallback_cells.clear();

        for (x, y, cell) in content {
            let symbol = cell.symbol().as_bytes();
            let rgb = match cell.fg {
                Color::Rgb(red, green, blue) => Some((red, green, blue)),
                _ => None,
            };
            if cell.bg == Color::Reset
                && cell.underline_color == Color::Reset
                && cell.modifier == Modifier::empty()
                && symbol.len() == 1
                && symbol[0].is_ascii()
            {
                if let Some((red, green, blue)) = rgb {
                    self.rgb_cells.push(RgbCell {
                        x,
                        y,
                        symbol: symbol[0],
                        red,
                        green,
                        blue,
                    });
                    continue;
                }
            }
            self.fallback_cells.push(OwnedCell {
                x,
                y,
                cell: cell.clone(),
            });
        }

        if !self.fallback_cells.is_empty() {
            self.inner.draw(
                self.fallback_cells
                    .iter()
                    .map(|entry| (entry.x, entry.y, &entry.cell)),
            )?;
        }

        self.rgb_cells
            .sort_unstable_by_key(|cell| (cell.color_key(), cell.y, cell.x));

        let writer = self.inner.writer_mut();
        let mut active_color = None;
        let mut last_position = None;
        for cell in &self.rgb_cells {
            let color_key = cell.color_key();
            if active_color != Some(color_key) {
                queue!(
                    writer,
                    SetForegroundColor(
                        Color::Rgb(cell.red, cell.green, cell.blue).into_crossterm()
                    )
                )?;
                active_color = Some(color_key);
                last_position = None;
            }
            if !matches!(last_position, Some((x, y)) if cell.x == x + 1 && cell.y == y) {
                queue!(writer, MoveTo(cell.x, cell.y))?;
            }
            queue!(writer, Print(char::from(cell.symbol)))?;
            last_position = Some((cell.x, cell.y));
        }

        queue!(
            writer,
            SetForegroundColor(CrosstermColor::Reset),
            SetBackgroundColor(CrosstermColor::Reset),
            SetAttribute(CrosstermAttribute::Reset),
        )
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

/// Publishes complete terminal frames without passing them through a PTY.
///
/// Plasma's terminal parser occasionally stops draining its 4 KiB PTY queue,
/// which makes an otherwise-fast render thread block in `write(2)`. The
/// desktop widget uses this double-buffered runtime file instead. A normal
/// terminal continues to use `GroupedCrosstermBackend`.
pub(crate) struct SharedFrameBackend {
    file: File,
    size: Size,
    cursor: Position,
    cells: Vec<u8>,
    sequence: u64,
}

impl SharedFrameBackend {
    pub(crate) fn new(path: &Path, size: Size) -> io::Result<Self> {
        if size.width == 0 || size.height == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "shared frame dimensions must be non-zero",
            ));
        }

        let cell_bytes = usize::from(size.width)
            .checked_mul(usize::from(size.height))
            .and_then(|cells| cells.checked_mul(SHARED_FRAME_CELL_SIZE))
            .ok_or_else(|| io::Error::other("shared frame dimensions overflow"))?;
        let file_size = SHARED_FRAME_HEADER_SIZE
            .checked_add(cell_bytes.saturating_mul(2))
            .ok_or_else(|| io::Error::other("shared frame file size overflow"))?;

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.set_len(file_size as u64)?;

        let mut header = [0_u8; SHARED_FRAME_HEADER_SIZE];
        header[..8].copy_from_slice(SHARED_FRAME_MAGIC);
        header[8..12].copy_from_slice(&u32::from(size.width).to_le_bytes());
        header[12..16].copy_from_slice(&u32::from(size.height).to_le_bytes());
        header[24..28].copy_from_slice(&(SHARED_FRAME_CELL_SIZE as u32).to_le_bytes());
        let generation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ u64::from(std::process::id());
        let generation = generation.max(1);
        header[SHARED_FRAME_GENERATION_OFFSET..SHARED_FRAME_GENERATION_OFFSET + 8]
            .copy_from_slice(&generation.to_le_bytes());
        file.write_all_at(&header, 0)?;

        let mut backend = Self {
            file,
            size,
            cursor: Position::ORIGIN,
            cells: vec![0; cell_bytes],
            sequence: 0,
        };
        backend.clear_cells();
        backend.flush()?;
        Ok(backend)
    }

    fn clear_cells(&mut self) {
        for cell in self.cells.chunks_exact_mut(SHARED_FRAME_CELL_SIZE) {
            cell[..4].copy_from_slice(&u32::from(b' ').to_le_bytes());
            cell[4..8].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    fn set_cell(&mut self, x: u16, y: u16, cell: &Cell) {
        if x >= self.size.width || y >= self.size.height {
            return;
        }
        let index = usize::from(y) * usize::from(self.size.width) + usize::from(x);
        let destination =
            &mut self.cells[index * SHARED_FRAME_CELL_SIZE..][..SHARED_FRAME_CELL_SIZE];
        let codepoint = cell.symbol().chars().next().unwrap_or(' ') as u32;
        destination[..4].copy_from_slice(&codepoint.to_le_bytes());
        let (red, green, blue) = frame_color(cell.fg);
        destination[4..8].copy_from_slice(&[red, green, blue, 255]);
    }
}

impl Backend for SharedFrameBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            self.set_cell(x, y, cell);
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = position.into();
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_cells();
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        if clear_type == ClearType::All {
            self.clear()
        } else {
            Ok(())
        }
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(
                self.size.width.saturating_mul(9),
                self.size.height.saturating_mul(20),
            ),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sequence += 1;
        let buffer_index = (self.sequence & 1) as usize;
        let offset = SHARED_FRAME_HEADER_SIZE + buffer_index * self.cells.len();
        self.file.write_all_at(&self.cells, offset as u64)?;

        // Publishing this naturally-aligned u64 is the commit point. Readers
        // copy the newly active buffer and verify the state did not change.
        let state = (self.sequence << 1) | buffer_index as u64;
        self.file.write_all_at(&state.to_le_bytes(), 16)
    }
}

const fn frame_color(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Black => (0, 0, 0),
        Color::Red => (178, 24, 24),
        Color::Green => (24, 178, 24),
        Color::Yellow => (178, 104, 24),
        Color::Blue => (24, 24, 178),
        Color::Magenta => (178, 24, 178),
        Color::Cyan => (24, 178, 178),
        Color::Gray => (178, 178, 178),
        Color::DarkGray => (104, 104, 104),
        Color::LightRed => (255, 84, 84),
        Color::LightGreen => (84, 255, 84),
        Color::LightYellow => (255, 255, 84),
        Color::LightBlue => (84, 84, 255),
        Color::LightMagenta => (255, 84, 255),
        Color::LightCyan => (84, 255, 255),
        Color::White | Color::Reset | Color::Indexed(_) => (255, 255, 255),
    }
}
