use std::{any::Any, collections::HashSet, fmt::Write};

use super::lookaround::StrIndex;

/// byte-offsets of ranges in a conventional commit header.
#[derive(Debug, Default, Clone)]
struct PrefixLengths {
    /// the byte-length of the type section of the conventional commit subject.
    /// Always nonzero.
    type_: u8,
    /// the byte-length of the scope section of the conventional commit subject.
    /// Zero iff there is no scope.
    scope: u8,
    /// the byte-length of the rest of the conventional commit subject.
    /// Alternately, the length between the end of the type or scope and the colon.
    rest: u8,
}
impl PrefixLengths {
    fn new(line: &str) -> Self {
        let mut offsets = Self::default();
        enum State {
            /// we're in the type section
            Type,
            /// the number of open parentheses
            Scope,
            ScopeDone,
            /// we _might_ have seen the send of the scope at .0, but we're not sure
            ScopeRecovery(u8),
            /// the scope ended, now we're looking for the colon
            Rest,
            /// we _might_ have seen where the colon should be at .0, but we're not sure
            EndRecovery(u8),
        }
        let mut state = State::Type;
        let mut cursor = 0u8; // the byte offset of the current character
        let mut chars = line.chars();
        for c in chars {
            state = match c {
                // which section to assign the current character to
                '(' => match state {
                    State::Type => State::Scope, // expected start of the scope :)
                    State::Scope | State::ScopeRecovery(_) => state, // continue in an attempt to recover
                    State::Rest | State::ScopeDone => State::EndRecovery(cursor),
                    State::EndRecovery(_) => break, // we probably aren't in the prefix anymore
                },
                ')' => match state {
                    State::Type => State::Rest, // unexpected, but consider ')' as the end of the scope anyway
                    State::Scope | State::ScopeRecovery(_) => State::ScopeDone, // expected end of the scope :)
                    State::Rest | State::ScopeDone => State::EndRecovery(cursor), // unexpected >:( // continue in an attempt to recover
                    State::EndRecovery(_) => break, // we probably aren't in the prefix anymore
                },
                '!' => match state {
                    State::Type => State::Rest, // expected end of the type :)
                    State::Scope => State::ScopeRecovery(cursor), // unexpected ! in the middle of the scope >:(
                    State::ScopeRecovery(n) => {
                        // we probably aren't in the scope anymore
                        debug_assert!(n > 0, "There should be no way to get to ScopeRecovery(0)");
                        offsets.scope = n - offsets.type_;
                        cursor = n;
                        chars = line[n as usize..].chars();
                        State::Rest
                    }
                    State::Rest | State::ScopeDone => State::Rest,
                    State::EndRecovery(_) => state, // ok -- carry on until we see a colon
                },
                ' ' | '\t' => match state {
                    State::Type => State::Type, // unexpected, but continue in hope of seeing '(' | '!' | ':'
                    State::Scope => State::ScopeRecovery(cursor), // unexpected, but continue in hope of seeing ')' | '!' | ':'
                    State::ScopeRecovery(n) => {
                        // we probably aren't in the scope anymore
                        debug_assert!(n > 0, "There should be no way to get to ScopeRecovery(0)");
                        offsets.scope = n - offsets.type_;
                        cursor = n;
                        chars = line[n as usize..].chars();
                        State::Rest
                    }
                    State::Rest | State::ScopeDone => State::EndRecovery(cursor), // unexpected
                    State::EndRecovery(_) => state, // keep going in hope of seeing a colon
                },
                ':' => match state {
                    State::Type | State::Rest | State::ScopeDone | State::EndRecovery(_) => {
                        // we're done with the prefix, but make sure we count the colon
                        // for the 'rest' section
                        state = State::Rest;
                        offsets.rest += c.len_utf8() as u8;
                        break;
                    }
                    State::Scope => State::ScopeRecovery(cursor), // unexpected; continue in hope of seeing ')'
                    State::ScopeRecovery(_) => {
                        todo!();
                    }
                },
                _ => match state {
                    State::Type | State::Scope | State::ScopeRecovery(_) => state, // par for the course
                    State::Rest | State::ScopeDone => State::EndRecovery(cursor), // anything other than ! and : are unexpected
                    State::EndRecovery(_) => break, // we probably aren't in the prefix anymore
                },
            };

            let len = c.len_utf8() as u8;
            match state {
                State::Type => offsets.type_ += len,
                State::Scope | State::ScopeRecovery(_) | State::ScopeDone => offsets.scope += len,
                State::Rest | State::EndRecovery(_) => offsets.rest += len,
            };
            cursor += len;
        }
        match state {
            State::Type | State::Scope | State::Rest | State::ScopeDone => {} // this is pretty much as good as we can do
            State::ScopeRecovery(n) => {
                // recover, ending the scope at `n`
                debug_assert!(n > 0, "There should be no way to get to ScopeRecovery(0)");
                offsets.scope = n - offsets.type_;
                offsets.rest = 0;
            }
            State::EndRecovery(n) => {
                // recover, rending the scope at `n`
                offsets.rest = n - offsets.scope - offsets.type_;
            }
        };
        offsets
    }
    fn type_byte_range(&self) -> std::ops::Range<usize> {
        0..self.type_.into()
    }
    fn scope_byte_range(&self) -> std::ops::Range<usize> {
        let start = self.type_;
        let end = start + self.scope;
        start.into()..end.into()
    }
    fn rest_byte_range(&self) -> std::ops::Range<usize> {
        let start = self.type_ + self.scope;
        let end = start + self.rest;
        start.into()..end.into()
    }
    fn prefix_end_byte_offset(&self) -> usize {
        (self.type_ + self.scope + self.rest) as usize
    }
    fn prefix_byte_range(&self) -> std::ops::Range<usize> {
        0..self.prefix_end_byte_offset()
    }
}

/// byte and char indices of significant characters in a conventional commit header.
/// Used to parse the header into its constituent parts and to find relevant completions.
#[derive(Debug, Default, Clone)]
pub struct Subject {
    pub line: String,
    pub line_number: u8,
    offsets: PrefixLengths,
}
// basics
// TODO: parsing fns

// fn match_prefix(input: &str) -> &str {
//     if let Some(i) = input
//         .find(':')
//         .or_else(|| input.find('!'))
//         .or_else(|| input.find(')'))
//         .or_else(|| input.find(' '))
//     {
//         return &input[0..i + 1];
//     };
//     if let Some(open_paren) = input.find('(') {
//         // find the next space
//         if let Some(i) = &input[open_paren..].find(' ') {
//             return &input[0..open_paren + i + 1];
//         } else {
//             input
//         }
//     }
//     // TODO: handle "feat(scope subject"
//     input
// }

impl Subject {
    /// parse a conventional commit header into its byte indices
    /// ```txt
    /// type(scope)!: subject
    ///     (     )!:_
    /// ```
    pub(crate) fn new(line: String, line_number: usize) -> Self {
        let offsets = PrefixLengths::new(&line);
        Self {
            line,
            line_number: line_number as u8,
            offsets,
        }
    }
    pub(crate) fn type_text(&self) -> &str {
        &self.line[self.offsets.type_byte_range()]
    }
    pub(crate) fn scope_text(&self) -> &str {
        &self.line[self.offsets.scope_byte_range()]
    }
    pub(crate) fn rest_text(&self) -> &str {
        &self.line[self.offsets.rest_byte_range()]
    }
    pub(crate) fn prefix_text(&self) -> &str {
        &self.line[self.offsets.prefix_byte_range()]
    }
    pub(crate) fn message_text(&self) -> &str {
        &self.line[self.offsets.prefix_end_byte_offset()..]
    }
}

// lookaround & ranges
impl Subject {
    /// returns the char index of the end of the type, NON-INCLUSIVE:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```

    fn next_char(&self, index: StrIndex) -> StrIndex {
        if let Some(c) = &self.line[index.byte as usize..].chars().next() {
            return StrIndex {
                byte: index.byte + TryInto::<u8>::try_into(c.len_utf8()).unwrap(),
                char: index.char + 1,
            };
        } else {
            return index;
        }
    }

    pub(crate) fn debug_ranges(&self) -> String {
        // TODO: ensure this function call is a no-op in release builds
        let n_chars = self.line.chars().count();
        let mut ranges = String::with_capacity(n_chars);
        for _ in self.type_text().chars() {
            ranges.write_char('t').unwrap();
        }
        for _ in self.scope_text().chars() {
            ranges.write_char('s').unwrap();
        }
        for _ in self.rest_text().chars() {
            ranges.write_char('r').unwrap();
        }
        for _ in self.message_text().chars() {
            ranges.write_char('m').unwrap();
        }
        ranges
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
        super::make_line_diagnostic(self.line_number.into(), start, end, severity, message)
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
        let type_text = self.type_text();
        if type_text.chars().any(|c| c.is_whitespace()) {
            Some(self.make_diagnostic(
                0,
                type_text.chars().count().try_into().unwrap(),
                lsp_types::DiagnosticSeverity::WARNING,
                format!("type contains whitespace"),
            ))
        } else {
            None
        }
    }

    fn check_scope(&self) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        use lsp_types::DiagnosticSeverity as Severity;
        let scope_text = self.scope_text();
        let start = self.type_text().chars().count();
        let end = scope_text.chars().count() + start;
        if let Some(open) = scope_text.chars().next() {
            if open != '(' {
                lints.push(self.make_diagnostic(
                    start.try_into().unwrap(),
                    (start + 1).try_into().unwrap(),
                    Severity::WARNING,
                    format!("scope should start with '('"),
                ));
            }
        }
        if let Some(close) = scope_text.chars().last() {
            if close != ')' {
                lints.push(self.make_diagnostic(
                    (end - 1).try_into().unwrap(),
                    end.try_into().unwrap(),
                    Severity::WARNING,
                    format!("scope should end with ')'"),
                ));
            }
        }
        if !scope_text
            .chars()
            .any(|c| !c.is_whitespace() && c != '(' && c != ')')
        {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                end.try_into().unwrap(),
                Severity::WARNING,
                format!("empty scope"),
            ));
        }
        if scope_text.chars().any(|c| c.is_whitespace()) {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                end.try_into().unwrap(),
                Severity::WARNING,
                format!("scope contains whitespace"),
            ));
        }
        lints
    }

    fn check_rest(&self) -> Vec<lsp_types::Diagnostic> {
        use lsp_types::DiagnosticSeverity as Severity;
        let mut lints = vec![];
        let rest_text = self.rest_text();
        let start = self.type_text().chars().count() + self.scope_text().chars().count();
        let end = start + rest_text.chars().count();
        let illegal_chars: String = {
            let unique: HashSet<char> = rest_text
                .chars()
                .filter(|c| *c != '!' && *c != ':')
                .collect();
            let mut sorted: Vec<char> = unique.iter().map(|c| *c).collect();
            sorted.sort();
            sorted.iter().collect()
        };
        if illegal_chars.len() > 0 {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                end.try_into().unwrap(),
                Severity::WARNING,
                format!("illegal characters after type/scope: {:?}", illegal_chars),
            ));
        }
        let missing_colon = if let Some(last) = rest_text.chars().last() {
            last != ':'
        } else {
            true
        };
        if missing_colon {
            lints.push(self.make_diagnostic(
                (end - 1).try_into().unwrap(),
                end.try_into().unwrap(),
                Severity::WARNING,
                format!("Missing colon"),
            ));
        }
        lints
    }

    fn check_message(&self) -> Vec<lsp_types::Diagnostic> {
        let mut lints = vec![];
        let message = self.message_text();
        let missing_space = if let Some(first) = message.chars().next() {
            first != ' '
        } else {
            true
        };
        let start = self.prefix_text().chars().count();
        if missing_space {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                (start + 1).try_into().unwrap(),
                lsp_types::DiagnosticSeverity::WARNING,
                format!("message should start with a space"),
            ));
        }
        if message.len() == 0 || message.chars().all(|c| c.is_whitespace()) {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                (start + message.chars().count()).try_into().unwrap(),
                lsp_types::DiagnosticSeverity::WARNING,
                format!("empty subject message"),
            ));
        }
        let n_leading_space_chars = message.chars().take_while(|c| c.is_whitespace()).count();
        if n_leading_space_chars > 1 {
            lints.push(self.make_diagnostic(
                start.try_into().unwrap(),
                (start + n_leading_space_chars).try_into().unwrap(),
                lsp_types::DiagnosticSeverity::WARNING,
                format!("excess leading whitespace in subject message"),
            ));
        }
        lints
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
        lints.extend(self.check_rest());
        lints.extend(self.check_message());
        lints
    }

    pub(crate) fn auto_format(&self) -> String {
        let mut formatted = String::with_capacity(self.line.len());
        for c in self.type_text().chars() {
            if !c.is_whitespace() && !":!()".contains(c) {
                formatted.write_char(c).unwrap();
            }
        }
        let scope_text = self.scope_text();
        if scope_text.len() > 0 {
            for c in scope_text.chars() {
                if !c.is_whitespace() && !":!".contains(c) {
                    formatted.write_char(c).unwrap();
                }
            }
        }
        for c in self.rest_text().chars() {
            if c == '!' {
                formatted.write_char(c).unwrap();
            }
        }
        formatted.push_str(": ");
        formatted.push_str(self.message_text().trim());
        formatted
    }
}
