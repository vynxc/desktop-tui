use std::io::{self, Write};

/// Removes ANSI state changes that do not affect the terminal image.
///
/// Ratatui's crossterm backend emits a background reset alongside every
/// foreground-color change. The renderer never paints cell backgrounds, so
/// repeating that reset for thousands of cells wastes roughly three bytes per
/// changed cell and eventually applies backpressure to the widget's PTY.
pub(crate) struct AnsiOptimizer<W> {
    inner: W,
    escape: Vec<u8>,
    output: Vec<u8>,
    background_is_reset: bool,
}

impl<W> AnsiOptimizer<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            escape: Vec::with_capacity(32),
            output: Vec::with_capacity(1024),
            background_is_reset: true,
        }
    }

    #[cfg(test)]
    fn into_inner(self) -> W {
        self.inner
    }

    fn finish_escape(&mut self) {
        let is_sgr = self.escape.get(1) == Some(&b'[') && self.escape.last() == Some(&b'm');
        if !is_sgr {
            self.output.extend_from_slice(&self.escape);
            self.escape.clear();
            return;
        }

        let body = &self.escape[2..self.escape.len() - 1];
        let redundant_background_reset = self.background_is_reset && body == b"49";
        let redundant_combined_reset =
            self.background_is_reset && body.ends_with(b";49") && !sgr_sets_background(body);

        if redundant_combined_reset {
            self.output
                .extend_from_slice(&self.escape[..self.escape.len() - 4]);
            self.output.push(b'm');
        } else if !redundant_background_reset {
            self.output.extend_from_slice(&self.escape);
        }

        self.background_is_reset = sgr_background_is_reset(body, self.background_is_reset);
        self.escape.clear();
    }
}

impl<W: Write> Write for AnsiOptimizer<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.output.clear();

        for &byte in buffer {
            if self.escape.is_empty() {
                if byte == 0x1b {
                    self.escape.push(byte);
                } else {
                    self.output.push(byte);
                }
                continue;
            }

            self.escape.push(byte);
            if self.escape.len() == 2 && byte != b'[' {
                self.output.extend_from_slice(&self.escape);
                self.escape.clear();
            } else if self.escape.len() > 2 && (0x40..=0x7e).contains(&byte) {
                self.finish_escape();
            }
        }

        self.inner.write_all(&self.output)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.escape.is_empty() {
            self.inner.write_all(&self.escape)?;
            self.escape.clear();
        }
        self.inner.flush()
    }
}

fn sgr_sets_background(body: &[u8]) -> bool {
    let (parameters, length) = sgr_parameters(body);
    let parameters = &parameters[..length];
    let mut index = 0;
    while index < parameters.len() {
        match parameters[index] {
            48 => return true,
            38 => index += color_parameter_count(&parameters[index..]),
            _ => index += 1,
        }
    }
    false
}

fn sgr_background_is_reset(body: &[u8], initial: bool) -> bool {
    let (parameters, length) = sgr_parameters(body);
    let parameters = &parameters[..length];
    if parameters.is_empty() {
        return true;
    }

    let mut is_reset = initial;
    let mut index = 0;
    while index < parameters.len() {
        match parameters[index] {
            0 | 49 => {
                is_reset = true;
                index += 1;
            }
            38 => index += color_parameter_count(&parameters[index..]),
            48 => {
                is_reset = false;
                index += color_parameter_count(&parameters[index..]);
            }
            _ => index += 1,
        }
    }
    is_reset
}

fn sgr_parameters(body: &[u8]) -> ([u16; 16], usize) {
    let mut parameters = [0_u16; 16];
    let mut length = 0;

    for value in body.split(|byte| *byte == b';') {
        if length == parameters.len() {
            break;
        }
        let Some(number) = std::str::from_utf8(value)
            .ok()
            .and_then(|value| value.parse().ok())
        else {
            continue;
        };
        parameters[length] = number;
        length += 1;
    }
    (parameters, length)
}

fn color_parameter_count(parameters: &[u16]) -> usize {
    match parameters.get(1) {
        Some(2) => parameters.len().min(5),
        Some(5) => parameters.len().min(3),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_redundant_background_resets() {
        let mut optimizer = AnsiOptimizer::new(Vec::new());
        optimizer
            .write_all(b"\x1b[38;2;225;225;226;49m#\x1b[49m")
            .unwrap();
        optimizer.flush().unwrap();

        assert_eq!(optimizer.into_inner(), b"\x1b[38;2;225;225;226m#");
    }

    #[test]
    fn preserves_reset_after_colored_background() {
        let mut optimizer = AnsiOptimizer::new(Vec::new());
        optimizer
            .write_all(b"\x1b[48;2;12;34;56m \x1b[38;2;1;2;3;49m#")
            .unwrap();
        optimizer.flush().unwrap();

        assert_eq!(
            optimizer.into_inner(),
            b"\x1b[48;2;12;34;56m \x1b[38;2;1;2;3;49m#"
        );
    }

    #[test]
    fn optimizes_escape_split_across_writes() {
        let mut optimizer = AnsiOptimizer::new(Vec::new());
        optimizer.write_all(b"\x1b[38;2;10").unwrap();
        optimizer.write_all(b";20;30;49").unwrap();
        optimizer.write_all(b"mX").unwrap();
        optimizer.flush().unwrap();

        assert_eq!(optimizer.into_inner(), b"\x1b[38;2;10;20;30mX");
    }
}
