#![cfg(feature = "std")]

use std::io::Write;

use crate::error::ErrMode;
use crate::stream::Stream;

pub struct Depth {
    depth: usize,
    inc: bool,
}

impl Depth {
    pub fn new() -> Self {
        let depth = DEPTH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let inc = true;
        Self { depth, inc }
    }

    pub fn existing() -> Self {
        let depth = DEPTH.load(std::sync::atomic::Ordering::SeqCst);
        let inc = false;
        Self { depth, inc }
    }
}

impl Drop for Depth {
    fn drop(&mut self) {
        if self.inc {
            let _ = DEPTH.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl AsRef<usize> for Depth {
    #[inline(always)]
    fn as_ref(&self) -> &usize {
        &self.depth
    }
}

impl crate::lib::std::ops::Deref for Depth {
    type Target = usize;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.depth
    }
}

static DEPTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub enum Severity {
    Success,
    Backtrack,
    Cut,
    Incomplete,
}

impl Severity {
    pub fn with_result<T, E>(result: &Result<T, ErrMode<E>>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(ErrMode::Backtrack(_)) => Self::Backtrack,
            Err(ErrMode::Cut(_)) => Self::Cut,
            Err(ErrMode::Incomplete(_)) => Self::Incomplete,
        }
    }
}

pub fn start<I: Stream>(
    depth: usize,
    name: &dyn crate::lib::std::fmt::Display,
    count: usize,
    input: &I,
) {
    let ansi_color = ansi_color();
    let reset = if ansi_color {
        anstyle::Reset.render().to_string()
    } else {
        "".to_owned()
    };
    let gutter_style = if ansi_color {
        anstyle::Style::new().bold()
    } else {
        anstyle::Style::new()
    }
    .render();
    let input_style = if ansi_color {
        anstyle::Style::new().underline()
    } else {
        anstyle::Style::new()
    }
    .render();
    let eof_style = if ansi_color {
        anstyle::Style::new().fg_color(Some(anstyle::AnsiColor::Cyan.into()))
    } else {
        anstyle::Style::new()
    }
    .render();

    let (call_width, input_width) = column_widths();

    let count = if 0 < count {
        format!(":{count}")
    } else {
        "".to_owned()
    };
    let call_column = format!("{:depth$}> {name}{count}", "");

    let eof_offset = input.eof_offset();
    let offset = input.offset_at(input_width).unwrap_or(eof_offset);
    let (_, slice) = input.next_slice(offset);

    // The debug version of `slice` might be wider, either due to rendering one byte as two nibbles or
    // escaping in strings.
    let mut debug_slice = format!("{:#?}", slice);
    let (debug_slice, eof) = if let Some(debug_offset) = debug_slice
        .char_indices()
        .enumerate()
        .find_map(|(pos, (offset, _))| (input_width <= pos).then(|| offset))
    {
        debug_slice.truncate(debug_offset);
        let eof = "";
        (debug_slice, eof)
    } else {
        let eof = if debug_slice.chars().count() < input_width {
            "∅"
        } else {
            ""
        };
        (debug_slice, eof)
    };

    let writer = std::io::stderr();
    let mut writer = writer.lock();
    let _ = writeln!(writer, "{call_column:call_width$} {gutter_style}|{reset} {input_style}{debug_slice}{eof_style}{eof}{reset}");
}

pub fn end(
    depth: usize,
    name: &dyn crate::lib::std::fmt::Display,
    count: usize,
    consumed: Option<usize>,
    severity: Severity,
) {
    let ansi_color = ansi_color();
    let reset = if ansi_color {
        anstyle::Reset.render().to_string()
    } else {
        "".to_owned()
    };
    let gutter_style = if ansi_color {
        anstyle::Style::new().bold()
    } else {
        anstyle::Style::new()
    }
    .render();

    let (call_width, _) = column_widths();

    let count = if 0 < count {
        format!(":{count}")
    } else {
        "".to_owned()
    };
    let call_column = format!("{:depth$}< {name}{count}", "");

    let (mut status_style, status) = match severity {
        Severity::Success => {
            let style = anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Green.into()))
                .render();
            let status = format!("+{}", consumed.unwrap_or_default());
            (style, status)
        }
        Severity::Backtrack => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Yellow.into()))
                .render(),
            "backtrack".to_owned(),
        ),
        Severity::Cut => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Red.into()))
                .render(),
            "cut".to_owned(),
        ),
        Severity::Incomplete => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Red.into()))
                .render(),
            "incomplete".to_owned(),
        ),
    };
    if !ansi_color {
        status_style = anstyle::Style::new().render();
    }

    let writer = std::io::stderr();
    let mut writer = writer.lock();
    let _ = writeln!(
        writer,
        "{status_style}{call_column:call_width$}{reset} {gutter_style}|{reset} {status_style}{status}{reset}"
    );
}

pub fn result(depth: usize, name: &dyn crate::lib::std::fmt::Display, severity: Severity) {
    let ansi_color = ansi_color();
    let reset = if ansi_color {
        anstyle::Reset.render().to_string()
    } else {
        "".to_owned()
    };
    let gutter_style = if ansi_color {
        anstyle::Style::new().bold()
    } else {
        anstyle::Style::new()
    }
    .render();

    let (call_width, _) = column_widths();

    let call_column = format!("{:depth$}| {name}", "");

    let (mut status_style, status) = match severity {
        Severity::Success => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Green.into()))
                .render(),
            "",
        ),
        Severity::Backtrack => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Yellow.into()))
                .render(),
            "backtrack",
        ),
        Severity::Cut => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Red.into()))
                .render(),
            "cut",
        ),
        Severity::Incomplete => (
            anstyle::Style::new()
                .fg_color(Some(anstyle::AnsiColor::Red.into()))
                .render(),
            "incomplete",
        ),
    };
    if !ansi_color {
        status_style = anstyle::Style::new().render();
    }

    let writer = std::io::stderr();
    let mut writer = writer.lock();
    let _ = writeln!(
        writer,
        "{status_style}{call_column:call_width$}{reset} {gutter_style}|{reset} {status_style}{status}{reset}"
    );
}

fn ansi_color() -> bool {
    concolor::get(concolor::Stream::Stderr).ansi_color()
}

fn column_widths() -> (usize, usize) {
    let term_width = term_width();

    let min_call_width = 40;
    let min_input_width = 20;
    let decor_width = 3;
    let extra_width = term_width
        .checked_sub(min_call_width + min_input_width + decor_width)
        .unwrap_or_default();
    let call_width = min_call_width + 2 * extra_width / 3;
    let input_width = min_input_width + extra_width / 3;

    (call_width, input_width)
}

fn term_width() -> usize {
    columns_env().or_else(query_width).unwrap_or(80)
}

fn query_width() -> Option<usize> {
    use is_terminal::IsTerminal;
    if std::io::stderr().is_terminal() {
        terminal_size::terminal_size().map(|(w, _h)| w.0.into())
    } else {
        None
    }
}

fn columns_env() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
}
