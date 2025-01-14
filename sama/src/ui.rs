use crate::emulator::{ControlStatusRegisters, Emulator, Ram, Registers};
use crate::lua::LuaStyle;

use crossbeam::channel::{Receiver, Sender};

use directories::ProjectDirs;

use mlua::{Function, Lua, Table};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Padding, StatefulWidget, Widget},
};

use tui_textarea::{CursorMove, TextArea};

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};

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
pub struct RamWidget {
    lua: Arc<Mutex<Lua>>,
    emulator: Arc<Mutex<Emulator>>,
}

pub struct RamWidgetState {
    ram: Ram,
    view_offset: u16,
    style: [Style; 0x10000],
}

impl RamWidget {
    pub fn new(lua: Arc<Mutex<Lua>>, emulator: Arc<Mutex<Emulator>>) -> Self {
        Self { lua, emulator }
    }
}

impl Default for RamWidgetState {
    fn default() -> Self {
        Self {
            ram: Ram::default(),
            view_offset: 0,
            style: [Style::default(); 0x10000],
        }
    }
}

impl StatefulWidget for RamWidget {
    type State = RamWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
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

        // update the internal state, if possible. if we cannot update the internal state, we will
        // simply display the old state that we have buffered. for efficiency, we only update the
        // portions of the internal state that will be displayed
        if let Ok(lua) = self.lua.try_lock() {
            if let Ok(view_offset) = lua.load("widgets.ram.view_offset").eval() {
                state.view_offset = view_offset;
            }

            if let Ok(style_handle) = lua.load("widgets.ram.style").eval::<Function>() {
                for row_index in 0..inner_area.height {
                    for value_index in 0..values_per_row {
                        let address = state.view_offset + values_per_row * row_index + value_index;
                        if let Ok(style) = style_handle.call::<_, LuaStyle>(address) {
                            state.style[usize::from(address)] = style.into();
                        }
                    }
                }
            }
        }

        if let Ok(emulator) = self.emulator.try_lock() {
            for row_index in 0..inner_area.height {
                for value_index in 0..values_per_row {
                    let address = state.view_offset + values_per_row * row_index + value_index;
                    state.ram[address] = emulator.ram[address];
                }
            }
        }

        // construct the lines which the widget displays
        let mut lines = Vec::new();
        for row_index in 0..inner_area.height {
            let mut line = vec![Span::raw(format!(
                "{:#06x}:",
                state.view_offset + values_per_row * row_index
            ))];
            for value_index in 0..values_per_row {
                line.push(Span::raw(" "));

                let address = state.view_offset + values_per_row * row_index + value_index;
                line.push(Span::styled(
                    format!("{:04x}", state.ram[address]),
                    state.style[usize::from(address)],
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
/// this widget is reponsible for rendering the contents of the general-purpose registers. the
/// widget can be styled via a provided lua function. when displaying the contents of a register,
/// the `style_handle' function will be called, with the index of the register provided as an
/// argument
pub struct RegistersWidget {
    lua: Arc<Mutex<Lua>>,
    emulator: Arc<Mutex<Emulator>>,
}

pub struct RegistersWidgetState {
    registers: Registers,
    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
    style: [Style; 32],
}

impl RegistersWidget {
    pub fn new(lua: Arc<Mutex<Lua>>, emulator: Arc<Mutex<Emulator>>) -> Self {
        Self { lua, emulator }
    }
}

impl Default for RegistersWidgetState {
    fn default() -> Self {
        Self {
            registers: Registers::default(),
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
            style: [Style::default(); 32],
        }
    }
}

impl RegistersWidgetState {
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

impl StatefulWidget for RegistersWidget {
    type State = RegistersWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("Registers")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);

        block.render(area, buf);

        let register_names_max_length = usize::from(state.minimum_width()) - 11;

        if let Ok(lua) = self.lua.try_lock() {
            if let Ok(visibility_bitmask) = lua.load("widgets.registers.visibility_bitmask").eval()
            {
                state.visibility_bitmask = visibility_bitmask;
            }

            if let Ok(aliases) = lua.load("widgets.registers.aliases").eval::<Table>() {
                for register_index in 0..32 {
                    state.aliases[register_index] = aliases.get(register_index).ok();
                }
            }

            if let Ok(style_handle) = lua.load("widgets.registers.style").eval::<Function>() {
                for register_index in 0..32 {
                    if let Ok(style) = style_handle.call::<_, LuaStyle>(register_index) {
                        state.style[register_index] = style.into();
                    }
                }
            }
        }

        if let Ok(emulator) = self.emulator.try_lock() {
            for register_index in 0..32 {
                state.registers[register_index] = emulator.registers[register_index];
            }
        }

        // Construct lines to be rendered for each of the visible registers.
        let mut lines = Vec::new();
        for i in 0u16..32 {
            if state.visibility_bitmask & (1 << i) != 0 {
                let mut line = Vec::new();

                let register_name = match &state.aliases[usize::from(i)] {
                    Some(a) => format!("{}", a),
                    None => format!("r{}", i),
                };
                line.push(Span::raw(format!(
                    "{:register_names_max_length$} ",
                    register_name
                )));

                line.push(Span::styled(
                    format!("{:#06x}", state.registers[i]),
                    state.style[usize::from(i)],
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
/// this widget is reponsible for rendering the contents of the control/status registers. the
/// widget can be styled via a provided lua function. when displaying the contents of a register,
/// the `style_handle' function will be called, with the index of the register provided as an
/// argument
pub struct ControlStatusRegistersWidget {
    lua: Arc<Mutex<Lua>>,
    emulator: Arc<Mutex<Emulator>>,
}

pub struct ControlStatusRegistersWidgetState {
    control_status_registers: ControlStatusRegisters,
    aliases: [Option<String>; 32],
    visibility_bitmask: u32,
    style: [Style; 32],
}

impl ControlStatusRegistersWidget {
    pub fn new(lua: Arc<Mutex<Lua>>, emulator: Arc<Mutex<Emulator>>) -> Self {
        Self { lua, emulator }
    }
}

impl Default for ControlStatusRegistersWidgetState {
    fn default() -> Self {
        Self {
            control_status_registers: ControlStatusRegisters::default(),
            aliases: [const { None }; 32],
            visibility_bitmask: 0xFFFFFFFF,
            style: [Style::default(); 32],
        }
    }
}

impl ControlStatusRegistersWidgetState {
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

impl StatefulWidget for ControlStatusRegistersWidget {
    type State = ControlStatusRegistersWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title("C/S Registers")
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);

        block.render(area, buf);

        let register_names_max_length = usize::from(state.minimum_width()) - 11;

        if let Ok(lua) = self.lua.try_lock() {
            if let Ok(visibility_bitmask) = lua
                .load("widgets.control_status_registers.visibility_bitmask")
                .eval()
            {
                state.visibility_bitmask = visibility_bitmask;
            }

            if let Ok(aliases) = lua
                .load("widgets.control_status_registers.aliases")
                .eval::<Table>()
            {
                for register_index in 0..32 {
                    match register_index {
                        0b10011..=0b10101 => continue,
                        _ => state.aliases[register_index] = aliases.get(register_index).ok(),
                    }
                }
            }

            if let Ok(style_handle) = lua
                .load("widgets.control_status_registers.style")
                .eval::<Function>()
            {
                for register_index in 0..32 {
                    match register_index {
                        0b10011..=0b10101 => continue,
                        _ => {
                            if let Ok(style) = style_handle.call::<_, LuaStyle>(register_index) {
                                state.style[register_index] = style.into();
                            }
                        }
                    }
                }
            }
        }

        if let Ok(emulator) = self.emulator.try_lock() {
            for register_index in 0..32 {
                match register_index {
                    0b10011..=0b10101 => continue,
                    _ => {
                        state.control_status_registers[register_index] =
                            emulator.control_status_registers[register_index]
                    }
                }
            }
        }

        let mut lines = Vec::new();
        for i in 0u16..32 {
            if state.visibility_bitmask & (1 << i) != 0 {
                let mut line = Vec::new();

                let register_name = match &state.aliases[usize::from(i)] {
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
                    format!("{:#06x}", state.control_status_registers[i]),
                    state.style[usize::from(i)],
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

    history_file: Option<File>,
    history: Vec<String>,
    history_index: usize,

    input_sender: Sender<String>,
    output_receiver: Receiver<String>,
}

#[derive(Default)]
pub struct PromptWidgetState {
    output_buffer: String,
}

impl PromptWidget<'_> {
    pub fn new(input_sender: Sender<String>, output_receiver: Receiver<String>) -> Self {
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
            history_file,
            history,
            history_index,

            input_sender,
            output_receiver,
        }
    }

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

    pub fn evaluate_input_buffer(&mut self) {
        let input_buffer = self.text_area.lines()[0].clone();

        self.input_sender.send(input_buffer.clone());

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

impl StatefulWidget for &PromptWidget<'_> {
    type State = PromptWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut PromptWidgetState) {
        if let Ok(output) = self.output_receiver.try_recv() {
            state.output_buffer = output;
        }

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
        Text::from(state.output_buffer.as_str()).render(output_area, buf);
        self.text_area.render(text_area, buf);
    }
}
