use crate::emulator::{ControlStatusRegisters, Ram, Registers};

use ratatui::{
    prelude::{Buffer, Rect},
    text::{Line, Text},
    widgets::{Block, BorderType, Padding, Widget},
};

pub struct RamWidget<'a> {
    ram: &'a Ram,
    view_offset: u16,
}

impl<'a> RamWidget<'a> {
    pub fn new(ram: &'a Ram) -> Self {
        Self {
            ram,
            view_offset: 0,
        }
    }
}

impl Widget for RamWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("RAM")
            .padding(Padding::horizontal(1));
        let inner_area = block.inner(area);

        block.render(area, buf);

        let displayable_width = (inner_area.width - 7) / 5;

        let mut lines = Vec::new();
        for i in 0..inner_area.height {
            let mut line = format!("{:#06x}:", self.view_offset + displayable_width * i);
            for j in 0..displayable_width {
                line.push_str(&format!(
                    " {:04x}",
                    self.ram[self.view_offset + displayable_width * i + j]
                ));
            }
            lines.push(Line::from(line));
        }

        Text::from(lines).render(inner_area, buf);
    }
}

pub struct RegistersWidget<'a> {
    registers: &'a Registers,

    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
}

impl<'a> RegistersWidget<'a> {
    pub fn new(registers: &'a Registers) -> Self {
        Self {
            registers,
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
        }
    }

    pub fn minimum_width(&self) -> u16 {
        let register_names_max_length = self
            .aliases
            .iter()
            .enumerate()
            .map(|(i, a)| {
                if self.visibility_bitmask & (1 << i) == 0 {
                    0
                } else {
                    a.as_ref().map_or(if i < 10 { 2 } else { 3 }, |s| s.len())
                }
            })
            .max()
            .unwrap_or_default();

        (register_names_max_length as u16) + 11
    }

    pub fn minimum_height(&self) -> u16 {
        (self.visibility_bitmask.count_ones() as u16) + 2
    }
}

impl Widget for RegistersWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Registers")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);

        block.render(area, buf);

        let register_names_max_length = usize::from(self.minimum_width()) - 11;

        // Construct lines to be rendered for each of the visible registers.
        let mut lines = Vec::new();
        for i in 0u16..32 {
            if self.visibility_bitmask & (1 << i) != 0 {
                let register_name = match &self.aliases[usize::from(i)] {
                    Some(a) => format!("{}", a),
                    None => format!("r{}", i),
                };
                lines.push(Line::from(format!(
                    "{:register_names_max_length$} {:#06x}",
                    register_name, self.registers[i]
                )));
            }
        }

        // Render all of the lines to the provided area.
        Text::from(lines).render(inner_area, buf);
    }
}

pub struct ControlStatusRegistersWidget<'a> {
    control_status_registers: &'a ControlStatusRegisters,

    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
}

impl<'a> ControlStatusRegistersWidget<'a> {
    pub fn new(control_status_registers: &'a ControlStatusRegisters) -> Self {
        Self {
            control_status_registers,
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
        }
    }

    pub fn minimum_width(&self) -> u16 {
        let register_names_max_length = self
            .aliases
            .iter()
            .enumerate()
            .map(|(i, a)| {
                if self.visibility_bitmask & (1 << i) == 0 {
                    0
                } else {
                    a.as_ref().map_or(
                        match i {
                            0b00000..=0b01001 => 3,
                            0b01010..=0b01111 => 4,
                            0b10000 => 2,
                            0b10001 => 3,
                            0b10010 => 2,
                            0b10110..=0b10111 => 4,
                            0b11000..=0b11111 => 4,
                            _ => 0,
                        },
                        |s| s.len(),
                    )
                }
            })
            .max()
            .unwrap_or_default();

        (register_names_max_length as u16) + 11
    }

    pub fn minimum_height(&self) -> u16 {
        (self.visibility_bitmask.count_ones() as u16) - 1
    }
}

impl Widget for ControlStatusRegistersWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("C/S Registers")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);

        block.render(area, buf);

        let register_names_max_length = usize::from(self.minimum_width()) - 11;

        let mut lines = Vec::new();
        for i in 0u16..32 {
            if self.visibility_bitmask & (1 << i) != 0 {
                let register_name = match &self.aliases[usize::from(i)] {
                    Some(a) => format!("{}", a),
                    None => match i {
                        0b00000..=0b01111 => format!("im{}", i),
                        0b10000 => format!("iv"),
                        0b10001 => format!("ipc"),
                        0b10010 => format!("ic"),
                        0b10011..=0b10101 => continue,
                        0b10110..=0b10111 => format!("mpc{}", i & 0b00001),
                        0b11000..=0b11111 => format!("mpa{}", i & 0b00111),
                        _ => unreachable!(),
                    },
                };

                lines.push(Line::from(format!(
                    "{:register_names_max_length$} {:#06x}",
                    register_name, self.control_status_registers[i]
                )));
            }
        }

        Text::from(lines).render(inner_area, buf);
    }
}

pub struct PromptWidget<'a> {
    input_buffer: &'a str,
    output_buffer: &'a str,
}

impl<'a> PromptWidget<'a> {
    pub fn new(input_buffer: &'a str, output_buffer: &'a str) -> Self {
        Self {
            input_buffer,
            output_buffer,
        }
    }
}

impl Widget for PromptWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Lua Prompt")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);

        block.render(area, buf);

        Text::from(vec![
            Line::from(self.output_buffer),
            Line::from(format!("> {}█", self.input_buffer)),
        ])
        .render(inner_area, buf);
    }
}
