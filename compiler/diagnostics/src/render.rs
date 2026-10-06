//! Plain-text rendering:
//!
//! ```text
//! error[E0002]: unterminated string literal
//!   --> m.tarn:1:6
//!    |
//!  1 | x := "a
//!    |      ^^ string starts here
//!    |
//!    = help: close it with `"`
//! ```

use crate::{Diagnostic, SourceMap};
use std::fmt::Write;

pub fn render(d: &Diagnostic, sources: &SourceMap) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}[{}]: {}", d.severity.as_str(), d.code, d.message);

    // Labels grouped by file (primary file first), then by position; only
    // single-line underlines for now
    // (a multi-line span is underlined to the end of its first line).
    let mut labels: Vec<_> = d.labels.iter().collect();
    let primary_file = d.primary_span().map(|s| s.file);
    labels.sort_by_key(|l| (Some(l.span.file) != primary_file, l.span.file, l.span.start));

    let width = labels
        .iter()
        .map(|l| sources.line_col(l.span).line.to_string().len())
        .max()
        .unwrap_or(1);
    let pad = " ".repeat(width);

    if let Some(span) = d.primary_span() {
        let file = sources.file(span.file);
        let lc = file.line_col(span.start);
        let _ = writeln!(out, "{pad}--> {}:{}:{}", file.name, lc.line, lc.column);
    }

    if !labels.is_empty() {
        let _ = writeln!(out, "{pad} |");
    }
    let mut last_line = None;
    let mut current_file = d.primary_span().map(|s| s.file);
    for l in &labels {
        let file = sources.file(l.span.file);
        let start = file.line_col(l.span.start);
        let text = file.line_text(start.line);
        // Label in another file: announce it, as `::: file:line:col`.
        if current_file != Some(l.span.file) {
            let _ = writeln!(out, "{pad}::: {}:{}:{}", file.name, start.line, start.column);
            current_file = Some(l.span.file);
        }
        if last_line != Some((l.span.file, start.line)) {
            let _ = writeln!(out, "{:>width$} | {}", start.line, text);
            last_line = Some((l.span.file, start.line));
        }
        let end = file.line_col(l.span.end);
        let line_len = text.chars().count() as u32;
        let end_col = if end.line == start.line { end.column } else { line_len + 1 };
        let marks = (end_col.saturating_sub(start.column)).max(1) as usize;
        let mark = if l.primary { "^" } else { "-" };
        let indent = " ".repeat(start.column as usize - 1);
        let msg = if l.message.is_empty() { String::new() } else { format!(" {}", l.message) };
        let _ = writeln!(out, "{pad} | {indent}{}{msg}", mark.repeat(marks));
    }

    if !d.notes.is_empty() || d.help.is_some() {
        let _ = writeln!(out, "{pad} |");
    }
    for n in &d.notes {
        let _ = writeln!(out, "{pad} = note: {n}");
    }
    if let Some(h) = &d.help {
        let _ = writeln!(out, "{pad} = help: {h}");
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, SourceMap, Span};

    #[test]
    fn renders_primary_and_secondary() {
        let mut map = SourceMap::new();
        let f = map.add("main.tarn", "fn main() {\n    b := buffer\n    print(buffer)\n}\n");
        let d = Diagnostic::error("E4001", "use_after_move", "use of moved value `buffer`")
            .primary(Span::new(f, 38, 44), "value used here after move")
            .secondary(Span::new(f, 21, 27), "value moved here")
            .note("`Buffer` is not a copy type");
        let expected = "\
error[E4001]: use of moved value `buffer`
 --> main.tarn:3:11
  |
2 |     b := buffer
  |          ------ value moved here
3 |     print(buffer)
  |           ^^^^^^ value used here after move
  |
  = note: `Buffer` is not a copy type
";
        assert_eq!(d.render(&map), expected);
    }
}
