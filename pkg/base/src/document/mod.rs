pub mod lints;
pub(crate) mod lookaround;
pub(crate) mod subject;
use std::path::PathBuf;

use crop::{Rope, RopeSlice};
use lookaround::{find_byte_offset, to_point};
use subject::Subject;

use crate::LANGUAGE;

use self::lints::LintConfig;

lazy_static! {
    static ref SUBJECT_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(subject) @subject",).unwrap();
    // static ref BODY_QUERY: tree_sitter::Query =
    //     tree_sitter::Query::new(LANGUAGE.clone(), "(body) @body",).unwrap();
    static ref TRAILER_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(trailer) @trailer",).unwrap();
    static ref FILE_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(filepath) @text.uri",).unwrap();
    static ref BAD_TRAILER_QUERY: tree_sitter::Query = tree_sitter::Query::new(
        LANGUAGE.clone(),
        "(trailer (token) @token (value)? @value)",
    ).unwrap();

    // static ref MISSING_BODY_QUERY: tree_sitter::Query = tree_sitter::Query::new(
    //     LANGUAGE.clone(),
    //     "(message) @message",
    // ).unwrap();
}

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

/// state management for a git commit document
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
    fn update_subject(&mut self) {
        self.subject =
            if let Some((subject_line, line_number)) = self.get_subject_line_with_number() {
                let subject = Subject::new(subject_line.to_string(), line_number);
                log_debug!("new subject:");
                log_debug!("\t{}", subject_line);
                log_debug!("\t{}", subject.debug_ranges());

                Some(subject)
            } else {
                None
            };
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

            log_debug!("start..end byte: {}..{}", start_byte, end_byte);
            self.code.replace(start_byte..end_byte, &edit.text);
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
            log_debug!("found end position, submitting edit");
            self.syntax_tree.edit(&tree_sitter::InputEdit {
                start_byte,
                old_end_byte: end_byte,
                new_end_byte: start_byte + edit.text.len(),
                start_position: to_point(range.start),
                old_end_position: to_point(range.end),
                new_end_position,
            });
            log_debug!("parsing");
            {
                // update the semantic ranges --------------------------------------
                let prev_tree = &self.syntax_tree;
                self.syntax_tree = self
                    .parser
                    .parse(&(self.code.to_string()), Some(prev_tree))
                    .unwrap();
                log_info!("{}", &self.syntax_tree.root_node().to_sexp());
                // TODO: detect if the subject line changed.
                // HACK: for now, just recompute the indices
                self.update_subject();
            }
        }

        self
    }
}

/// navigation & queries
impl GitCommitDocument {
    /// returns the 0-indexed line number of each body line, NOT including the subject
    /// line but including trailers and blank lines
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
    fn get_subject_line_with_number(&self) -> Option<(String, usize)> {
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

    /// returns the 0-indexed line number of each trailer
    fn get_trailers_lines(&self) -> Vec<u32> {
        let mut cursor = tree_sitter::QueryCursor::new();
        let code = self.code.to_string();
        let matches = cursor.matches(
            &TRAILER_QUERY,
            self.syntax_tree.root_node(),
            code.as_bytes(),
        );
        let mut line_numbers = vec![];
        for m in matches {
            for c in m.captures {
                // a trailer can be only one line
                // line numbers are 0-indexed, and that's expected
                line_numbers.push(c.node.range().start_point.row as u32);
                // let trailer_text = trailer.utf8_text(code.as_bytes()).unwrap();
                // eprintln!("trailer: {}", trailer_text);
                // eprintln!("\tstart: {:?}", trailer.range().start_point);
                // eprintln!("\tend: {:?}", trailer.range().end_point);
            }
        }
        line_numbers
    }
    pub(crate) fn get_links(&self, repo_root: PathBuf) -> Vec<lsp_types::DocumentLink> {
        let mut cursor = tree_sitter::QueryCursor::new();
        let code = self.code.to_string();
        let matches = cursor.matches(&FILE_QUERY, self.syntax_tree.root_node(), code.as_bytes());
        // let mut path = repo_root.clone();
        let mut result = vec![];
        for m in matches {
            for c in m.captures {
                let text = c.node.utf8_text(code.as_bytes()).unwrap();
                let path = repo_root.join(text);
                // eprintln!("repo root: {:?}", repo_root);
                // eprintln!("path: {:?}", path);
                let range = c.node.range();
                result.push(lsp_types::DocumentLink {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: range.start_point.row as u32,
                            character: range.start_point.column as u32,
                        },
                        end: lsp_types::Position {
                            line: range.end_point.row as u32,
                            character: range.end_point.column as u32,
                        },
                    },
                    target: Some(
                        lsp_types::Url::parse(
                            format!("file://{}", path.to_str().unwrap()).as_str(),
                        )
                        .unwrap(),
                    ),
                    tooltip: None,
                    data: None,
                })
            }
        }
        result
    }
}

// TODO: extract all lints into standalone functions?

/// linting
impl GitCommitDocument {
    pub(crate) fn get_diagnostics(&self, config: &dyn LintConfig) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        if let Some(subject) = &self.subject {
            lints.extend(subject.get_diagnostics(config));
            let mut body_lines = self.get_body();
            if let Some((padding_line_number, next_line)) = body_lines.next() {
                if next_line.chars().next().is_some() {
                    lints.push(config.make_line_lint(
                        "body-leading-blank",
                        "there should be a blank line between the subject and the body".into(),
                        padding_line_number,
                        0,
                        0,
                    ));
                } else {
                    // the first body line is blank
                    // check for multiple blank lines before the body
                    let mut n_blank_lines = 1u8;
                    for (line_number, line) in body_lines {
                        if line.chars().next().is_some() && line.chars().any(|c| !c.is_whitespace())
                        {
                            if n_blank_lines > 1 {
                                lints.push(config.make_lint(
                                    "body-leading-blank", // TODO: make distinct code? Parametrize?
                                    format!("{n_blank_lines} blank lines between subject and body"),
                                    padding_line_number,
                                    0,
                                    line_number,
                                    // since lsp_types::Position line numbers are 1-indexed
                                    // and enumeration line numbers are 0-indexed, `first_body_line_number`
                                    // is the line number of the preceding blank line
                                    0,
                                ));
                            }
                            break;
                        } else {
                            n_blank_lines += 1
                        }
                    }
                    // ignore multiple trailing newlines without a body
                };
            };
        };

        {
            // validation of message body
            lints.extend(self.check_trailers(config))
            // TODO: check trailers are (1) grouped (2) at the end of the document (3) have a blank line before them
        }
        // IDEA: check for common trailer misspellings, e.g. lowercasing of "breaking change:",
        // "signed-off-by:", etc.
        lints
    }

    /// check the first non-subject body line is blank; return the 0-indexed line number if not
    pub(crate) fn get_missing_padding_line_number(&self) -> Option<usize> {
        let mut body_lines = self.get_body();
        if let Some((padding_line_number, next_line)) = body_lines.next() {
            if next_line.chars().next().is_some() {
                return Some(padding_line_number);
            }
        }
        None
    }

    /// check there's a blank line before the first trailer line
    pub(crate) fn get_missing_trailer_padding_line(&self) -> Option<usize> {
        if let Some(first_trailer_line) = self.get_trailers_lines().first() {
            let _body_lines = self.get_body();
            let mut body_lines = _body_lines.peekable();
            while let Some((n, line)) = body_lines.next() {
                if let Some((next_line, _)) = body_lines.peek() {
                    if *(next_line) == (*first_trailer_line) as usize {
                        if line.chars().next().is_some() {
                            return Some(n);
                        }
                    }
                }
            }
        }
        None
    }

    fn check_trailers(&self, config: &dyn LintConfig) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        let trailer_lines = self.get_trailers_lines();
        if trailer_lines.is_empty() {
            return lints; // no trailers => no lints
        }
        if let Some(missing_padding_line) = self.check_missing_trailer_padding_line(config) {
            lints.push(missing_padding_line);
        }
        lints.extend(self.check_trailer_values(config));
        lints.extend(self.check_trailer_arrangement(config));
        // TODO: check for common trailer misspellings
        lints
    }

    fn check_missing_trailer_padding_line(
        &self,
        config: &dyn LintConfig,
    ) -> Option<lsp_types::Diagnostic> {
        if let Some(missing_line) = self.get_missing_trailer_padding_line() {
            Some(config.make_line_lint(
                "footer-leading-blank",
                "missing blank line before trailers".into(),
                missing_line,
                0,
                0,
            ))
        } else {
            None
        }
    }

    fn check_trailer_values(&self, config: &dyn LintConfig) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        let mut cursor = tree_sitter::QueryCursor::new();
        let code = self.code.to_string();
        let trailers = cursor.matches(
            &BAD_TRAILER_QUERY,
            self.syntax_tree.root_node(),
            code.as_bytes(),
        );
        for trailer_match in trailers {
            debug_assert!(trailer_match.captures.len() == 2);
            let value = trailer_match.captures[1].node;
            let value_text = value.utf8_text(&code.as_bytes()).unwrap().trim();
            if value_text.is_empty() {
                let key = trailer_match.captures[0].node;
                let key_text = key.utf8_text(code.as_bytes()).unwrap();
                let start = value.start_position();
                let end = value.end_position();
                lints.push(config.make_lint(
                    "footer-empty",
                    format!("Empty value for trailer {:?}", key_text),
                    start.row,
                    start.column as u32,
                    end.row,
                    end.column as u32,
                ));
            }
        }
        lints
    }
    fn check_trailer_arrangement(&self, config: &dyn LintConfig) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        let _trailer_lines = self.get_trailers_lines();
        let mut trailer_lines = _trailer_lines.iter().peekable();
        let mut body_lines = self.get_body();
        while let Some(trailer_line_number) = trailer_lines.next() {
            while let Some((body_line_number, line)) = body_lines.next() {
                if body_line_number as u32 <= *trailer_line_number {
                    continue;
                } else if Some(&&(body_line_number as u32)) == trailer_lines.peek() {
                    break;
                } else {
                    // this is a body line that comes after a trailer line
                    let n_chars = line.chars().count() as u32;
                    if n_chars == 0 {
                        continue; // ignore empty lines
                    }
                    log_debug!("found body line after trailer: {}", body_line_number);
                    let diagnostic = config.make_line_lint(
                        "INVALID",
                        "Message body after trailer.".into(),
                        body_line_number,
                        0,
                        n_chars,
                    );
                    lints.push(diagnostic);
                }
            }
        }
        lints
    }
}

impl GitCommitDocument {
    pub(crate) fn format(&self) -> Vec<lsp_types::TextEdit> {
        let mut fixes = Vec::<lsp_types::TextEdit>::new();
        if let Some(subject) = &self.subject {
            // always auto-format the subject line, if any
            fixes.push(lsp_types::TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: subject.line_number as u32,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: subject.line_number as u32,
                        character: subject.line.chars().count() as u32,
                    },
                },
                new_text: subject.auto_format(),
            });
            if let Some(missing_subject_padding_line) = self.get_missing_padding_line_number() {
                fixes.push(lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: missing_subject_padding_line as u32,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: missing_subject_padding_line as u32,
                            character: 0,
                        },
                    },
                    new_text: "\n".into(),
                })
            }
            if let Some(missing_trailer_padding_line) = self.get_missing_trailer_padding_line() {
                fixes.push(lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: (missing_trailer_padding_line + 1) as u32,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: (missing_trailer_padding_line + 1) as u32,
                            character: 0,
                        },
                    },
                    new_text: "\n".into(),
                })
            }
        };
        // TODO: ensure trailers are at the end of the commit message
        fixes
    }
}
