use crate::emulator::{ControlStatusRegisters, Emulator, Ram, Registers};
use crate::lua::{LuaEmulator, LuaStyle};
use crate::ui::{ControlStatusRegistersWidget, PromptWidget, RamWidget, RegistersWidget};

use crossbeam::channel::{self, Receiver, Sender};

use directories::ProjectDirs;

use mlua::{Function, Lua, MultiValue, Table};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Widget,
    DefaultTerminal, Frame,
};

use tui_textarea::{CursorMove, TextArea};

use std::fs::{create_dir_all, read_to_string, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct App<'a> {
    lua: Arc<Mutex<Lua>>,
    emulator: Arc<Mutex<Emulator>>,

    ram: Ram,
    ram_view_offset: u16,
    ram_style: [Style; 0x10000],

    registers: Registers,
    registers_aliases: [Option<String>; 32],
    registers_visibility_bitmask: u32,
    registers_style: [Style; 32],

    control_status_registers: ControlStatusRegisters,
    control_status_registers_aliases: [Option<String>; 32],
    control_status_registers_visibility_bitmask: u32,
    control_status_registers_style: [Style; 32],

    text_area: TextArea<'a>,
    input_buffer: String,
    output_buffer: String,

    input_sender: Sender<String>,
    output_receiver: Receiver<String>,

    history_file: Option<File>,
    history: Vec<String>,
    history_index: usize,

    exit: bool,
}

impl Default for App<'_> {
    fn default() -> Self {
        // construct the global lua state and the emulator
        let lua = Lua::new();
        let emulator = Arc::new(Mutex::new(Emulator::default()));

        // set up the global lua state
        let _ = lua.globals().set("emulator", LuaEmulator(emulator.clone()));
        let _ = lua.load(include_str!("init.lua")).exec();

        // attempt to load and execute the user's init file, and fetch the command history
        if let Some(project_dirs) = ProjectDirs::from("", "", "sama") {
            let _ = create_dir_all(project_dirs.config_dir());
            let init_lua_path = project_dirs.config_dir().join("init.lua");
            if let Ok(init_lua) = read_to_string(init_lua_path) {
                let _ = lua.load(init_lua).exec();
            }
        }

        let history_file = ProjectDirs::from("", "", "sama").and_then(|project_dirs| {
            let _ = create_dir_all(project_dirs.data_dir());
            let history_path = project_dirs.data_dir().join("history");
            OpenOptions::new()
                .append(true)
                .create(true)
                .read(true)
                .open(history_path)
                .ok()
        });

        let history = match &history_file {
            Some(history_file) => BufReader::new(history_file)
                .lines()
                .filter_map(|l| l.ok())
                .collect(),
            None => Vec::new(),
        };

        let history_index = history.len();

        // wrap the global lua state in a mutex
        let lua = Arc::new(Mutex::new(lua));

        let (input_sender, input_receiver) = channel::unbounded();
        let (output_sender, output_receiver) = channel::unbounded();

        // spawn the lua evaluation thread
        let lua_handle = lua.clone();
        thread::spawn(move || {
            loop {
                // wait until we receive some input to evaluate, then evaluate it
                if let Ok(input_buffer) = input_receiver.recv() {
                    let output = match lua_handle
                        .lock()
                        .unwrap()
                        .load(&input_buffer)
                        .eval::<MultiValue>()
                    {
                        Ok(v) => {
                            format!(
                                "{}",
                                v.iter()
                                    .map(|value| format!("{:#?}", value))
                                    .collect::<Vec<_>>()
                                    .join("\t")
                            )
                        }
                        Err(e) => format!("{}", e),
                    };

                    let _ = output_sender.send(output);
                }
            }
        });

        // create the big bundle o' state
        Self {
            lua,
            emulator,

            ram: Ram::default(),
            ram_view_offset: 0,
            ram_style: [Style::default(); 0x10000],

            registers: Registers::default(),
            registers_aliases: [const { None }; 32],
            registers_visibility_bitmask: 0xFFFFFFFF,
            registers_style: [Style::default(); 32],

            control_status_registers: ControlStatusRegisters::default(),
            control_status_registers_aliases: [const { None }; 32],
            control_status_registers_visibility_bitmask: 0xFFFFFFFF,
            control_status_registers_style: [Style::default(); 32],

            text_area: TextArea::default(),
            input_buffer: String::new(),
            output_buffer: String::new(),

            input_sender,
            output_receiver,

            history_file,
            history,
            history_index,

            exit: false,
        }
    }
}

impl App<'_> {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            // FIXME: i should do a bit of math to get some rough upper bound on how large the
            // update needs to be, but i'm too lazy to do that right now, so 0x1000 it is
            self.try_update(0x1000);

            terminal.draw(|frame| self.draw(frame))?;

            if let Ok(true) = event::poll(Duration::from_secs_f64(1.0 / 30.0)) {
                if let Event::Key(key_event) = event::read()? {
                    self.handle_key_event(key_event);
                }
            }
        }

        Ok(())
    }

    fn try_update(&mut self, amount: u16) {
        // if we can access the global lua state, update all cached lua values
        if let Ok(lua) = self.lua.try_lock() {
            if let Ok(view_offset) = lua.load("widgets.ram.view_offset").eval() {
                self.ram_view_offset = view_offset;
            }

            if let Ok(style_handle) = lua.load("widgets.ram.style").eval::<Function>() {
                for i in 0..amount {
                    let address = self.ram_view_offset + i;
                    if let Ok(style) = style_handle.call::<_, LuaStyle>(address) {
                        self.ram_style[usize::from(address)] = style.into();
                    }
                }
            }

            if let Ok(visibility_bitmask) = lua.load("widgets.registers.visibility_bitmask").eval()
            {
                self.registers_visibility_bitmask = visibility_bitmask;
            }
            if let Ok(aliases) = lua.load("widgets.registers.aliases").eval::<Table>() {
                for i in 0..32 {
                    self.registers_aliases[i] = aliases.get(i).ok();
                }
            }
            if let Ok(style_handle) = lua.load("widgets.registers.style").eval::<Function>() {
                for i in 0..32 {
                    if let Ok(style) = style_handle.call::<_, LuaStyle>(i) {
                        self.registers_style[i] = style.into();
                    }
                }
            }

            if let Ok(visibility_bitmask) = lua
                .load("widgets.control_status_registers.visibility_bitmask")
                .eval()
            {
                self.control_status_registers_visibility_bitmask = visibility_bitmask;
            }
            if let Ok(aliases) = lua
                .load("widgets.control_status_registers.aliases")
                .eval::<Table>()
            {
                for i in 0..32 {
                    match i {
                        0b10011..=0b10101 => continue,
                        _ => self.control_status_registers_aliases[i] = aliases.get(i).ok(),
                    }
                }
            }
            if let Ok(style_handle) = lua
                .load("widgets.control_status_registers.style")
                .eval::<Function>()
            {
                for i in 0..32 {
                    match i {
                        0b10011..=0b10101 => continue,
                        _ => {
                            if let Ok(style) = style_handle.call::<_, LuaStyle>(i) {
                                self.control_status_registers_style[i] = style.into();
                            }
                        }
                    }
                }
            }
        }

        // if we can access the emulator, update all cached emulator state
        if let Ok(emulator) = self.emulator.try_lock() {
            for i in 0..amount {
                let address = self.ram_view_offset + i;
                self.ram[address] = emulator.ram[address];
            }
            self.registers = emulator.registers;
            self.control_status_registers = emulator.control_status_registers;
        }

        // fetch any output from the lua interpreter
        if let Ok(output) = self.output_receiver.try_recv() {
            self.output_buffer = output;
        }
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.exit = true;
                }
                KeyCode::Up => {
                    self.history_previous();
                }
                KeyCode::Char('p') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.history_previous();
                }
                KeyCode::Down => {
                    self.history_next();
                }
                KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.history_next();
                }
                KeyCode::Enter => {
                    self.evaluate_input();
                }
                KeyCode::Char('m') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.evaluate_input();
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

    fn history_next(&mut self) {
        if self.history_index < self.history.len() {
            self.history_index += 1;
            self.text_area = if self.history_index == self.history.len() {
                TextArea::new(vec![self.input_buffer.clone()])
            } else {
                TextArea::new(vec![self.history[self.history_index].clone()])
            };
            self.text_area.move_cursor(CursorMove::End);
        }
    }

    fn history_previous(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.text_area = TextArea::new(vec![self.history[self.history_index].clone()]);
            self.text_area.move_cursor(CursorMove::End);
        }
    }

    fn evaluate_input(&mut self) {
        let input = self.text_area.lines()[0].clone();

        // if we evaluate an empty buffer, don't pollute the history with blank lines
        if !input.is_empty() {
            if let Some(file) = &mut self.history_file {
                let _ = writeln!(file, "{}", &input);
            }
            self.history.push(input.clone());
            self.history_index = self.history.len();
        }

        // send the input to be evaluated
        let _ = self.input_sender.send(input);

        // reset the input buffer and the text area
        self.input_buffer = String::new();
        self.text_area = TextArea::default();
    }
}

impl Widget for &App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // create all of the widgets
        let ram_widget = RamWidget::new(&self.ram, self.ram_view_offset, &self.ram_style);
        let registers_widget = RegistersWidget::new(
            &self.registers,
            &self.registers_aliases,
            self.registers_visibility_bitmask,
            &self.registers_style,
        );
        let control_status_registers_widget = ControlStatusRegistersWidget::new(
            &self.control_status_registers,
            &self.control_status_registers_aliases,
            self.control_status_registers_visibility_bitmask,
            &self.control_status_registers_style,
        );
        let prompt_widget = PromptWidget::new(&self.text_area, &self.output_buffer);

        // compute the areas in which the various widgets should be rendered
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Fill(0), Constraint::Max(4)])
            .split(area);

        let prompt_area = split[1];

        let registers_widgets_width = registers_widget
            .minimum_width()
            .max(control_status_registers_widget.minimum_width());
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Fill(0),
                Constraint::Max(registers_widgets_width),
            ])
            .split(split[0]);

        let ram_area = split[0];

        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Max(registers_widget.minimum_height()),
                Constraint::Max(control_status_registers_widget.minimum_height()),
            ])
            .split(split[1]);

        let registers_area = split[0];
        let control_status_registers_area = split[1];

        // render the widgets
        ram_widget.render(ram_area, buf);
        registers_widget.render(registers_area, buf);
        control_status_registers_widget.render(control_status_registers_area, buf);
        prompt_widget.render(prompt_area, buf);
    }
}
