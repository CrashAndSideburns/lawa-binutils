use crate::lex::Opcode;
use crate::parse::{Code, Immediate, Parser, Program};

use miette::{LabeledSpan, Result, SourceSpan};

use poki::{ExportTableEntry, Poki, RelocationTableEntry};

use std::collections::HashMap;

#[derive(Debug)]
pub struct Assembler<'a> {
    source: &'a str,
    program: Program<'a>,
    partial_poki: Poki,
    segment_index: u16,
    segment_offset: u16,
}

impl<'a> Assembler<'a> {
    pub fn try_new(source: &'a str) -> Result<Self> {
        let program = Parser::new(source).parse()?;
        let partial_poki = Poki::new_empty();

        Ok(Self {
            source,
            program,
            partial_poki,
            segment_index: 0,
            segment_offset: 0,
        })
    }

    fn symbol_table(&self) -> Result<SymbolTable> {
        // HACK: This is another hack. We only really need this method because the `symbol_table`
        // method defined on `Program<'a>` doesn't have access to the source code, and so it can't
        // attach it itself, so we do that here instead.
        self.program
            .symbol_table()
            .map_err(|e| e.with_source_code(self.source.to_string()))
    }

    pub fn assemble(mut self) -> Result<Poki> {
        // At some point I need to run a quick scan to check that I'm not exporting any labels that
        // I haven't defined.
        for export in &self.program.exports {
            if !self.symbol_table()?.contains_key(export.label) {
                return Err(miette::miette!(
                    labels = vec![LabeledSpan::underline(export.source_span)],
                    "label {0} exported, but is not defined",
                    export.label
                )
                .with_source_code(self.source.to_string()));
            }
        }

        // HACK: This is a total hack. I managed to restructure things in a way that the borrow
        // checker did not appreciate, so I'm just using `clone` as a bandage here until I actually
        // solve the problem.
        for segment in &self.program.segments.clone() {
            for code in segment {
                self.add_code(&code)?;
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
                            label: label.label.to_string(),
                            offset: self.segment_offset,
                        });
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
                        match self.symbol_table()?.get(label.label) {
                            Some(SymbolTableEntry {
                                segment_index,
                                segment_offset,
                                ..
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
                                    .position(|s| s == &label.label)
                                {
                                    Some(segment_offset) => segment_offset,
                                    None => {
                                        self.partial_poki
                                            .unresolved_table
                                            .push(label.label.to_string());
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
                        match self.symbol_table()?.get(label.label) {
                            Some(SymbolTableEntry {
                                segment_index,
                                segment_offset,
                                ..
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
                                    .position(|s| s == &label.label)
                                {
                                    Some(segment_offset) => segment_offset,
                                    None => {
                                        self.partial_poki
                                            .unresolved_table
                                            .push(label.label.to_string());
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
    pub fn symbol_table(&self) -> Result<SymbolTable> {
        fn symbol_table_helper<'a>(
            segment: &Vec<Code<'a>>,
            segment_index: u16,
            mut segment_offset: u16,
            partial_symbol_table: &mut SymbolTable,
            ctx: String,
        ) -> Result<()> {
            for code in segment {
                if let Code::Block { label, contents } = code {
                    let absolute_label = if ctx.is_empty() {
                        label.to_string()
                    } else {
                        format!("{ctx}.{label}")
                    };
                    if let Some(previous_definition) = partial_symbol_table.insert(
                        absolute_label.clone(),
                        SymbolTableEntry {
                            segment_index,
                            segment_offset,
                            source_span: label.source_span,
                        },
                    ) {
                        miette::bail!(
                            labels = vec![
                                LabeledSpan::at(
                                    previous_definition.source_span,
                                    "label first defined here"
                                ),
                                LabeledSpan::at(label.source_span, "and again here")
                            ],
                            "label {absolute_label} is defined more than once"
                        );
                    }

                    symbol_table_helper(
                        contents,
                        segment_index,
                        segment_offset,
                        partial_symbol_table,
                        absolute_label,
                    )?;
                }
                segment_offset += code.size();
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
    source_span: SourceSpan,
}
