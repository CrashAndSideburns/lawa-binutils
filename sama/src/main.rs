mod devices;
mod emulator;
mod lua;
mod ui;

use lua::LuaEmulator;
use ui::{
    ControlStatusRegistersWidget, ControlStatusRegistersWidgetState, PromptWidget,
    PromptWidgetState, RamWidget, RamWidgetState, RegistersWidget, RegistersWidgetState,
};

use crossbeam::channel;

use directories::ProjectDirs;

use mlua::{Lua, MultiValue};

use ratatui::{
    crossterm::event::{self, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    DefaultTerminal,
};

use std::fs::read_to_string;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let app_result = run(terminal);
    ratatui::restore();
    app_result
}

fn run(mut terminal: DefaultTerminal) -> io::Result<()> {
    // begin by initialising the global lua state
    let lua = Lua::new();

    let emulator = LuaEmulator::default();
    let emulator_handle = emulator.0.clone();
    lua.globals().set("emulator", emulator);
    lua.load(include_str!("init.lua")).exec();

    // load and execute user `init.lua' file
    if let Some(project_dirs) = ProjectDirs::from("", "", "sama") {
        let init_lua_path = project_dirs.config_dir().join("init.lua");
        if let Ok(init_lua) = read_to_string(init_lua_path) {
            lua.load(init_lua).exec().unwrap();
        }
    }

    // wrap the global lua state in a mutex so that it can be shared across threads
    let lua = Arc::new(Mutex::new(lua));

    let (input_sender, input_receiver) = channel::unbounded();
    let (output_sender, output_receiver) = channel::unbounded();

    let lua_handle = lua.clone();
    thread::spawn(move || {
        loop {
            // wait until we receive some input to evaluate it, then evaluate it
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

                output_sender.send(output);
            }
        }
    });

    let mut prompt_widget = PromptWidget::new(input_sender, output_receiver);
    let mut prompt_widget_state = PromptWidgetState::default();

    let mut ram_widget_state = RamWidgetState::default();
    let mut registers_widget_state = RegistersWidgetState::default();
    let mut control_status_registers_widget_state = ControlStatusRegistersWidgetState::default();

    loop {
        terminal.draw(|frame| {
            // create all of the widgets
            let ram_widget = RamWidget::new(lua.clone(), emulator_handle.clone());
            let registers_widget = RegistersWidget::new(lua.clone(), emulator_handle.clone());
            let control_status_registers_widget =
                ControlStatusRegistersWidget::new(lua.clone(), emulator_handle.clone());

            // compute the areas in which the various widgets should be rendered
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Fill(0), Constraint::Max(4)])
                .split(frame.area());

            let prompt_area = split[1];

            let registers_widgets_width = registers_widget_state
                .minimum_width()
                .max(control_status_registers_widget_state.minimum_width());
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
                    Constraint::Max(registers_widget_state.minimum_height()),
                    Constraint::Max(control_status_registers_widget_state.minimum_height()),
                ])
                .split(split[1]);

            let registers_area = split[0];
            let control_status_registers_area = split[1];

            // Render the widgets.
            frame.render_stateful_widget(ram_widget, ram_area, &mut ram_widget_state);
            frame.render_stateful_widget(
                registers_widget,
                registers_area,
                &mut registers_widget_state,
            );
            frame.render_stateful_widget(
                control_status_registers_widget,
                control_status_registers_area,
                &mut control_status_registers_widget_state,
            );
            frame.render_stateful_widget(&prompt_widget, prompt_area, &mut prompt_widget_state);
        })?;

        // NOTE: this effectively caps the framerate of the tui at 30fps, which is entirely
        // arbitrary, but probably sufficient
        if let Ok(true) = event::poll(Duration::from_secs_f64(1.0/30.0)) {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(())
                        }
                        KeyCode::Enter => {
                            prompt_widget.evaluate_input_buffer();
                        }
                        KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            prompt_widget.evaluate_input_buffer();
                        }
                        _ => {
                            prompt_widget.process_key_event(key);
                        }
                    }
                }
            }
        }
    }
}
