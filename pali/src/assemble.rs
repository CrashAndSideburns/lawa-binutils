use crate::lex::Opcode;
use crate::parse::{Code, Immediate, Parser, Program};

use miette::Result;

use poki::{ExportTableEntry, Poki, RelocationTableEntry};

use std::collections::HashMap;

#[derive(Debug)]
pub struct Assembler<'a> {
    program: Program<'a>,
    partial_poki: Poki<'a>,
    segment_index: u16,
    segment_offset: u16,
}

impl<'a> Assembler<'a> {
    pub fn try_new(source: &'a str) -> Result<Self> {
        let program = Parser::new(source).parse()?;
        let partial_poki = Poki::new_empty();

        Ok(Self {
            program,
            partial_poki,
            segment_index: 0,
            segment_offset: 0,
        })
    }

    pub fn assemble(mut self) -> Result<Poki<'a>> {
        // HACK: This is a total hack. I managed to restructure things in a way that the borrow
        // checker did not appreciate, so I'm just using `clone` as a bandage here until I actually
        // solve the problem.
        for segment in &self.program.segments.clone() {
            for code in segment {
                self.add_code(&code.clone())?;
            }
            self.segment_index += 1;
        }

        Ok(self.partial_poki)
    }

    fn add_code(&mut self, code: &Code<'a>) -> Result<()> {
        match code {
            Code::Block { label, contents } => {
                if self.program.exports.contains(label) {
                    self.partial_poki.segments[usize::from(self.segment_index)]
                        .export_table
                        .push(ExportTableEntry {
                            label,
                            offset: self.segment_offset,
                        })
                }

                for code in contents {
                    self.add_code(code)?;
                }
            }
            Code::String(s) => {
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .extend(s.encode_utf16());
            }
            Code::Number(n) => {
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(*n);
            }
            Code::Instruction { opcode, dst, src } => {
                let instruction = (*opcode as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(instruction);
            }
            Code::ImmediateInstruction {
                opcode,
                dst,
                src,
                imm,
            } => {
                let instruction = (*opcode as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(instruction);
                let immediate = match imm {
                    Immediate::Label(label) => {
                        match self.program.symbol_table()?.get(*label) {
                            Some(SymbolTableEntry {
                                segment_index,
                                segment_offset,
                            }) => {
                                let relocation_table_entry = RelocationTableEntry {
                                    offset: self.segment_offset + 1,
                                    segment_index: *segment_index,
                                    segment_offset: *segment_offset,
                                };
                                self.partial_poki.segments[usize::from(self.segment_index)]
                                    .relocation_table
                                    .push(relocation_table_entry);
                            }
                            None => {
                                let segment_offset = match self
                                    .partial_poki
                                    .unresolved_table
                                    .iter()
                                    .position(|s| s == label)
                                {
                                    Some(segment_offset) => segment_offset,
                                    None => {
                                        self.partial_poki.unresolved_table.push(label);
                                        self.partial_poki.unresolved_table.len() - 1
                                    }
                                };
                                let relocation_table_entry = RelocationTableEntry {
                                    offset: self.segment_offset + 1,
                                    segment_index: 0xFFFF,
                                    segment_offset: u16::try_from(segment_offset).unwrap(),
                                };
                                self.partial_poki.segments[usize::from(self.segment_index)]
                                    .relocation_table
                                    .push(relocation_table_entry);
                            }
                        }
                        0
                    }
                    Immediate::Number(n) => *n,
                };
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(immediate);
            }
            Code::RCSR { dst, src } => {
                let instruction =
                    (Opcode::RCSR as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(instruction);
            }
            Code::WCSR { dst, src } => {
                let instruction =
                    (Opcode::WCSR as u16) | ((*dst as u16) << 6) | ((*src as u16) << 11);
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(instruction);
            }
            Code::JSH { imm } => {
                let immediate = match imm {
                    Immediate::Label(label) => {
                        match self.program.symbol_table()?.get(*label) {
                            Some(SymbolTableEntry {
                                segment_index,
                                segment_offset,
                            }) => {
                                let relocation_table_entry = RelocationTableEntry {
                                    offset: self.segment_offset,
                                    segment_index: *segment_index,
                                    segment_offset: *segment_offset,
                                };
                                self.partial_poki.segments[usize::from(self.segment_index)]
                                    .relocation_table
                                    .push(relocation_table_entry);
                            }
                            None => {
                                let segment_offset = match self
                                    .partial_poki
                                    .unresolved_table
                                    .iter()
                                    .position(|s| s == label)
                                {
                                    Some(segment_offset) => segment_offset,
                                    None => {
                                        self.partial_poki.unresolved_table.push(label);
                                        self.partial_poki.unresolved_table.len() - 1
                                    }
                                };
                                let relocation_table_entry = RelocationTableEntry {
                                    offset: self.segment_offset,
                                    segment_index: 0xFFFF,
                                    segment_offset: u16::try_from(segment_offset).unwrap(),
                                };
                                self.partial_poki.segments[usize::from(self.segment_index)]
                                    .relocation_table
                                    .push(relocation_table_entry);
                            }
                        }
                        0
                    }
                    Immediate::Number(n) => *n,
                };
                let instruction = (Opcode::JSH as u16) | (immediate << 6);
                self.partial_poki.segments[usize::from(self.segment_index)]
                    .contents
                    .push(instruction);
            }
        }
        self.segment_offset += code.size();

        Ok(())
    }
}

impl<'a> Program<'a> {
    // HACK: This whole method is just absolutely disgusting. It works, but I am very unwilling to
    // call it done until I put some effort into making it not abhorrent to look at.
    pub fn symbol_table(&self) -> Result<SymbolTable> {
        fn symbol_table_helper<'a>(
            code: &Vec<Code<'a>>,
            segment_index: u16,
            segment_offset: u16,
            partial_symbol_table: &mut SymbolTable,
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
                            .insert(
                                absolute_symbol.clone(),
                                SymbolTableEntry {
                                    segment_index,
                                    segment_offset,
                                },
                            )
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

type SymbolTable = HashMap<String, SymbolTableEntry>;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct SymbolTableEntry {
    pub segment_index: u16,
    pub segment_offset: u16,
}
