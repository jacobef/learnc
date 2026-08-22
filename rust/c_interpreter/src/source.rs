use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(file: FileId, start: usize, end: usize) -> Self {
        Self { file, start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        debug_assert_eq!(self.file, other.file);
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
    line_origins: Option<Vec<LineOrigin>>,
}

#[derive(Debug, Clone, Copy)]
pub struct LineOrigin {
    pub file: FileId,
    pub line_number: usize,
}

impl SourceFile {
    fn new(path: PathBuf, text: String, line_origins: Option<Vec<LineOrigin>>) -> Self {
        let mut line_starts = vec![0];
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + 1);
            }
        }
        Self {
            path,
            text,
            line_starts,
            line_origins,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let line_start = self.line_starts[line_idx];
        (line_idx + 1, offset.saturating_sub(line_start) + 1)
    }

    fn line_origin(&self, line_number: usize) -> Option<LineOrigin> {
        self.line_origins
            .as_ref()
            .and_then(|origins| origins.get(line_number.saturating_sub(1)))
            .copied()
    }

    pub fn line_text(&self, line_number: usize) -> &str {
        let start = self.line_starts[line_number - 1];
        let end = self
            .line_starts
            .get(line_number)
            .copied()
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches('\n')
    }
}

#[derive(Debug, Clone)]
pub struct Snippet {
    pub path: PathBuf,
    pub line_number: usize,
    pub column: usize,
    pub line_text: String,
    pub marker_start: usize,
    pub marker_len: usize,
}

#[derive(Debug, Default)]
pub struct SourceManager {
    files: Vec<SourceFile>,
}

impl SourceManager {
    pub fn add_file(&mut self, path: PathBuf, text: String) -> FileId {
        let id = FileId(self.files.len());
        self.files.push(SourceFile::new(path, text, None));
        id
    }

    pub fn add_generated_file(
        &mut self,
        path: PathBuf,
        text: String,
        line_origins: Vec<LineOrigin>,
    ) -> FileId {
        let id = FileId(self.files.len());
        self.files
            .push(SourceFile::new(path, text, Some(line_origins)));
        id
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.0]
    }

    pub fn find_file(&self, path: &std::path::Path) -> Option<FileId> {
        self.files
            .iter()
            .position(|file| file.path() == path)
            .map(FileId)
    }

    pub fn span_text(&self, span: Span) -> &str {
        &self.file(span.file).text()[span.start..span.end]
    }

    pub(crate) fn span_on_line(
        &self,
        file: FileId,
        line_number: usize,
        start_column: usize,
        width: usize,
    ) -> Span {
        let source = self.file(file);
        let line_index = line_number.saturating_sub(1);
        let line_start = source
            .line_starts
            .get(line_index)
            .copied()
            .unwrap_or(source.text.len());
        let line_end = source
            .line_starts
            .get(line_index.saturating_add(1))
            .copied()
            .unwrap_or(source.text.len())
            .saturating_sub(1)
            .max(line_start);
        let start = line_start.saturating_add(start_column).min(line_end);
        Span::new(file, start, start.saturating_add(width).min(line_end))
    }

    pub fn snippet(&self, span: Span) -> Snippet {
        let file = self.file(span.file);
        let (line_number, column) = file.line_col(span.start);
        let (file, line_number) = match file.line_origin(line_number) {
            Some(origin) => (self.file(origin.file), origin.line_number),
            None => (file, line_number),
        };
        let line_text = file.line_text(line_number).to_owned();
        let marker_len = span.end.saturating_sub(span.start).max(1);
        Snippet {
            path: file.path().clone(),
            line_number,
            column,
            line_text,
            marker_start: column.saturating_sub(1),
            marker_len,
        }
    }

    pub fn span_location(&self, span: Span) -> (PathBuf, usize, usize) {
        let generated = self.file(span.file);
        let (start_line, _) = generated.line_col(span.start);
        let (end_line, _) = generated.line_col(span.end.saturating_sub(1).max(span.start));
        let start_origin = generated.line_origin(start_line);
        let end_origin = generated.line_origin(end_line);
        let start_file = start_origin.map(|origin| origin.file).unwrap_or(span.file);
        let start_line = start_origin
            .map(|origin| origin.line_number)
            .unwrap_or(start_line);
        let end_line = end_origin
            .filter(|origin| origin.file == start_file)
            .map(|origin| origin.line_number)
            .unwrap_or(start_line);
        (
            self.file(start_file).path().clone(),
            start_line,
            end_line.max(start_line),
        )
    }

    pub fn span_display_range(&self, span: Span) -> (PathBuf, usize, usize, usize, usize) {
        let generated = self.file(span.file);
        let (generated_start_line, generated_start_column) = generated.line_col(span.start);
        let (generated_end_line, generated_end_column) =
            generated.line_col(span.end.max(span.start));
        let start_origin = generated.line_origin(generated_start_line);
        let end_origin = generated.line_origin(generated_end_line);
        let display_file = start_origin
            .map(|origin| self.file(origin.file))
            .unwrap_or(generated);
        let start_line = start_origin
            .map(|origin| origin.line_number)
            .unwrap_or(generated_start_line);
        let end_line = end_origin
            .filter(|origin| self.file(origin.file).path() == display_file.path())
            .map(|origin| origin.line_number)
            .unwrap_or(generated_end_line);
        let start_column = start_origin
            .map(|origin| {
                remap_generated_column(
                    generated.line_text(generated_start_line),
                    self.file(origin.file).line_text(origin.line_number),
                    generated_start_column.saturating_sub(1),
                    RangeEdge::Start,
                )
            })
            .unwrap_or_else(|| generated_start_column.saturating_sub(1));
        let end_column = end_origin
            .filter(|origin| self.file(origin.file).path() == display_file.path())
            .map(|origin| {
                remap_generated_column(
                    generated.line_text(generated_end_line),
                    self.file(origin.file).line_text(origin.line_number),
                    generated_end_column.saturating_sub(1),
                    RangeEdge::End,
                )
            })
            .unwrap_or_else(|| generated_end_column.saturating_sub(1));
        (
            display_file.path().clone(),
            start_line.saturating_sub(1),
            start_column,
            end_line.saturating_sub(1),
            end_column,
        )
    }
}

#[derive(Clone, Copy)]
enum RangeEdge {
    Start,
    End,
}

fn remap_generated_column(
    generated: &str,
    original: &str,
    column: usize,
    edge: RangeEdge,
) -> usize {
    if generated == original {
        return column.min(original.len());
    }

    let generated_bytes = generated.as_bytes();
    let original_bytes = original.as_bytes();
    let common_prefix = generated_bytes
        .iter()
        .zip(original_bytes)
        .take_while(|(generated, original)| generated == original)
        .count();
    let max_suffix = generated_bytes
        .len()
        .saturating_sub(common_prefix)
        .min(original_bytes.len().saturating_sub(common_prefix));
    let common_suffix = generated_bytes
        .iter()
        .rev()
        .zip(original_bytes.iter().rev())
        .take(max_suffix)
        .take_while(|(generated, original)| generated == original)
        .count();

    if column <= common_prefix {
        return column.min(original.len());
    }
    let generated_suffix_start = generated.len().saturating_sub(common_suffix);
    if column >= generated_suffix_start {
        return original
            .len()
            .saturating_sub(generated.len().saturating_sub(column))
            .min(original.len());
    }

    match edge {
        RangeEdge::Start => common_prefix,
        RangeEdge::End => original.len().saturating_sub(common_suffix),
    }
}
