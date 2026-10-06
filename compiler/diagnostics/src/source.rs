//! Source files and byte spans.

/// Index of a file inside a [`SourceMap`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub u32);

/// A half-open byte range `[start, end)` inside one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(file: FileId, start: u32, end: u32) -> Span {
        debug_assert!(start <= end);
        Span { file, start, end }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Smallest span covering both. Both spans must be in the same file.
    pub fn to(self, other: Span) -> Span {
        debug_assert_eq!(self.file, other.file);
        Span::new(self.file, self.start.min(other.start), self.end.max(other.end))
    }
}

/// 1-based line and column. Columns count Unicode scalar values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub column: u32,
}

pub struct SourceFile {
    pub name: String,
    pub text: String,
    /// Byte offset of the start of every line.
    line_starts: Vec<u32>,
}

impl SourceFile {
    fn new(name: String, text: String) -> SourceFile {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        SourceFile { name, text, line_starts }
    }

    pub fn line_col(&self, offset: u32) -> LineCol {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let start = self.line_starts[line] as usize;
        let end = (offset as usize).min(self.text.len());
        let column = self.text[start..end].chars().count() as u32 + 1;
        LineCol { line: line as u32 + 1, column }
    }

    /// Text of a 1-based line without its trailing newline.
    pub fn line_text(&self, line: u32) -> &str {
        let i = (line - 1) as usize;
        let start = self.line_starts[i] as usize;
        let end = self
            .line_starts
            .get(i + 1)
            .map(|&e| e as usize)
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches(['\n', '\r'])
    }

    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }
}

#[derive(Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> SourceMap {
        SourceMap::default()
    }

    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(name.into(), text.into()));
        id
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.0 as usize]
    }

    pub fn line_col(&self, span: Span) -> LineCol {
        self.file(span.file).line_col(span.start)
    }

    pub fn snippet(&self, span: Span) -> &str {
        &self.file(span.file).text[span.start as usize..span.end as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_and_line_text() {
        let mut map = SourceMap::new();
        let id = map.add("a.tarn", "ab\ncdé\n\nx");
        let f = map.file(id);
        assert_eq!(f.line_col(0), LineCol { line: 1, column: 1 });
        assert_eq!(f.line_col(3), LineCol { line: 2, column: 1 });
        // 'é' is two bytes; the column after it is 4.
        assert_eq!(f.line_col(7), LineCol { line: 2, column: 4 });
        assert_eq!(f.line_col(9), LineCol { line: 4, column: 1 });
        assert_eq!(f.line_text(2), "cdé");
        assert_eq!(f.line_text(3), "");
        assert_eq!(f.line_count(), 4);
    }
}
