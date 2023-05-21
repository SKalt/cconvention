use super::lookaround::StrIndex;

/// byte and char indices of significant characters in a conventional commit header.
/// Used to parse the header into its constituent parts and to find relevant completions.
#[derive(Debug, Default, Clone)]
pub struct Subject {
    pub line: String,
    pub line_number: usize,
    // TODO: compact to u8? Subjects should never be more than 255 chars
    /// the first opening parenthesis in the subject line
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #   ( -> <Some(4)>
    /// type!: subject
    /// <None>
    /// type: subject()
    /// # -> <None>
    /// ### invalid ###
    /// type ( scope ) ! : subject
    /// #    ( -> <Some(5)>
    /// type scope)!: subject
    /// <None>
    /// ```
    pub open_paren: Option<StrIndex>,
    /// the first closing parenthesis in the subject line.
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #         ) -> <Some(9)>
    /// type!: subject
    /// # -> <None>
    /// type: subject()
    /// # -> <None>
    /// ### invalid ###
    /// type ( scope ) ! : subject
    /// #            ) -> <Some(11)>
    /// type scope)!: subject
    /// <None>
    /// ```
    pub close_paren: Option<StrIndex>,
    /// the first exclamation mark in the subject line *before* the colon.
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #          ! -> <Some(10)>
    /// type!: subject
    /// #   ! -> <Some(3)>
    /// type: subject!()
    /// # -> <None>
    pub bang: Option<StrIndex>,
    pub colon: Option<StrIndex>,
    pub space: Option<StrIndex>,
}
// basics
impl Subject {
    /// parse a conventional commit header into its byte indices
    /// ```txt
    /// type(scope)!: subject
    ///     (     )!:_
    /// ```
    pub(crate) fn new(header: String, line_number: usize) -> Self {
        let mut subject = Self::default();
        subject.line_number = line_number;
        subject.line = header;
        let line = subject.line.as_str();
        // type(scope)!: subject
        // type(scope)! subject
        let prefix = if let Some(colon) = line.find(':') {
            let colon_index = StrIndex {
                byte: colon.try_into().unwrap(),
                char: line[..colon].chars().count().try_into().unwrap(),
            };
            subject.colon = Some(colon_index);
            &line[..colon]
        } else {
            line
        };

        // type(scope)!
        // type(scope) subject
        let prefix = if let Some(bang) = prefix.find('!') {
            let prefix = &prefix[..bang];
            let bang_index = StrIndex {
                byte: bang.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };
            subject.bang = Some(bang_index);
            prefix
        } else {
            prefix
        };

        // type(scope
        // type(scope subject
        let prefix = if let Some(close_paren) = prefix.find(')') {
            let prefix = &prefix[..close_paren];
            let close_paren_index = StrIndex {
                byte: close_paren.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };
            subject.close_paren = Some(close_paren_index);
            prefix
        } else {
            prefix
        };

        // type(
        // type scope subject
        // type subject
        let prefix = if let Some(open_paren) = prefix.find('(') {
            let prefix = &prefix[..open_paren];
            let open_paren_index = StrIndex {
                byte: open_paren.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };

            subject.open_paren = Some(open_paren_index);
            prefix
        } else {
            prefix
        };

        // type scope subject
        // type subject
        if let Some(space) = prefix.find(' ') {
            let prefix = &prefix[..space];
            let space_index = StrIndex {
                byte: space.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };

            subject.space = Some(space_index);
        }
        subject
    }
}

// lookaround & ranges
impl Subject {
    /// returns the char index of the end of the type, NON-INCLUSIVE:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```
    pub(crate) fn type_end(&self) -> StrIndex {
        self.open_paren
            .or(self.bang)
            .or(self.colon)
            .or(self.space)
            .unwrap_or_else(|| StrIndex {
                char: self.line.chars().count().try_into().unwrap(),
                byte: self.line.len().try_into().unwrap(),
            })
    }

    fn scope_start(&self) -> Option<StrIndex> {
        if self.open_paren.is_none() && self.close_paren.is_none() {
            return None;
        } else {
            return self
                .open_paren
                .or(Some(self.type_end()))
                .map(|i| self.next_char(i));
        }
    }
    fn next_char(&self, index: StrIndex) -> StrIndex {
        if let Some(c) = &self.line.as_str()[index.byte as usize..].chars().next() {
            return StrIndex {
                byte: index.byte + TryInto::<u8>::try_into(c.len_utf8()).unwrap(),
                char: index.char + 1,
            };
        } else {
            return index;
        }
    }
    /// returns the char index of the end of the scope:
    /// ```txt
    /// type(scope): <subject>
    /// #         ^
    /// type scope): <subject>
    /// #         ^
    /// type(scope: <subject>
    /// #        ^
    /// type: <subject>
    /// # -> <None>
    /// ```
    pub(crate) fn scope_end(&self) -> Option<StrIndex> {
        if self.close_paren.is_none() && self.open_paren.is_none() {
            return None;
        } else {
            self.close_paren
                // .map(|i| {
                //     if let Some(c) = self.line.as_str()[i.byte as usize..].chars().next() {
                //         return StrIndex {
                //             byte: i.byte + TryInto::<u8>::try_into(c.len_utf8()).unwrap(),
                //             char: i.char + 1,
                //         };
                //     } else {
                //         return i;
                //     };
                // })
                .or(self.bang)
                .or(self.colon)
                .or(self.space)
        }
    }

    pub(crate) fn prefix_end(&self) -> Option<StrIndex> {
        self.colon.or(self.bang).or(self.close_paren).or(self.space)
    }

    pub(crate) fn debug_indices(&self) {
        // TODO: ensure this function call is a no-op in release builds
        eprintln!("debugging indices");
        let mut cursor = 0;

        eprint!("\t{}\n\t", self.line);
        if let Some(open_paren) = &self.open_paren {
            while cursor < open_paren.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("(");
        }
        if let Some(close_paren) = &self.close_paren {
            while cursor < close_paren.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = &self.bang {
            while cursor < bang.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("!");
        }
        if let Some(colon) = &self.colon {
            while cursor < colon.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(":");
        }
        if let Some(space) = &self.space {
            while cursor < space.char {
                cursor += 1;
                eprint!(" ");
            }
            eprint!("_")
        }
        eprintln!("\n");
    }
    pub(crate) fn debug_ranges(&self) {
        // TODO: ensure this function call is a no-op in release builds
        eprintln!("debugging ranges:");
        eprint!("\t{}\n\t", self.line);
        let mut cursor = 0u8;
        while cursor < self.type_end().char {
            cursor += 1;
            eprint!("t")
        }
        if let Some(open_paren) = &self.open_paren {
            while cursor < open_paren.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("(");
        }
        while cursor < self.scope_end().map(|i| i.char).unwrap_or(0) {
            cursor += 1;
            eprint!("s");
        }
        if let Some(close_paren) = &self.close_paren {
            while cursor < close_paren.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = &self.bang {
            while cursor < bang.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("!")
        }
        if let Some(colon) = &self.colon {
            while cursor < colon.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(":")
        }
        if let Some(space) = &self.space {
            while cursor < space.char {
                cursor += 1;
                eprint!(" ")
            }
            eprint!("_")
        }
        eprintln!("\n");
    }
}
// diagnostics
impl Subject {
    fn make_diagnostic(
        &self,
        start: u32,
        end: u32,
        severity: lsp_types::DiagnosticSeverity,
        message: String,
    ) -> lsp_types::Diagnostic {
        super::make_line_diagnostic(self.line_number, start, end, severity, message)
    }

    fn check_line_length(&self, cutoff: usize) -> Option<lsp_types::Diagnostic> {
        let n_chars = self.line.chars().count();
        if n_chars > cutoff {
            Some(self.make_diagnostic(
                cutoff.try_into().unwrap(),
                n_chars.try_into().unwrap(),
                lsp_types::DiagnosticSeverity::ERROR,
                format!("line is longer than {} characters", cutoff),
            ))
        } else {
            None
        }
    }

    fn check_space_in_type(&self) -> Option<lsp_types::Diagnostic> {
        let start = 0;
        let end = self.type_end();
        let type_ = &self.line[start..end.byte as usize];
        if type_.chars().any(|c| c.is_whitespace()) {
            Some(self.make_diagnostic(
                start.try_into().unwrap(),
                end.char.into(),
                lsp_types::DiagnosticSeverity::ERROR,
                format!("type contains whitespace"),
            ))
        } else {
            None
        }
    }
    fn check_scope(&self) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        use lsp_types::DiagnosticSeverity as Severity;
        let end = self.scope_end();
        let start = self.scope_start();
        if let (Some(start), Some(end)) = (start, end) {
            let scope = &self.line[start.byte as usize..end.byte as usize];
            eprintln!("scope: >{:?}<", scope);
            if self.open_paren.is_none() {
                lints.push(self.make_diagnostic(
                    start.char.into(),
                    end.char.into(),
                    Severity::ERROR,
                    "Missing opening parenthesis".into(),
                ));
            } else if self.close_paren.is_none() {
                lints.push(self.make_diagnostic(
                    (end.char - 1).into(),
                    end.char.into(),
                    Severity::ERROR,
                    "Missing closing parenthesis".into(),
                ));
            }
            if scope.chars().any(|c| c.is_whitespace()) {
                lints.push(self.make_diagnostic(
                    start.char.into(),
                    end.char.into(),
                    lsp_types::DiagnosticSeverity::ERROR,
                    format!("scope contains whitespace"),
                ));
            }
        }
        lints
    }

    fn check_colon(&self) -> Vec<lsp_types::Diagnostic> {
        use lsp_types::DiagnosticSeverity as Severity;
        let mut lints = vec![];
        if let Some(colon) = self.colon {
            // check for illegal characters before colon
            let start = self
                .scope_end()
                .map(|i| self.next_char(i))
                .unwrap_or_else(|| self.type_end());
            let span = &self.line.as_str()[start.byte as usize..colon.byte as usize];
            eprintln!("span: >{:?}<", span);
            for c in span.chars() {
                if c != '!' {
                    lints.push(self.make_diagnostic(
                        start.char as u32,
                        colon.char as u32,
                        Severity::ERROR,
                        format!("Illegal character before colon: {:?}", c),
                    ));
                    break;
                }
            }
        } else {
            // the colon's missing
            let start = self
                .bang
                .or(self.close_paren)
                .map(|i| self.next_char(i))
                .unwrap_or_else(|| self.type_end());
            let end = self.next_char(start);
            let span = &self.line.as_str()[start.byte as usize..end.byte as usize];
            eprintln!("span: >{:?}<", span);

            lints.push(self.make_diagnostic(
                start.char.into(),
                end.char.into(),
                Severity::ERROR,
                "Missing colon".into(),
            ));
        }
        lints
    }

    fn check_space_after_colon(&self) -> Option<lsp_types::Diagnostic> {
        let start = self
            .colon
            .or(self.bang)
            .or(self.close_paren)
            .or_else(|| self.scope_end())
            .map(|i| self.next_char(i))
            .unwrap_or_else(|| self.type_end());
        let line = &self.line.as_str()[start.byte as usize..];
        let has_next_space = line.chars().next().map(|c| c == ' ').unwrap_or(false);
        if !has_next_space {
            let end = self.next_char(start);
            let span = &self.line[start.byte as usize..end.byte as usize];
            eprintln!("span: >{:?}<", span);
            return Some(self.make_diagnostic(
                start.char.into(),
                end.char.into(),
                lsp_types::DiagnosticSeverity::WARNING,
                "Missing space after colon".into(),
            ));
        }
        None
    }
    pub(crate) fn get_diagnostics(&self, cutoff: usize) -> Vec<lsp_types::Diagnostic> {
        let mut lints = Vec::new();

        if let Some(lint) = self.check_line_length(cutoff) {
            lints.push(lint);
        }
        if let Some(lint) = self.check_space_in_type() {
            lints.push(lint);
        }
        lints.extend(self.check_scope());
        lints.extend(self.check_colon());
        if let Some(lint) = self.check_space_after_colon() {
            lints.push(lint);
        }
        lints
    }
}
