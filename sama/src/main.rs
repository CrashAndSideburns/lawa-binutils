mod emulator;
mod lua;
mod ui;

use lua::LuaEmulator;
use ui::{ControlStatusRegistersWidget, PromptWidget, RamWidget, RegistersWidget};

use directories::ProjectDirs;

use mlua::Lua;

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
    lua.load(include_str!("init.lua")).exec();

    // load and execute user `init.lua' file
    if let Some(project_dirs) = ProjectDirs::from("", "", "sama") {
        let init_lua_path = project_dirs.config_dir().join("init.lua");
        if let Ok(init_lua) = read_to_string(init_lua_path) {
            lua.load(init_lua).exec();
        }
    }

    let mut prompt_widget = PromptWidget::from_lua(lua);

    loop {
        terminal.draw(|frame| {
            // TODO: the error handling in here is a mess right now. a strong gust of wind is
            // enough to make this panic

            // Fetch a (wrapped) handle to the emulator from the globals table.
            let emulator = prompt_widget
                .lua
                .globals()
                .get::<_, LuaEmulator>("emulator")
                .unwrap();

            // Create the widget for displaying the RAM.
            let ram = emulator.0.borrow().ram;
            let view_offset = prompt_widget
                .lua
                .load("widgets.ram.view_offset")
                .eval()
                .unwrap_or_default();
            let ram_widget = RamWidget::new(
                &ram,
                view_offset,
                prompt_widget.lua.load("widgets.ram.style").eval().unwrap(),
            );

            // Create the widget for displaying the registers.
            let registers = emulator.0.borrow().registers;
            let registers_widget = RegistersWidget::new(
                &registers,
                prompt_widget
                    .lua
                    .load("widgets.registers.style")
                    .eval()
                    .unwrap(),
            );

            // Create the widget for displaying the control/status registers.
            let control_status_registers = emulator.0.borrow().control_status_registers;
            let control_status_registers_widget = ControlStatusRegistersWidget::new(
                &control_status_registers,
                prompt_widget
                    .lua
                    .load("widgets.control_status_registers.style")
                    .eval()
                    .unwrap(),
            );

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
            // Exit gracefully on C-c.
            if key.kind == KeyEventKind::Press {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }
            }

            prompt_widget.process_event(key);
        }
    }
}
