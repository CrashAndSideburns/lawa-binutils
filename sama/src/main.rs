mod devices;
mod emulator;
mod lua;
mod ui;

use lua::LuaEmulator;
use ui::{ControlStatusRegistersWidget, PromptWidget, RamWidget, RegistersWidget};

use directories::ProjectDirs;

use mlua::{Function, Lua, MultiValue};

use ratatui::{
    crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    DefaultTerminal,
};

use std::fs::read_to_string;
use std::io;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let app_result = run(terminal);
    ratatui::restore();
    app_result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    // Initialize the Lua state by sending the emulator over, as well as running the initialization
    // code.
    let lua = Lua::new();
    lua.globals().set("emulator", LuaEmulator::default());

    // attach a callback function to allow reloading configuration dynamically
    lua.globals().set(
        "reload_configuration",
        lua.create_function(|lua, _: MultiValue| {
            lua.load(include_str!("init.lua")).exec();

            // load and execute user `init.lua' file
            if let Some(project_dirs) = ProjectDirs::from("", "", "sama") {
                let init_lua_path = project_dirs.config_dir().join("init.lua");
                if let Ok(init_lua) = read_to_string(init_lua_path) {
                    lua.load(init_lua).exec();
                }
            }

            Ok(())
        })
        .unwrap(),
    );

    lua.load(include_str!("init.lua")).exec();

    // load and execute user `init.lua' file
    if let Some(project_dirs) = ProjectDirs::from("", "", "sama") {
        let init_lua_path = project_dirs.config_dir().join("init.lua");
        if let Ok(init_lua) = read_to_string(init_lua_path) {
            lua.load(init_lua).exec().unwrap();
        }
    }

    let mut prompt_widget = PromptWidget::default();

    loop {
        terminal.draw(|frame| {
            let globals = lua.globals();

            // fetch a handle to the emulator from the globals table. if, for some reason, we can't
            // fetch the emulator, something has gone wrong, so construct a new emulator and place
            // it in the globals table instead of crashing.
            let emulator = match globals.get::<_, LuaEmulator>("emulator") {
                Ok(emulator) => emulator,
                Err(_) => {
                    // something happened to the emulator! we need to replace it with a new one.
                    let emulator = LuaEmulator::default();
                    let emulator_handle = LuaEmulator(emulator.0.clone());
                    lua.globals().set("emulator", emulator);
                    emulator_handle
                }
            };
            let emulator = emulator.0.borrow();

            // create all of the widgets
            let ram_widget = RamWidget::new(&emulator, &lua);
            let registers_widget = RegistersWidget::new(&emulator, &lua);
            let control_status_registers_widget =
                ControlStatusRegistersWidget::new(&emulator, &lua);

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
            frame.render_widget(&prompt_widget, prompt_area);
        })?;

        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }
                    KeyCode::Enter => {
                        prompt_widget.evaluate_input_buffer(&lua);
                    }
                    KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        prompt_widget.evaluate_input_buffer(&lua);
                    }
                    _ => {
                        prompt_widget.process_key_event(key);
                    }
                }
            }
        }
    }
}
