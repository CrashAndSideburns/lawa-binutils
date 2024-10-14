mod emulator;
mod lua;
mod ui;

use lua::LuaEmulator;
use ui::{ControlStatusRegistersWidget, PromptWidget, RamWidget, RegistersWidget};

use mlua::{Lua, MultiValue};

use ratatui::{
    crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers},
    prelude::{Constraint, Direction, Layout},
    DefaultTerminal,
};

use std::io;

struct App {
    history: Vec<String>,
    history_index: usize,
    input_buffer: String,
    output_buffer: String,
}

impl App {
    fn new() -> Self {
        Self {
            history: Vec::new(),
            history_index: 0,
            input_buffer: String::new(),
            output_buffer: String::new(),
        }
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let app_result = run(terminal);
    ratatui::restore();
    app_result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();

    // Initialize the Lua state by sending the emulator over, as well as running the initialization
    // code.
    let mut lua = Lua::new();
    lua.globals().set("emulator", LuaEmulator::default());
    lua.load(include_str!("init.lua")).exec();

    loop {
        terminal.draw(|frame| {
            // Fetch a (wrapped) handle to the emulator from the globals table.
            let emulator: LuaEmulator = lua.globals().get("emulator").unwrap();

            // Create the widget for displaying the RAM.
            // TODO: Add proper error handling in the case where something goes wrong with
            // evaluating `widgets.ram.style`. In this case, default styling should be applied.
            let ram = emulator.0.borrow().ram;
            let view_offset = lua
                .load("widgets.ram.view_offset")
                .eval()
                .unwrap_or_default();
            let ram_widget = RamWidget::new(
                &ram,
                view_offset,
                lua.load("widgets.ram.style").eval().unwrap(),
            );

            // Create the widget for displaying the registers.
            let registers = emulator.0.borrow().registers;
            let registers_widget = RegistersWidget::new(
                &registers,
                lua.load("widgets.registers.style").eval().unwrap(),
            );

            // Create the widget for displaying the control/status registers.
            let control_status_registers = emulator.0.borrow().control_status_registers;
            let control_status_registers_widget =
                ControlStatusRegistersWidget::new(&control_status_registers, lua.load("widgets.control_status_registers.style").eval().unwrap());

            // Compute the areas in which the various widgets should be rendered.
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Fill(0), Constraint::Max(4)])
                .split(frame.area());

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

            // Render the widgets.
            frame.render_widget(ram_widget, ram_area);
            frame.render_widget(registers_widget, registers_area);
            frame.render_widget(
                control_status_registers_widget,
                control_status_registers_area,
            );

            // FIXME: The current handling of the prompt is quite hacky.
            let input = if app.history_index == app.history.len() {
                &app.input_buffer
            } else {
                &app.history[app.history_index]
            };
            let prompt = PromptWidget::new(input, &app.output_buffer);

            frame.render_widget(prompt, prompt_area);
        })?;

        // FIXME: This is some super janky input handling. I might want to switch to something like
        // `readline` in the future.
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                // Exit gracefully on C-c.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }

                match key.code {
                    KeyCode::Char(c) => {
                        if app.history_index != app.history.len() {
                            app.input_buffer = app.history[app.history_index].clone();
                            app.history_index = app.history.len();
                        }

                        app.input_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        if app.history_index != app.history.len() {
                            app.input_buffer = app.history[app.history_index].clone();
                            app.history_index = app.history.len();
                        }

                        app.input_buffer.pop();
                    }
                    KeyCode::Enter => {
                        if app.history_index != app.history.len() {
                            app.input_buffer = app.history[app.history_index].clone();
                            app.history_index = app.history.len();
                        }

                        // FIXME: This needs to be actually looked at. This is just a REPL that I
                        // copied from an example in the `mlua` repository.
                        app.output_buffer = match lua.load(&app.input_buffer).eval::<MultiValue>() {
                            Ok(v) => {
                                format!(
                                    "{}",
                                    v.iter()
                                        .map(|value| format!("{:#?}", value))
                                        .collect::<Vec<_>>()
                                        .join("\t")
                                )
                            }
                            Err(e) => {
                                format!("{}", e)
                            }
                        };

                        app.history.push(app.input_buffer.clone());
                        app.history_index += 1;
                        app.input_buffer.clear();
                    }
                    KeyCode::Up => {
                        if app.history_index > 0 {
                            app.history_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if app.history_index < app.history.len() {
                            app.history_index += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
