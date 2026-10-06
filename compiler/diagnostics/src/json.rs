//! Hand-written JSON output (keeps the crate dependency-free).

use crate::{Diagnostic, SourceMap, Span};

pub fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn field_str(out: &mut String, key: &str, value: &str) {
    escape(key, out);
    out.push(':');
    escape(value, out);
}

fn position(out: &mut String, sources: &SourceMap, span: Span) {
    let file = sources.file(span.file);
    let start = file.line_col(span.start);
    let end = file.line_col(span.end);
    field_str(out, "file", &file.name);
    out.push_str(&format!(
        ",\"line\":{},\"column\":{},\"end_line\":{},\"end_column\":{}",
        start.line, start.column, end.line, end.column
    ));
}

pub fn to_json(d: &Diagnostic, sources: &SourceMap) -> String {
    let mut out = String::from("{");
    field_str(&mut out, "code", d.code);
    out.push(',');
    field_str(&mut out, "severity", d.severity.as_str());
    out.push(',');
    field_str(&mut out, "kind", d.kind);
    out.push(',');
    field_str(&mut out, "message", &d.message);
    if let Some(span) = d.primary_span() {
        out.push(',');
        position(&mut out, sources, span);
    }
    out.push_str(",\"labels\":[");
    for (i, l) in d.labels.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('{');
        position(&mut out, sources, l.span);
        out.push_str(&format!(",\"primary\":{},", l.primary));
        field_str(&mut out, "message", &l.message);
        out.push('}');
    }
    out.push_str("],\"notes\":[");
    for (i, n) in d.notes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        escape(n, &mut out);
    }
    out.push(']');
    if let Some(h) = &d.help {
        out.push(',');
        field_str(&mut out, "help", h);
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, SourceMap, Span};

    #[test]
    fn json_shape() {
        let mut map = SourceMap::new();
        let f = map.add("m.tarn", "x := \"a\n");
        let d = Diagnostic::error("E0002", "unterminated_string", "unterminated string literal")
            .primary(Span::new(f, 5, 7), "string starts here")
            .help("close it with `\"`");
        assert_eq!(
            d.to_json(&map),
            r#"{"code":"E0002","severity":"error","kind":"unterminated_string","message":"unterminated string literal","file":"m.tarn","line":1,"column":6,"end_line":1,"end_column":8,"labels":[{"file":"m.tarn","line":1,"column":6,"end_line":1,"end_column":8,"primary":true,"message":"string starts here"}],"notes":[],"help":"close it with `\"`"}"#
        );
    }
}
