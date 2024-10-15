use crate::emulator::{ControlStatusRegisters, Ram, Registers};
use crate::lua::LuaStyle;

use mlua::{Function, Lua, MultiValue};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Padding, Widget, WidgetRef},
};

use tui_textarea::TextArea;

pub struct RamWidget<'a, 'lua> {
    ram: &'a Ram,
    view_offset: u16,
    style_handle: Function<'lua>,
}

impl<'a, 'lua> RamWidget<'a, 'lua> {
    pub fn new(ram: &'a Ram, view_offset: u16, style_handle: Function<'lua>) -> Self {
        Self {
            ram,
            view_offset,
            style_handle,
        }
    }
}

impl Widget for RamWidget<'_, '_> {
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
            let mut line = vec![Span::raw(format!(
                "{:#06x}:",
                self.view_offset + displayable_width * i
            ))];
            for j in 0..displayable_width {
                line.push(Span::raw(" "));

                let address = self.view_offset + displayable_width * i + j;
                let style = self
                    .style_handle
                    .call::<_, LuaStyle>(address)
                    .unwrap_or_default();

                line.push(Span::styled(format!("{:04x}", self.ram[address]), style));
            }
            lines.push(Line::from(line));
        }

        Text::from(lines).render(inner_area, buf);
    }
}

pub struct RegistersWidget<'a, 'lua> {
    registers: &'a Registers,
    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
    style_handle: Function<'lua>,
}

impl<'a, 'lua> RegistersWidget<'a, 'lua> {
    pub fn new(registers: &'a Registers, style_handle: Function<'lua>) -> Self {
        Self {
            registers,
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
            style_handle,
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

impl Widget for RegistersWidget<'_, '_> {
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

                let style = self.style_handle.call::<_, LuaStyle>(i).unwrap_or_default();
                line.push(Span::styled(format!("{:#06x}", self.registers[i]), style));

                lines.push(Line::from(line));
            }
        }

        // Render all of the lines to the provided area.
        Text::from(lines).render(inner_area, buf);
    }
}

pub struct ControlStatusRegistersWidget<'a, 'lua> {
    control_status_registers: &'a ControlStatusRegisters,
    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
    style_handle: Function<'lua>,
}

impl<'a, 'lua> ControlStatusRegistersWidget<'a, 'lua> {
    pub fn new(
        control_status_registers: &'a ControlStatusRegisters,
        style_handle: Function<'lua>,
    ) -> Self {
        Self {
            control_status_registers,
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
            style_handle,
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

impl Widget for ControlStatusRegistersWidget<'_, '_> {
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

                let style = self.style_handle.call::<_, LuaStyle>(i).unwrap_or_default();
                line.push(Span::styled(
                    format!("{:#06x}", self.control_status_registers[i]),
                    style,
                ));

                lines.push(Line::from(line));
            }
        }

        Text::from(lines).render(inner_area, buf);
    }
}

pub struct PromptWidget<'a> {
    text_area: TextArea<'a>,
    output_buffer: String,
    pub lua: Lua,
    // TODO: history
}

impl PromptWidget<'_> {
    pub fn from_lua(lua: Lua) -> Self {
        Self {
            text_area: TextArea::default(),
            output_buffer: String::new(),
            lua,
        }
    }

    pub fn process_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Enter => {
                    self.evaluate_input_buffer();
                }
                KeyCode::Char('m') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.evaluate_input_buffer();
                }
                // TODO: add keybinds for navigating history
                _ => {
                    self.text_area.input(key_event);
                }
            }
        }
    }

    fn evaluate_input_buffer(&mut self) {
        // FIXME: this is just a repl that i copied from an example in the `mlua' repository. it
        // definitely merits a more careful look
        self.output_buffer = match self
            .lua
            .load(self.text_area.lines()[0].clone())
            .eval::<MultiValue>()
        {
            Ok(v) => {
                format!(
                    "{}",
                    v.iter()
                        .map(|value| format!("{:#?}", value))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            Err(e) => {
                format!("{}", e)
            }
        };

        // TODO: add the input buffer to the history

        // NOTE: for some reason, it doesn't seem like there's a `clear' method, or similar, so
        // a completely new value of `text_area' has to be constructed
        self.text_area = TextArea::default();
    }
}

impl WidgetRef for PromptWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
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
        Text::from(self.output_buffer.as_str()).render(output_area, buf);
        self.text_area.render(text_area, buf);
    }
}
