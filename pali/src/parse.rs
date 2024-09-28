use crate::lex::{ControlStatusRegister, Lexer, Opcode, Register, TokenKind};

use miette::{LabeledSpan, Result};

use std::collections::HashMap;
use std::iter::Peekable;

#[derive(Debug)]
pub struct Parser<'a> {
    source: &'a str,
    pub lexer: Peekable<Lexer<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            lexer: Lexer::new(source).peekable(),
        }
    }

    pub fn parse(mut self) -> Result<Program<'a>> {
        let mut exports = Vec::new();
        let mut segments = [const { Vec::new() }; 8];

        loop {
            // Consume a LeftParen. If there are no more tokens to be consumed, we have finished
            // parsing the entire source.
            let token = self.lexer.next();
            let opening_parenthesis = match token {
                Some(token) => {
                    let token = token?;

                    match token.token_kind {
                        TokenKind::LeftParen => token,
                        other => {
                            return Err(miette::miette!(
                                labels = vec![LabeledSpan::underline(token.source_span)],
                                "expected left parenthesis, found {other} instead",
                            )
                            .with_source_code(self.source.to_string()))
                        }
                    }
                }
                None => break,
            };

            // The next token should either be an Export or a Segment. If it's an Export, we parse
            // out the export statement here. If it's a Segment, we fall through.
            match self.lexer.next() {
                Some(token) => {
                    let token = token?;
                    match token.token_kind {
                        TokenKind::Export => {
                            loop {
                                match self.lexer.next() {
                                    Some(token) => {
                                        let token = token?;

                                        match token.token_kind {
                                            TokenKind::RightParen => break,
                                            TokenKind::Label(label) => {
                                                exports.push(label);
                                            }
                                            other => {
                                                return Err(miette::miette!(
                                                    labels = vec![LabeledSpan::underline(
                                                        token.source_span
                                                    )],
                                                    "expected label, found {other} instead",
                                                )
                                                .with_source_code(self.source.to_string()));
                                            }
                                        }
                                    }
                                    None => {
                                        return Err(miette::miette!(
                                            labels = vec![LabeledSpan::at(
                                                opening_parenthesis.source_span,
                                                "unpaired opening parenthesis"
                                            )],
                                            "expected right parenthesis, found EOF instead",
                                        )
                                        .with_source_code(self.source.to_string()));
                                    }
                                }
                            }
                            continue;
                        }
                        TokenKind::Segment => {}
                        other => {
                            return Err(miette::miette!(
                                labels = vec![LabeledSpan::underline(token.source_span)],
                                "expected export or segment, found {other} instead",
                            )
                            .with_source_code(self.source.to_string()));
                        }
                    }
                }
                None => {
                    return Err(miette::miette!(
                        "expected export or segment, found EOF instead",
                    ));
                }
            };

            // If we fell through to here, we read a Segment token in the previous step, so next we
            // parse out the segment's permissions.
            let permissions = match self.lexer.next() {
                Some(token) => {
                    let token = token?;

                    if let TokenKind::SegmentPermissions(permissions) = token.token_kind {
                        permissions
                    } else {
                        return Err(miette::miette!(
                            labels = vec![LabeledSpan::underline(token.source_span)],
                            "expected segment permissions, found {0} instead",
                            token.token_kind
                        )
                        .with_source_code(self.source.to_string()));
                    }
                }
                None => {
                    return Err(miette::miette!(
                        labels = vec![LabeledSpan::at(
                            opening_parenthesis.source_span,
                            "unpaired opening parenthesis"
                        )],
                        "expected right parenthesis, found EOF instead",
                    )
                    .with_source_code(self.source.to_string()));
                }
            };

            // We're now in the body of a segment. At this point, we just parse Code until we see a
            // RightParen, at which point we have parsed the entire segment.
            loop {
                match self.lexer.peek() {
                    Some(token) => {
                        // HACK: I need to do kind of an awkward dance here. The errors from
                        // `miette` are not `Clone`, so if the next token in the iterator is an
                        // error, I need to actually take the error out of the iterator, since I
                        // can't clone. The `unwrap` is infallible, since we have just confirmed
                        // that the lexer is non-empty by `peek`ing at it.
                        let token = match token {
                            Ok(token) => token,
                            Err(_) => &self.lexer.next().unwrap()?,
                        };

                        if token.token_kind == TokenKind::RightParen {
                            // Remember to consume the RightParen, since here we've only `peek`ed
                            // at it!
                            self.lexer.next();
                            break;
                        } else {
                            segments[usize::from(u16::from(permissions))].push(self.parse_code()?);
                        }
                    }
                    None => {
                        return Err(miette::miette!(
                            labels = vec![LabeledSpan::at(
                                opening_parenthesis.source_span,
                                "unpaired opening parenthesis"
                            )],
                            "expected right parenthesis, found EOF instead",
                        )
                        .with_source_code(self.source.to_string()));
                    }
                }
            }
        }

        // Everything has been parsed. Return the parsed program.
        Ok(Program { exports, segments })
    }

    pub fn parse_register(&mut self) -> Result<Register> {
        match self.lexer.next() {
            Some(token) => {
                let token = token?;

                match token.token_kind {
                    TokenKind::Register(register) => Ok(register),
                    other => Err(miette::miette!(
                        labels = vec![LabeledSpan::underline(token.source_span)],
                        "expected register, found {other} instead",
                    )
                    .with_source_code(self.source.to_string())),
                }
            }
            None => Err(miette::miette!("expected register, found EOF instead")),
        }
    }

    pub fn parse_control_status_register(&mut self) -> Result<ControlStatusRegister> {
        match self.lexer.next() {
            Some(token) => {
                let token = token?;

                match token.token_kind {
                    TokenKind::ControlStatusRegister(control_status_register) => {
                        Ok(control_status_register)
                    }
                    other => Err(miette::miette!(
                        labels = vec![LabeledSpan::underline(token.source_span)],
                        "expected control/status register, found {other} instead",
                    )
                    .with_source_code(self.source.to_string())),
                }
            }
            None => Err(miette::miette!(
                "expected control/status register, found EOF instead"
            )),
        }
    }

    pub fn parse_code(&mut self) -> Result<Code<'a>> {
        // The first token may either be a literal, in which case the code is just the literal, or
        // a LeftParen, in which case we have begun either an instruction or a block. Either way,
        // we fall through.
        let opening_parenthesis = match self.lexer.next() {
            Some(token) => {
                let token = token?;
                match token.token_kind {
                    TokenKind::String(string) => return Ok(Code::String(string)),
                    TokenKind::Number(n) => return Ok(Code::Number(n)),
                    TokenKind::LeftParen => token,
                    other => {
                        return Err(miette::miette!(
                            labels = vec![LabeledSpan::underline(token.source_span)],
                            "expected literal or left parenthesis, found {other} instead",
                        )
                        .with_source_code(self.source.to_string()));
                    }
                }
            }
            None => {
                // NOTE: The reason why this is unreachable is a bit weird. `parse_code` is only
                // called by `parse`, which will only call it after first peeking at the lexer and
                // seeing that it is non-empty. As a result, `parse_code` is *currently* never
                // called with an empty lexer, and so this is unreachable. Of course, changes to
                // the grammar or the parser could very easily change this in the future.
                unreachable!();
            }
        };

        match self.lexer.next() {
            Some(token) => {
                let token = token?;
                match token.token_kind {
                    TokenKind::Opcode(opcode) => {
                        let code = match opcode {
                            Opcode::JSH => {
                                let imm = self.parse_immediate()?;
                                Ok(Code::JSH { imm })
                            }
                            Opcode::WCSR => {
                                let dst = self.parse_control_status_register()?;
                                let src = self.parse_register()?;

                                Ok(Code::WCSR { src, dst })
                            }
                            Opcode::RCSR => {
                                let dst = self.parse_register()?;
                                let src = self.parse_control_status_register()?;

                                Ok(Code::RCSR { src, dst })
                            }
                            _ if opcode.takes_immediate() => {
                                let dst = self.parse_register()?;
                                let src = self.parse_register()?;
                                let imm = self.parse_immediate()?;

                                Ok(Code::ImmediateInstruction {
                                    opcode,
                                    imm,
                                    src,
                                    dst,
                                })
                            }
                            _ => {
                                let dst = self.parse_register()?;
                                let src = self.parse_register()?;

                                Ok(Code::Instruction { opcode, src, dst })
                            }
                        };

                        // Check that we have the appropriate terminating RightParen.
                        match self.lexer.next() {
                            Some(token) => {
                                let token = token?;
                                match token.token_kind {
                                    TokenKind::RightParen => {}
                                    other => {
                                        return Err(miette::miette!(
                                            labels = vec![
                                                LabeledSpan::at(
                                                    opening_parenthesis.source_span,
                                                    "unpaired opening parenthesis"
                                                ),
                                                LabeledSpan::at(
                                                    token.source_span,
                                                    "expected right parenthesis here"
                                                )
                                            ],
                                            "expected right parenthesis, found {other} instead",
                                        )
                                        .with_source_code(self.source.to_string()));
                                    }
                                }
                            }
                            None => {
                                return Err(miette::miette!(
                                    labels = vec![LabeledSpan::at(
                                        opening_parenthesis.source_span,
                                        "unpaired opening parenthesis"
                                    )],
                                    "expected right parenthesis, found EOF instead",
                                )
                                .with_source_code(self.source.to_string()));
                            }
                        }

                        return code;
                    }
                    TokenKind::Block => {}
                    other => {
                        return Err(miette::miette!(
                            labels = vec![LabeledSpan::underline(token.source_span)],
                            "expected immediate, found {other} instead",
                        )
                        .with_source_code(self.source.to_string()))
                    }
                };
            }
            None => {
                return Err(miette::miette!(
                    "expected block or opcode, found EOF instead"
                ));
            }
        }

        // If we've falled through to here, it means that we have consumed a LeftParen followed by
        // a Block. We are in a block, so first parse the label, and then parse conde until we
        // encounter a terminating RightParen.
        let label = match self.lexer.next() {
            Some(token) => {
                let token = token?;
                match token.token_kind {
                    TokenKind::Label(label) => label,
                    other => {
                        return Err(miette::miette!(
                            labels = vec![LabeledSpan::underline(token.source_span)],
                            "expected label, found {other} instead",
                        )
                        .with_source_code(self.source.to_string()));
                    }
                }
            }
            None => {
                return Err(miette::miette!("expected label, found EOF instead"));
            }
        };

        let mut contents = Vec::new();
        loop {
            match self.lexer.peek() {
                Some(token) => {
                    // HACK: Same as in `parse`.
                    let token = match token {
                        Ok(token) => token,
                        Err(_) => &self.lexer.next().unwrap()?,
                    };

                    if token.token_kind == TokenKind::RightParen {
                        // Remember to consume the RightParen, since here we've only `peek`ed
                        // at it!
                        self.lexer.next();
                        break;
                    } else {
                        contents.push(self.parse_code()?);
                    }
                }
                None => {
                    return Err(miette::miette!(
                        labels = vec![LabeledSpan::at(
                            opening_parenthesis.source_span,
                            "unpaired opening parenthesis"
                        )],
                        "expected right parenthesis, found EOF instead",
                    )
                    .with_source_code(self.source.to_string()));
                }
            }
        }

        Ok(Code::Block { label, contents })
    }

    pub fn parse_immediate(&mut self) -> Result<Immediate<'a>> {
        match self.lexer.next() {
            Some(token) => {
                let token = token?;

                match token.token_kind {
                    TokenKind::Label(label) => Ok(Immediate::Label(label)),
                    TokenKind::Number(n) => Ok(Immediate::Number(n)),
                    other => Err(miette::miette!(
                        labels = vec![LabeledSpan::underline(token.source_span)],
                        "expected immediate, found {other} instead",
                    )
                    .with_source_code(self.source.to_string())),
                }
            }
            None => Err(miette::miette!("expected immediate, found EOF instead")),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Program<'a> {
    pub exports: Vec<&'a str>,
    pub segments: [Vec<Code<'a>>; 8],
}

impl<'a> Program<'a> {
    // HACK: This whole method is just absolutely disgusting. It works, but I am very unwilling to
    // call it done until I put some effort into making it not abhorrent to look at.
    pub fn symbol_table(&self) -> Result<HashMap<String, (u16, u16)>> {
        fn symbol_table_helper<'a>(
            code: &Vec<Code<'a>>,
            segment_index: u16,
            segment_offset: u16,
            partial_symbol_table: &mut HashMap<String, (u16, u16)>,
            ctx: String,
        ) -> Result<()> {
            let mut segment_offset = segment_offset;
            for n in code {
                match n {
                    Code::Block { label, contents } => {
                        let absolute_symbol = if ctx.is_empty() {
                            label.to_string()
                        } else {
                            format!("{ctx}.{label}")
                        };
                        if partial_symbol_table
                            .insert(absolute_symbol.clone(), (segment_index, segment_offset))
                            .is_some()
                        {
                            panic!("duplicate definition of label {label}")
                        }
                        symbol_table_helper(
                            contents,
                            segment_index,
                            segment_offset,
                            partial_symbol_table,
                            absolute_symbol,
                        )?;
                    }
                    _ => {}
                }
                segment_offset += n.size();
            }
            Ok(())
        }

        let mut symbol_table = HashMap::new();
        for i in 0u16..8 {
            symbol_table_helper(
                &self.segments[usize::from(i)],
                i,
                0,
                &mut symbol_table,
                String::new(),
            )?;
        }

        Ok(symbol_table)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Code<'a> {
    Block {
        label: &'a str,
        contents: Vec<Code<'a>>,
    },
    String(&'a str),
    Number(u16),
    Instruction {
        opcode: Opcode,
        dst: Register,
        src: Register,
    },
    ImmediateInstruction {
        opcode: Opcode,
        dst: Register,
        src: Register,
        imm: Immediate<'a>,
    },
    RCSR {
        dst: Register,
        src: ControlStatusRegister,
    },
    WCSR {
        dst: ControlStatusRegister,
        src: Register,
    },
    JSH {
        imm: Immediate<'a>,
    },
}

impl Code<'_> {
    pub fn size(&self) -> u16 {
        match self {
            Code::Block { contents, .. } => contents.iter().map(Self::size).sum(),
            Code::String(s) => {
                // FIXME: Either the parser should be proving the guarantee that the string
                // is at most u16::MAX words long in a UTF-16 representation, or we should
                // signal an error here. It is probably preferable for the parser to
                // provide this guarantee.
                u16::try_from(s.encode_utf16().collect::<Vec<_>>().len()).unwrap()
            }
            Code::ImmediateInstruction { .. } => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Immediate<'a> {
    Label(&'a str),
    Number(u16),
}
