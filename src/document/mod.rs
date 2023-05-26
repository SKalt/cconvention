pub(crate) mod lookaround;
pub(crate) mod subject;
use crop::{Rope, RopeSlice};
use lookaround::{find_byte_offset, to_point};
use subject::Subject;

use crate::LANGUAGE;

lazy_static! {
    static ref SUBJECT_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(subject) @subject",).unwrap();
    // static ref BODY_QUERY: tree_sitter::Query =
    //     tree_sitter::Query::new(LANGUAGE.clone(), "(body) @body",).unwrap();
    static ref TRAILER_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(trailer) @trailer",).unwrap();
}
pub const LINT_PROVIDER: &str = "git conventional commit language server";

pub struct GitCommitDocument {
    pub code: crop::Rope,
    parser: tree_sitter::Parser, // since the parser is stateful, it needs to be owned by the document
    pub syntax_tree: tree_sitter::Tree,
    pub subject: Option<Subject>,
}

fn get_subject_line(code: &Rope) -> Option<(RopeSlice, usize)> {
    for (number, line) in code.lines().enumerate() {
        if line.bytes().next() != Some(b'#') {
            return Some((line, number));
        }
    }
    None
}

fn make_diagnostic(
    start_line: usize,
    start_char: u32,
    end_line: usize,
    end_char: u32,
    severity: lsp_types::DiagnosticSeverity,
    message: String,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        source: Some(LINT_PROVIDER.to_string()),
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: start_line as u32,
                character: start_char,
            },
            end: lsp_types::Position {
                line: end_line as u32,
                character: end_char,
            },
        },
        severity: Some(severity),
        message,
        ..Default::default()
    }
}

/// make a diagnostic for a single line
pub(crate) fn make_line_diagnostic(
    line_number: usize,
    start: u32,
    end: u32,
    severity: lsp_types::DiagnosticSeverity,
    message: String,
) -> lsp_types::Diagnostic {
    make_diagnostic(line_number, start, line_number, end, severity, message)
}

impl GitCommitDocument {
    pub(crate) fn new(text: String) -> Self {
        let code = crop::Rope::from(text.clone());
        let subject = if let Some((subject, line_number)) = get_subject_line(&code) {
            Some(Subject::new(subject.to_string(), line_number))
        } else {
            None
        };
        let mut parser = {
            let language = tree_sitter_gitcommit::language();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(language).unwrap();
            parser.set_timeout_micros(500_000); // .5 seconds
            parser
        };
        let syntax_tree = parser.parse(&text, None).unwrap();
        GitCommitDocument {
            code,
            parser,
            syntax_tree,
            subject,
        }
    }
    fn recompute_indices(&mut self) {
        self.subject =
            if let Some((subject_line, line_number)) = self._get_subject_line_with_number() {
                let subject = Subject::new(subject_line.to_string(), line_number);
                eprintln!("new subject:");
                eprintln!("\t{}", subject_line);
                eprintln!("\t{}", subject.debug_ranges());

                Some(subject)
            } else {
                None
            };
    }
    fn _get_subject_line_with_number(&self) -> Option<(String, usize)> {
        if let Some(node) = self.get_ts_subject_line() {
            return Some((
                node.utf8_text(self.code.to_string().as_bytes())
                    .unwrap()
                    .to_string(),
                node.start_position().row,
            ));
        }
        if let Some((text, number)) = get_subject_line(&self.code) {
            return Some((text.to_string(), number));
        }
        None
    }
    fn get_ts_subject_line(&self) -> Option<tree_sitter::Node> {
        let mut cursor = tree_sitter::QueryCursor::new();
        let names = SUBJECT_QUERY.capture_names();
        let code = self.code.to_string();
        let matches = cursor.matches(
            &SUBJECT_QUERY,
            self.syntax_tree.root_node(),
            code.as_bytes(),
        );
        for m in matches {
            for c in m.captures {
                let name = names[c.index as usize].as_str();
                match name {
                    "subject" => {
                        return Some(c.node);
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }
        None
    }

    pub(crate) fn edit(
        &mut self,
        edits: &[lsp_types::TextDocumentContentChangeEvent],
    ) -> &mut Self {
        // FIXME: sometimes deletions/bulk inserts cause duplicate characters to creep in
        for edit in edits {
            debug_assert!(edit.range.is_some(), "range is none");
            if edit.range.is_none() {
                continue;
            }
            let range = edit.range.unwrap();
            let start_byte = find_byte_offset(&self.code, range.start);
            let end_byte = find_byte_offset(&self.code, range.end);

            eprintln!("start..end byte: {}..{}", start_byte, end_byte);
            self.code.replace(start_byte..end_byte, &edit.text);
            eprintln!("new code:\n{}", self.code.to_string());
            let new_end_position = match edit.text.rfind('\n') {
                Some(ind) => {
                    let num_newlines = edit.text.bytes().filter(|&c| c == b'\n').count();
                    tree_sitter::Point {
                        row: range.start.line as usize + num_newlines,
                        column: edit.text.len() - ind,
                    }
                }
                None => tree_sitter::Point {
                    row: range.end.line as usize,
                    column: range.end.character as usize + edit.text.len(),
                },
            };
            eprintln!("found end position, submitting edit");
            self.syntax_tree.edit(&tree_sitter::InputEdit {
                start_byte,
                old_end_byte: end_byte,
                new_end_byte: start_byte + edit.text.len(),
                start_position: to_point(range.start),
                old_end_position: to_point(range.end),
                new_end_position,
            });
            eprintln!("parsing");
            {
                // update the semantic ranges --------------------------------------
                let prev_tree = &self.syntax_tree;
                self.syntax_tree = self
                    .parser
                    .parse(&(self.code.to_string()), Some(prev_tree))
                    .unwrap();
                eprintln!("{}", &self.syntax_tree.root_node().to_sexp());
                // TODO: detect if the subject line changed.
                // HACK: for now, just recompute the indices
                self.recompute_indices();
            }
        }

        self
    }

    fn get_body(&self) -> impl Iterator<Item = (usize, RopeSlice)> + '_ {
        let subject_line_number = if let Some(subject) = &self.subject {
            subject.line_number + 1
        } else {
            0
        };
        return self
            .code
            .lines()
            .enumerate()
            .skip(subject_line_number.into())
            .filter(|(_, line)| line.bytes().next() != Some(b'#'));
    }

    pub(crate) fn get_missing_padding_line_number(&self) -> Option<usize> {
        let mut body_lines = self.get_body();
        if let Some((padding_line_number, next_line)) = body_lines.next() {
            if next_line.chars().next().is_some() {
                return Some(padding_line_number);
            }
        }
        None
    }
    pub(crate) fn get_diagnostics(&self, cutoff: u8) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        if let Some(subject) = &self.subject {
            lints.extend(subject.get_diagnostics(cutoff));
            let mut body_lines = self.get_body();
            if let Some((padding_line_number, next_line)) = body_lines.next() {
                if next_line.chars().next().is_some() {
                    lints.push(make_line_diagnostic(
                        padding_line_number,
                        0,
                        0,
                        lsp_types::DiagnosticSeverity::WARNING,
                        "there should be a blank line between the subject and the body".into(),
                    ));
                } else {
                    // the first line is blank
                    if let Some((first_body_line_number, _)) = body_lines
                        .filter(|(_, line)| line.chars().next().is_some())
                        .filter(|(_, line)| line.chars().any(|c| !c.is_whitespace()))
                        .next()
                    {
                        if padding_line_number + 1 != first_body_line_number {
                            lints.push(make_diagnostic(
                                padding_line_number,
                                0,
                                first_body_line_number,
                                // since lsp_types::Position line numbers are 1-indexed
                                // and enumeration line numbers are 0-indexed, `first_body_line_number`
                                // is the line number of the preceding blank line
                                0,
                                lsp_types::DiagnosticSeverity::WARNING,
                                "multiple blank lines between subject and body".into(),
                            ));
                        }
                    }
                    // ignore multiple trailing newlines without a body
                };
            };
        };

        { // validation of message body
             // TODO: check trailers are (1) grouped (2) at the end of the document (3) have a blank line before them
        }
        // IDEA: check for common trailer misspellings, e.g. lowercasing of "breaking change:",
        // "signed-off-by:", etc.
        lints
    }
}
