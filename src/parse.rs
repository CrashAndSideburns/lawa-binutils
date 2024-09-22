use crate::lex::{ControlStatusRegister, Lexer, Opcode, Register, SegmentPermissions, TokenKind};

use miette::{Result};

use std::iter::Peekable;

#[derive(Debug)]
pub struct Parser<'a> {
    // source: &'a str,
    pub lexer: Peekable<Lexer<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            // source,
            lexer: Lexer::new(source).peekable(),
        }
    }

    pub fn parse(mut self) -> Result<Program<'a>> {
        let mut exports = Vec::new();
        let mut segments = Vec::new();

        loop {
            if self.lexer.next().is_none() {
                break;
            }
            // token should be a LeftParen, too lazy to check

            match self.lexer.next() {
                Some(token) => match token?.token_kind {
                    TokenKind::Export => { loop {
                        match self.lexer.next() {
                            Some(token) => {
                                if token.as_ref().unwrap().token_kind == TokenKind::RightParen {
                                    break;
                                } else if let TokenKind::Label(label) = token.as_ref().unwrap().token_kind {
                                    exports.push(label);
                                }
                            }
                            None => {
                                panic!("block was not closed with parenthesis")
                            }
                        }
                    }
                    continue;
                    },
                    TokenKind::Segment => {}
                    other => {
                        panic!("expected export or segment, found {:?}", other)
                    }
                },
                None => {
                    panic!("expected export or segment, but lexer was empty")
                }
            };

            let permissions = match self.lexer.next() {
                Some(token) => {
                    if let TokenKind::SegmentPermissions(permissions) = token?.token_kind {
                        permissions
                    } else {
                        panic!("did not give permissions to segment")
                    }
                }
                None => {
                    panic!("block was not closed with parenthesis")
                }
            };

            let mut contents = Vec::new();
            loop {
                match self.lexer.peek() {
                    Some(token) => {
                        if token.as_ref().unwrap().token_kind == TokenKind::RightParen {
                            self.lexer.next();
                            break;
                        } else {
                            contents.push(self.parse_code()?);
                        }
                    }
                    None => {
                        panic!("block was not closed with parenthesis")
                    }
                }
            }

            segments.push(Segment {
                permissions,
                contents,
            });
        }

        // TODO: Actually parse shit lmao

        Ok(Program { exports, segments })
    }

    pub fn parse_code(&mut self) -> Result<Code<'a>> {
        match self.lexer.next() {
            Some(token) => match token?.token_kind {
                TokenKind::String(string) => return Ok(Code::String(string)),
                TokenKind::Number(n) => return Ok(Code::Number(n)),
                TokenKind::LeftParen => {}
                other => {
                    panic!("expected code, found {:?}", other)
                }
            },
            None => {
                panic!("expected code, but lexer was empty")
            }
        };

        match self.lexer.next() {
            Some(token) => match token?.token_kind {
                TokenKind::Opcode(opcode) => match opcode {
                    Opcode::JSH => {
                        let imm = self.parse_immediate()?;
                        self.lexer.next();
                        return Ok(Code::JSH { imm });
                    }
                    Opcode::WCSR => {
                        let dst = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::ControlStatusRegister(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give anything to an immediate opcode");
                        };

                        let src = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a second register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give a second anything to an immediate opcode");
                        };

                        self.lexer.next();
                        return Ok(Code::WCSR { src, dst });
                    }
                    Opcode::RCSR => {
                        let dst = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give anything to an immediate opcode");
                        };

                        let src = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::ControlStatusRegister(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a second register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give a second anything to an immediate opcode");
                        };

                        self.lexer.next();
                        return Ok(Code::RCSR { src, dst });
                    }
                    _ if opcode.takes_immediate() => {
                        let dst = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give anything to an immediate opcode");
                        };

                        let src = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a second register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give a second anything to an immediate opcode");
                        };

                        let imm = self.parse_immediate()?;
                        self.lexer.next();
                        return Ok(Code::ImmediateInstruction {
                            opcode,
                            imm,
                            src,
                            dst,
                        });
                    }
                    _ => {
                        let dst = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give anything to an immediate opcode");
                        };

                        let src = if let Some(Ok(token)) = self.lexer.next() {
                            if let TokenKind::Register(register) = token.token_kind {
                                register
                            } else {
                                panic!("you didn't give a second register to an immediate opcode");
                            }
                        } else {
                            panic!("you didn't give a second anything to an immediate opcode");
                        };

                        self.lexer.next();
                        return Ok(Code::Instruction { opcode, src, dst });
                    }
                },
                TokenKind::Block => {}
                other => {
                    panic!(
                        "expected an opcode or a block declaration, found {:?} instead",
                        other
                    )
                }
            },
            None => {
                panic!("code parenthesis wasn't closed!")
            }
        }

        // We have consumed a '(' followed by a 'block'. We are in a block.

        let label = self.lexer.next().unwrap()?;
        let label = if let TokenKind::Label(label) = label.token_kind {
            label
        } else {
            panic!(
                "expected block to begin with label, but began with {:?}",
                label
            )
        };

        let mut contents = Vec::new();

        loop {
            match self.lexer.peek() {
                Some(token) => {
                    if token.as_ref().unwrap().token_kind == TokenKind::RightParen {
                        self.lexer.next();
                        break;
                    } else {
                        contents.push(self.parse_code()?);
                    }
                }
                None => {
                    panic!("block was not closed with parenthesis")
                }
            }
        }

        Ok(Code::Block { label, contents })
    }

    pub fn parse_immediate(&mut self) -> Result<Immediate<'a>> {
        match self.lexer.next() {
            Some(token) => match token?.token_kind {
                TokenKind::Label(label) => Ok(Immediate::Label(label)),
                TokenKind::Number(n) => Ok(Immediate::Number(n)),
                other => {
                    panic!("expected immediate, found {:?}", other)
                }
            },
            None => {
                panic!("expected immediate, but lexer was empty")
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Program<'a> {
    exports: Vec<&'a str>,
    segments: Vec<Segment<'a>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct Segment<'a> {
    permissions: SegmentPermissions,
    contents: Vec<Code<'a>>,
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Immediate<'a> {
    Label(&'a str),
    Number(u16),
}
