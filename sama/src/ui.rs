use crate::emulator::{ControlStatusRegisters, Ram, Registers};

use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Padding, Widget},
};

use tui_textarea::TextArea;

/// a widget for displaying the contents of ram
///
/// TODO: rewrite detailed documentation
pub struct RamWidget<'a> {
    ram: &'a Ram,
    view_offset: u16,
    style: &'a [Style; 0x10000],
}

impl<'a> RamWidget<'a> {
    pub fn new(ram: &'a Ram, view_offset: u16, style: &'a [Style; 0x10000]) -> Self {
        Self {
            ram,
            view_offset,
            style,
        }
    }
}

impl Widget for RamWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // render the surrounding block
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("RAM")
            .padding(Padding::left(1));
        let inner_area = block.inner(area);
        block.render(area, buf);

        // 7 columns are taken up by the address which begins each line and the colon, and
        // displaying each value takes 5 columns, 4 for the value and 1 for the space
        let values_per_row = (inner_area.width - 7) / 5;

        // construct the lines which the widget displays
        let mut lines = Vec::new();
        for row_index in 0..inner_area.height {
            let mut line = vec![Span::raw(format!(
                "{:#06x}:",
                self.view_offset + values_per_row * row_index
            ))];
            for value_index in 0..values_per_row {
                line.push(Span::raw(" "));

                let address = self.view_offset + values_per_row * row_index + value_index;
                line.push(Span::styled(
                    format!("{:04x}", self.ram[address]),
                    self.style[usize::from(address)],
                ));
            }
            lines.push(Line::from(line));
        }

        // render the body of the widget
        Text::from(lines).render(inner_area, buf);
    }
}

/// a widget for displaying the contents of the general-purpose registers
///
/// TODO: rewrite detailed documentation
pub struct RegistersWidget<'a> {
    registers: &'a Registers,
    aliases: &'a [Option<String>; 32],
    visibility_bitmask: u32,
    style: &'a [Style; 32],
}

impl<'a> RegistersWidget<'a> {
    pub fn new(
        registers: &'a Registers,
        aliases: &'a [Option<String>; 32],
        visibility_bitmask: u32,
        style: &'a [Style; 32],
    ) -> Self {
        Self {
            registers,
            aliases,
            visibility_bitmask,
            style,
        }
    }
}

impl RegistersWidget<'_> {
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
                let mut line = Vec::new();

                let register_name = match &self.aliases[usize::from(i)] {
                    Some(a) => format!("{}", a),
                    None => format!("r{}", i),
                };
                line.push(Span::raw(format!(
                    "{:register_names_max_length$} ",
                    register_name
                )));

                line.push(Span::styled(
                    format!("{:#06x}", self.registers[i]),
                    self.style[usize::from(i)],
                ));

                lines.push(Line::from(line));
            }
        }

        // Render all of the lines to the provided area.
        Text::from(lines).render(inner_area, buf);
    }
}

/// a widget for displaying the contents of the control/status registers
///
/// TODO: rewrite detailed documentation
pub struct ControlStatusRegistersWidget<'a> {
    control_status_registers: &'a ControlStatusRegisters,
    aliases: &'a [Option<String>; 32],
    visibility_bitmask: u32,
    style: &'a [Style; 32],
}

impl<'a> ControlStatusRegistersWidget<'a> {
    pub fn new(
        control_status_registers: &'a ControlStatusRegisters,
        aliases: &'a [Option<String>; 32],
        visibility_bitmask: u32,
        style: &'a [Style; 32],
    ) -> Self {
        Self {
            control_status_registers,
            aliases,
            visibility_bitmask,
            style,
        }
    }
}

impl ControlStatusRegistersWidget<'_> {
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
                let mut line = Vec::new();

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
                line.push(Span::raw(format!(
                    "{:register_names_max_length$} ",
                    register_name
                )));

                line.push(Span::styled(
                    format!("{:#06x}", self.control_status_registers[i]),
                    self.style[usize::from(i)],
                ));

                lines.push(Line::from(line));
            }
        }

        Text::from(lines).render(inner_area, buf);
    }
}

/// a widget for displaying a lua prompt, as well as the result of lua evaluation
///
/// TODO: write detailed documentation
pub struct PromptWidget<'a> {
    text_area: &'a TextArea<'a>,
    output: &'a str,
}

impl<'a> PromptWidget<'a> {
    pub fn new(text_area: &'a TextArea<'_>, output: &'a str) -> Self {
        Self { text_area, output }
    }
}

impl Widget for &PromptWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Lua Prompt")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Length(1)])
            .split(inner_area);
        let output_area = split[0];
        let text_area = split[1];

        block.render(area, buf);
        Text::from(self.output).render(output_area, buf);
        self.text_area.render(text_area, buf);
    }
}
