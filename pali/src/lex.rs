use miette::{IntoDiagnostic, LabeledSpan, Result, SourceSpan, WrapErr};
use strum::{Display, EnumString};

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::string::ToString;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EnumString)]
#[strum(ascii_case_insensitive)]
#[repr(u16)]
pub enum Opcode {
    ADD = 0b000000,
    SUB = 0b000001,
    AND = 0b000010,
    OR = 0b000011,
    XOR = 0b000100,
    SLL = 0b000101,
    SRL = 0b000110,
    SRA = 0b000111,

    ADDI = 0b001000,

    ANDI = 0b001010,
    ORI = 0b001011,
    XORI = 0b001100,
    SLLI = 0b001101,

    SRAI = 0b001111,
    LD = 0b010000,
    ST = 0b010001,
    DEI = 0b010010,
    DEO = 0b010011,
    RCSR = 0b010100,
    WCSR = 0b010101,
    SWPR = 0b010110,

    LDIO = 0b011000,
    STIO = 0b011001,

    JAL = 0b101000,
    JSH = 0b101001,
    BEQ = 0b101010,
    BNE = 0b101011,
    BLT = 0b101100,
    BGE = 0b101101,
    BLTU = 0b101110,
    BGEU = 0b101111,
}

impl Opcode {
    pub fn takes_immediate(self) -> bool {
        (((self as usize) & 0b001000) != 0) & (self != Self::JSH)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EnumString)]
#[strum(ascii_case_insensitive)]
#[repr(u16)]
pub enum Register {
    R0 = 0b00000,
    R1 = 0b00001,
    R2 = 0b00010,
    R3 = 0b00011,
    R4 = 0b00100,
    R5 = 0b00101,
    R6 = 0b00110,
    R7 = 0b00111,
    R8 = 0b01000,
    R9 = 0b01001,
    R10 = 0b01010,
    R11 = 0b01011,
    R12 = 0b01100,
    R13 = 0b01101,
    R14 = 0b01110,
    R15 = 0b01111,
    R16 = 0b10000,
    R17 = 0b10001,
    R18 = 0b10010,
    R19 = 0b10011,
    R20 = 0b10100,
    R21 = 0b10101,
    R22 = 0b10110,
    R23 = 0b10111,
    R24 = 0b11000,
    R25 = 0b11001,
    R26 = 0b11010,
    R27 = 0b11011,
    R28 = 0b11100,
    R29 = 0b11101,
    R30 = 0b11110,
    R31 = 0b11111,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EnumString)]
#[strum(ascii_case_insensitive)]
#[repr(u16)]
pub enum ControlStatusRegister {
    IM0 = 0b00000,
    IM1 = 0b00001,
    IM2 = 0b00010,
    IM3 = 0b00011,
    IM4 = 0b00100,
    IM5 = 0b00101,
    IM6 = 0b00110,
    IM7 = 0b00111,
    IM8 = 0b01000,
    IM9 = 0b01001,
    IM10 = 0b01010,
    IM11 = 0b01011,
    IM12 = 0b01100,
    IM13 = 0b01101,
    IM14 = 0b01110,
    IM15 = 0b01111,
    IV = 0b10000,
    IPC = 0b10001,
    IC = 0b10010,

    MPC0 = 0b10110,
    MPC1 = 0b10111,
    MPA0 = 0b11000,
    MPA1 = 0b11001,
    MPA2 = 0b11010,
    MPA3 = 0b11011,
    MPA4 = 0b11100,
    MPA5 = 0b11101,
    MPA6 = 0b11110,
    MPA7 = 0b11111,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ParseSegmentPermissionsError;

impl fmt::Display for ParseSegmentPermissionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid segment permissions")
    }
}

impl Error for ParseSegmentPermissionsError {}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct SegmentPermissions {
    readable: bool,
    writable: bool,
    executable: bool,
}

impl From<SegmentPermissions> for u16 {
    fn from(segment_permissions: SegmentPermissions) -> Self {
        (segment_permissions.executable as u16)
            | ((segment_permissions.writable as u16) << 1)
            | ((segment_permissions.readable as u16) << 2)
    }
}

impl FromStr for SegmentPermissions {
    type Err = ParseSegmentPermissionsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (readable, writable, executable) = match s {
            "x" => (false, false, true),
            "w" => (false, true, false),
            "wx" => (false, true, true),
            "r" => (true, false, false),
            "rx" => (true, false, true),
            "rw" => (true, true, false),
            "rwx" => (true, true, true),
            _ => return Err(ParseSegmentPermissionsError),
        };

        Ok(Self {
            readable,
            writable,
            executable,
        })
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Display)]
pub enum TokenKind<'a> {
    // Labels.
    Label(&'a str),

    // Single-character lexemes.
    LeftParen,
    RightParen,

    // Keywords.
    Opcode(Opcode),
    Register(Register),
    ControlStatusRegister(ControlStatusRegister),
    Segment,
    Block,
    Export,

    // Literals.
    Number(u16),
    String(&'a str),
    SegmentPermissions(SegmentPermissions),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Token<'a> {
    pub token_kind: TokenKind<'a>,
    pub source_span: SourceSpan,
}

impl<'a> Token<'a> {
    fn new(token_kind: TokenKind<'a>, source_span: impl Into<SourceSpan>) -> Self {
        Self {
            token_kind,
            source_span: source_span.into(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Lexer<'a> {
    // The source being lexed.
    source: &'a str,
    // The portion of the source which remains unlexed.
    unlexed: &'a str,

    // The current byte (not character) index of the lexer in the source. This needs to be a byte
    // index instead of a character index for `miette`. Note, in particular, that it is not in
    // general the case that `unlexed == source[index..]`.
    index: usize,

    // Whether or not the lexer has errored. If it has, we should always return None.
    errored: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            unlexed: source,

            index: 0,

            errored: false,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        // If the lexer has encountered an error, it should not yield any more tokens.
        if self.errored {
            return None;
        }

        loop {
            // Examine the first character of the portion of the source which remains unlexed.
            let c = self.unlexed.chars().nth(0)?;

            // The types of multi-character tokens that we support.
            enum Started {
                String,
                Number,
                Label,
            }

            // If `c` is a single-character token, return it. If `c` begins a multi-character
            // token, return the type of the multi-character token that was begun. Return an error
            // if an invalid character is encountered.
            let started = match c {
                '(' => {
                    let source_span = self.index..self.index + 1;
                    self.index += 1;
                    self.unlexed = &self.unlexed[1..];
                    return Some(Ok(Token::new(TokenKind::LeftParen, source_span)));
                }
                ')' => {
                    let source_span = self.index..self.index + 1;
                    self.index += 1;
                    self.unlexed = &self.unlexed[1..];
                    return Some(Ok(Token::new(TokenKind::RightParen, source_span)));
                }
                ';' => {
                    // NOTE: Unwrapping here is infallible, as we know that `self.unlexed` is
                    // non-empty, containing at least ';'.
                    let line = self.unlexed.lines().next().unwrap();
                    self.index += line.len();
                    self.unlexed = &self.unlexed[line.len()..];
                    continue;
                }
                _ if c.is_whitespace() => {
                    // NOTE: We need to be a bit careful here. Rust's `is_whitespace` method
                    // returns true if the character is a Unicode whitespace codepoint, not all of
                    // which are one byte when encoded in UTF-8.
                    self.index += c.len_utf8();
                    self.unlexed = &self.unlexed[c.len_utf8()..];
                    continue;
                }
                '"' => Started::String,
                _ if c.is_ascii_digit() => Started::Number,
                _ if unicode_ident::is_xid_start(c) | (c == '_') => Started::Label,
                _ => {
                    // We have encountered some invalid character in `source`.
                    let source_span = self.index..self.index + c.len_utf8();
                    self.errored = true;
                    return Some(Err(miette::miette!(
                        labels = vec![LabeledSpan::underline(source_span)],
                        "encountered invalid character '{c}' in input",
                    )
                    .with_source_code(self.source.to_string())));
                }
            };

            // If we make it here, it means that we are currently at the beginning of some
            // multi-character token of type `started`.
            match started {
                Started::String => {
                    // Find the end of the string literal, extract it, and update the state of the
                    // lexer as appropriate.
                    //
                    // NOTE: There's a bit of weird math that has to be done here. We search for a
                    // closing `"` character, so we examine `unlexed` with the beginning `"`
                    // trimmed off. As a result, we must add 2 to the returned index for it to
                    // actually point to the first character after the end of the string literal.
                    let end_index = match self.unlexed[1..].find('"') {
                        Some(i) => i + 2,
                        None => {
                            self.errored = true;
                            return Some(Err(miette::miette! {
                                    labels = vec![
                                        LabeledSpan::underline(self.index..self.index+self.unlexed.len())
                                    ],
                                    "unterminated string literal",
                                }.with_source_code(self.source.to_string())));
                        }
                    };
                    let literal = &self.unlexed[..end_index];
                    let source_span = self.index..self.index + literal.len();
                    self.index += literal.len();
                    self.unlexed = &self.unlexed[end_index..];

                    // FIXME: Strings are currently not actually escaped, so all this function does
                    // is trim the `"` characters from both ends of the literal. This should be
                    // changed in the future to actually allow escape sequences in string literals.
                    let literal = unescape(literal);
                    return Some(Ok(Token::new(TokenKind::String(literal), source_span)));
                }
                Started::Number => {
                    // Find the end of the numeric literal, extract it, and update the state of the
                    // lexer as appropriate.
                    //
                    // NOTE: There's some rather awkward checking and conditioning that happens
                    // here due to the fact that decimal literals lack the two-character prefix
                    // that numeric literals in other radices have. This is probably mostly
                    // uneliminable messiness.

                    let radix = match self.unlexed.chars().nth(1) {
                        Some('b') if c == '0' => 2,
                        Some('o') if c == '0' => 8,
                        Some('x') if c == '0' => 16,
                        _ => 10,
                    };
                    let end_index = if radix == 10 {
                        self.unlexed
                            .find(|c: char| !c.is_digit(radix))
                            .unwrap_or(self.unlexed.len())
                    } else {
                        self.unlexed[2..]
                            .find(|c: char| !c.is_digit(radix))
                            .unwrap_or(self.unlexed.len())
                            + 2
                    };
                    let literal = if radix == 10 {
                        &self.unlexed[..end_index]
                    } else {
                        &self.unlexed[2..end_index]
                    };
                    let source_span =
                        self.index..self.index + literal.len() + if radix == 10 { 0 } else { 2 };
                    self.index += literal.len() + if radix == 10 { 0 } else { 2 };
                    self.unlexed = &self.unlexed[end_index..];

                    return Some(
                        u16::from_str_radix(literal, radix)
                            .into_diagnostic()
                            .wrap_err("invalid numeric literal")
                            .map(|n| Token::new(TokenKind::Number(n), source_span)),
                    );
                }
                Started::Label => {
                    // Find the end of the label (or keyword) literal, extract it, and update the
                    // state of the lexer as appropriate.
                    let end_index = self
                        .unlexed
                        .find(|c| !(unicode_ident::is_xid_continue(c) | (c == '.')))
                        .unwrap_or(self.unlexed.len());
                    let literal = &self.unlexed[..end_index];
                    let source_span = self.index..self.index + literal.len();
                    self.index += literal.len();
                    self.unlexed = &self.unlexed[end_index..];

                    // Now that the literal has been identified, check if it's a reserved keyword,
                    // in which case the token corresponding to that keyword should be returned,
                    // rather than an identifier.
                    return Some(Ok(if let Ok(opcode) = Opcode::from_str(literal) {
                        Token::new(TokenKind::Opcode(opcode), source_span)
                    } else if let Ok(register) = Register::from_str(literal) {
                        Token::new(TokenKind::Register(register), source_span)
                    } else if let Ok(control_status_register) =
                        ControlStatusRegister::from_str(literal)
                    {
                        Token::new(
                            TokenKind::ControlStatusRegister(control_status_register),
                            source_span,
                        )
                    } else if let Ok(segment_permissions) = SegmentPermissions::from_str(literal) {
                        Token::new(
                            TokenKind::SegmentPermissions(segment_permissions),
                            source_span,
                        )
                    } else if literal.eq_ignore_ascii_case("segment") {
                        Token::new(TokenKind::Segment, source_span)
                    } else if literal.eq_ignore_ascii_case("block") {
                        Token::new(TokenKind::Block, source_span)
                    } else if literal.eq_ignore_ascii_case("export") {
                        Token::new(TokenKind::Export, source_span)
                    } else {
                        Token::new(TokenKind::Label(literal), source_span)
                    }));
                }
            };
        }
    }
}

// Given a slice which refers to a string literal, unescape it.
fn unescape(string_literal: &str) -> &str {
    // TODO: Support proper unescaping. For now, we just trim off the quotation marks.
    &string_literal[1..string_literal.len() - 1]
}
