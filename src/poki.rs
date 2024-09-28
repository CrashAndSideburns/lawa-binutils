use crate::lex::Opcode;
use crate::parse::{Code, Immediate, Program};

use miette::Result;

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poki<'a> {
    segments: [Segment<'a>; 8],
    unresolved_symbols: Vec<&'a str>,
}

impl<'a> Poki<'a> {
    pub fn from_program(program: Program<'a>) -> Result<Self> {
        let symbol_table = program.symbol_table()?;

        let mut segments = [const { Segment::new() }; 8];
        let mut unresolved_symbols = Vec::new();

        for i in 0u16..8 {
            let code = &program.segments[usize::from(i)];
            segments[usize::from(i)].add_code(
                code,
                &symbol_table,
                &mut unresolved_symbols,
                &program.exports,
            );
        }

        Ok(Poki {
            segments,
            unresolved_symbols,
        })
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Begin by writing the header.
        buffer.extend(b"poki");

        for segment in &self.segments {
            buffer.extend(u16::try_from(segment.contents.len()).unwrap().to_ne_bytes());
            buffer.extend(
                u16::try_from(3 * segment.relocation_table.len())
                    .unwrap()
                    .to_ne_bytes(),
            );
            buffer.extend(
                segment
                    .exported_symbols
                    .iter()
                    .map(Export::size)
                    .sum::<u16>()
                    .to_ne_bytes(),
            );
        }

        // Now that the header has been written, write each segment.
        for segment in &self.segments {
            for word in &segment.contents {
                buffer.extend(word.to_ne_bytes());
            }

            for relocation_table_entry in &segment.relocation_table {
                buffer.extend(relocation_table_entry.offset.to_ne_bytes());
                buffer.extend(relocation_table_entry.segment_index.to_ne_bytes());
                buffer.extend(relocation_table_entry.segment_offset.to_ne_bytes());
            }

            for export in &segment.exported_symbols {
                buffer.extend((export.size() - 2).to_ne_bytes());
                for c in export.label.encode_utf16() {
                    buffer.extend(c.to_ne_bytes());
                }
                buffer.extend(export.offset.to_ne_bytes());
            }
        }

        // Finally, write the unresolved symbols.
        for symbol in &self.unresolved_symbols {
            buffer.extend(
                u16::try_from(symbol.encode_utf16().count())
                    .unwrap()
                    .to_ne_bytes(),
            );
            for c in symbol.encode_utf16() {
                buffer.extend(c.to_ne_bytes());
            }
        }

        buffer
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct Segment<'a> {
    contents: Vec<u16>,
    relocation_table: Vec<RelocationTableEntry>,
    exported_symbols: Vec<Export<'a>>,
}

impl<'a> Segment<'a> {
    const fn new() -> Self {
        Self {
            contents: Vec::new(),
            relocation_table: Vec::new(),
            exported_symbols: Vec::new(),
        }
    }

    fn add_code(
        &mut self,
        program: &Vec<Code<'a>>,
        symbol_table: &HashMap<String, (u16, u16)>,
        unresolved_symbols: &mut Vec<&'a str>,
        exports: &Vec<&'a str>,
    ) {
        let mut offset = u16::try_from(self.contents.len()).unwrap();
        for code in program {
            match code {
                Code::Block { label, contents } => {
                    if exports.contains(label) {
                        self.exported_symbols.push(Export { label, offset })
                    }
                    self.add_code(contents, symbol_table, unresolved_symbols, exports);
                }
                Code::String(s) => {
                    self.contents.extend(s.encode_utf16());
                }
                Code::Number(n) => {
                    self.contents.push(*n);
                }
                Code::Instruction { opcode, dst, src } => {
                    let instruction =
                        (*opcode as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                    self.contents.push(instruction);
                }
                Code::ImmediateInstruction {
                    opcode,
                    dst,
                    src,
                    imm,
                } => {
                    let instruction =
                        (*opcode as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                    self.contents.push(instruction);
                    let immediate = match imm {
                        Immediate::Label(label) => {
                            match symbol_table.get(*label) {
                                Some((segment_index, segment_offset)) => {
                                    let relocation_table_entry = RelocationTableEntry {
                                        offset: offset + 1,
                                        segment_index: *segment_index,
                                        segment_offset: *segment_offset,
                                    };
                                    self.relocation_table.push(relocation_table_entry);
                                }
                                None => {
                                    let segment_offset =
                                        match unresolved_symbols.iter().position(|s| s == label) {
                                            Some(segment_offset) => segment_offset,
                                            None => {
                                                unresolved_symbols.push(label);
                                                unresolved_symbols.len() - 1
                                            }
                                        };
                                    let relocation_table_entry = RelocationTableEntry {
                                        offset: offset + 1,
                                        segment_index: 0xFFFF,
                                        segment_offset: u16::try_from(segment_offset).unwrap(),
                                    };
                                    self.relocation_table.push(relocation_table_entry);
                                }
                            }
                            0
                        }
                        Immediate::Number(n) => *n,
                    };
                    self.contents.push(immediate);
                }
                Code::RCSR { dst, src } => {
                    let instruction =
                        (Opcode::RCSR as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                    self.contents.push(instruction);
                }
                Code::WCSR { dst, src } => {
                    let instruction =
                        (Opcode::WCSR as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                    self.contents.push(instruction);
                }
                Code::JSH { imm } => {
                    let immediate = match imm {
                        Immediate::Label(label) => {
                            match symbol_table.get(*label) {
                                Some((segment_index, segment_offset)) => {
                                    let relocation_table_entry = RelocationTableEntry {
                                        offset: offset,
                                        segment_index: *segment_index,
                                        segment_offset: *segment_offset,
                                    };
                                    self.relocation_table.push(relocation_table_entry);
                                }
                                None => {
                                    let segment_offset =
                                        match unresolved_symbols.iter().position(|s| s == label) {
                                            Some(segment_offset) => segment_offset,
                                            None => {
                                                unresolved_symbols.push(label);
                                                unresolved_symbols.len() - 1
                                            }
                                        };
                                    let relocation_table_entry = RelocationTableEntry {
                                        offset: offset,
                                        segment_index: 0xFFFF,
                                        segment_offset: u16::try_from(segment_offset).unwrap(),
                                    };
                                    self.relocation_table.push(relocation_table_entry);
                                }
                            }
                            0
                        }
                        Immediate::Number(n) => *n,
                    };
                    let instruction = (Opcode::JSH as u16) | (immediate << 6);
                    self.contents.push(instruction);
                }
            }
            offset += code.size();
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct Export<'a> {
    label: &'a str,
    offset: u16,
}

impl<'a> Export<'a> {
    fn size(&self) -> u16 {
        u16::try_from(self.label.encode_utf16().count() + 2).unwrap()
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct RelocationTableEntry {
    offset: u16,
    segment_index: u16,
    segment_offset: u16,
}
