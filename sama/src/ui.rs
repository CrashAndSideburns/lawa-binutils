use crate::emulator::{ControlStatusRegisters, Ram, Registers};
use crate::lua::LuaStyle;

use directories::ProjectDirs;

use mlua::{Function, Lua, MultiValue};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Padding, Widget, WidgetRef},
};

use tui_textarea::{CursorMove, TextArea};

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

/// a widget for displaying the contents of ram
///
/// this widget is responsible for rendering a window into the ram of an emulator. this widget will
/// generally be the largest in the displayed ui. `view_offset' defines the point at which the view
/// into ram begins. the displayed addresses of ram begin from `view_offset'. the user will
/// typically want to set this based on the program counter to follow program execution, but may
/// occasionally wish to jump to a different point in ram, say to examine the state of some data
/// structure. styling the widget via a provided lua function is also possible. when displaying the
/// contents of each address in ram, the `style_handle' function will be called, with the address
/// provided as an argument
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
        // render the surrounding block
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("RAM")
            .padding(Padding::horizontal(1));
        let inner_area = block.inner(area);
        block.render(area, buf);

        // 7 columns are taken up by the address which begins each line and the colon, and
        // displaying each value takes 5 columns, 4 for the value and 1 for the space
        let values_per_row = (inner_area.width - 7) / 5;

        // construct the lines which the widget displays, calling the `style_handle' function for
        // each address to determine how it should be styled
        let mut lines = Vec::new();
        for row_index in 0..inner_area.height {
            let mut line = vec![Span::raw(format!(
                "{:#06x}:",
                self.view_offset + values_per_row * row_index
            ))];
            for value_index in 0..values_per_row {
                line.push(Span::raw(" "));

                let address = self.view_offset + values_per_row * row_index + value_index;
                let style = self
                    .style_handle
                    .call::<_, LuaStyle>(address)
                    .unwrap_or_default();

                line.push(Span::styled(format!("{:04x}", self.ram[address]), style));
            }
            lines.push(Line::from(line));
        }

        // render the body of the widget
        Text::from(lines).render(inner_area, buf);
    }
}

// TODO: add lua support for setting aliases and the visibility bitmask. here, that will just mean
// modifying the `new' function. the actual work on the lua side of things will need to be done
// when the widget is constructed
/// a widget for displaying the contents of the general-purpose registers
///
/// this widget is reponsible for rendering the contents of the general-purpose registers. the
/// widget can be styled via a provided lua function. when displaying the contents of a register,
/// the `style_handle' function will be called, with the index of the register provided as an
/// argument
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

// TODO: the same changes to lua support need to be made here as for the registers widget
/// a widget for displaying the contents of the control/status registers
///
/// this widget is reponsible for rendering the contents of the control/status registers. the
/// widget can be styled via a provided lua function. when displaying the contents of a register,
/// the `style_handle' function will be called, with the index of the register provided as an
/// argument
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
    input_buffer: String,

    output_buffer: String,
    history_file: Option<File>,
    history: Vec<String>,
    history_index: usize,
}

impl Default for PromptWidget<'_> {
    fn default() -> Self {
        // FIXME: i'm unwrapping here out of laziness. it should be quite hard to accidentally
        // trigger a crash with this
        let project_dirs = ProjectDirs::from("", "", "sama").unwrap();

        // make sure that the directory which contains the history exists
        create_dir_all(project_dirs.data_dir());

        // fetch the existing history
        let history_file_path = project_dirs.data_dir().join("history");
        let history_file = OpenOptions::new()
            .append(true)
            .create(true)
            .read(true)
            .open(history_file_path)
            .ok();
        let history = history_file
            .as_ref()
            .map(|f| {
                BufReader::new(f)
                    .lines()
                    .filter_map(|r| r.map(|s| s.to_string()).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let history_index = history.len();

        Self {
            text_area: TextArea::default(),
            input_buffer: String::new(),
            output_buffer: String::new(),
            history_file,
            history,
            history_index,
        }
    }
}

impl PromptWidget<'_> {
    pub fn process_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                // FIXME: this feels horribly hacky, but it works, at least as far as i can tell
                KeyCode::Up => {
                    if self.history_index > 0 {
                        self.history_index -= 1;
                        self.text_area =
                            TextArea::new(vec![self.history[self.history_index].clone()]);
                        self.text_area.move_cursor(CursorMove::End);
                    }
                }
                KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.history_index > 0 {
                        self.history_index -= 1;
                        self.text_area =
                            TextArea::new(vec![self.history[self.history_index].clone()]);
                        self.text_area.move_cursor(CursorMove::End);
                    }
                }
                KeyCode::Down => {
                    if self.history_index < self.history.len() {
                        self.history_index += 1;
                        self.text_area = if let Some(history) = self.history.get(self.history_index)
                        {
                            TextArea::new(vec![history.clone()])
                        } else {
                            TextArea::new(vec![self.input_buffer.clone()])
                        };
                        self.text_area.move_cursor(CursorMove::End);
                    }
                }
                KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.history_index < self.history.len() {
                        self.history_index += 1;
                        self.text_area = if let Some(history) = self.history.get(self.history_index)
                        {
                            TextArea::new(vec![history.clone()])
                        } else {
                            TextArea::new(vec![self.input_buffer.clone()])
                        };
                        self.text_area.move_cursor(CursorMove::End);
                    }
                }
                _ => {
                    // if we're currently examining history, we need to check whether or not we're
                    // modifying it, in which case the modified history should become the new input
                    // state.
                    if self.history_index != self.history.len() {
                        let mut new_text_area = self.text_area.clone();
                        new_text_area.input(key_event);

                        // we need to be a bit careful about checking that we're actually making
                        // a modification, since otherwise navigaing through history, copy/pasting,
                        // etc. could cause our current input to be overwritten
                        if self.text_area.lines()[0] != new_text_area.lines()[0] {
                            // we're attempting to modify an entry in history. make it the new
                            // current input
                            self.input_buffer = new_text_area.lines()[0].clone();
                            self.text_area = new_text_area;
                            self.history_index = self.history.len();
                            return;
                        }
                    }

                    self.text_area.input(key_event);

                    if self.history_index == self.history.len() {
                        self.input_buffer = self.text_area.lines()[0].clone();
                    }
                }
            }
        }
    }

    pub fn evaluate_input_buffer(&mut self, lua: &Lua) {
        let input_buffer = self.text_area.lines()[0].clone();

        // FIXME: this is just a repl that i copied from an example in the `mlua' repository. it
        // definitely merits a more careful look
        self.output_buffer = match lua.load(&input_buffer).eval::<MultiValue>() {
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

        // if we evaluate an empty buffer, don't pollute the history with blank lines
        if !input_buffer.is_empty() {
            if let Some(file) = &mut self.history_file {
                writeln!(file, "{}", &input_buffer);
            }
            self.history.push(input_buffer);
            self.history_index = self.history.len();
        }

        self.input_buffer = String::new();

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
