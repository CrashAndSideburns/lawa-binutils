use crate::emulator::Emulator;

use mlua::{
    Error::FromLuaConversionError, FromLua, Lua, MetaMethod, Result, Table, UserData,
    UserDataFields, UserDataMethods, Value,
};

use ratatui::style::{Color, Modifier, Style};

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

/// a wrapper to allow lua interop for emulator::Emulator
#[derive(Default)]
pub struct LuaEmulator(pub Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for emulator::Ram
struct LuaRam(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for emulator::Registers
struct LuaRegisters(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for emulator::ControlStatusRegisters
struct LuaControlStatusRegisters(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for the interrupt mask control/status registers
struct LuaInterruptMaskRegisters(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for the memory protection control control/status registers
struct LuaMemoryProtectionControlRegisters(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for the memory protection address control/status registers
struct LuaMemoryProtectionAddressRegisters(Rc<RefCell<Emulator>>);

/// a wrapper to allow lua interop for ratatui::style::Style
#[derive(Default)]
pub struct LuaStyle(Style);

impl FromLua<'_> for LuaEmulator {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => Ok(LuaEmulator(ud.borrow::<Self>()?.0.clone())),
            _ => unreachable!(),
        }
    }
}

impl UserData for LuaEmulator {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("ram", |_, this| Ok(LuaRam(this.0.clone())));
        fields.add_field_method_get("registers", |_, this| Ok(LuaRegisters(this.0.clone())));
        fields.add_field_method_get("control_status_registers", |_, this| {
            Ok(LuaControlStatusRegisters(this.0.clone()))
        });
        fields.add_field_method_get("program_counter", |_, this| {
            Ok(this.0.borrow().program_counter)
        });
        fields.add_field_method_set("program_counter", |_, this, value: u16| {
            this.0.borrow_mut().program_counter = value;
            Ok(())
        });
    }

    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("step", |_, this, ()| {
            this.0.borrow_mut().step();
            Ok(())
        });
    }
}

impl UserData for LuaRam {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| &e.ram)[index])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| &mut e.ram)[index] = value;
                Ok(())
            },
        );
    }
}

impl UserData for LuaRegisters {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| &e.registers)[index])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| &mut e.registers)[index] =
                    value;
                Ok(())
            },
        );
    }
}

impl UserData for LuaControlStatusRegisters {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("im", |_, this| {
            Ok(LuaInterruptMaskRegisters(this.0.clone()))
        });
        fields.add_field_method_get("iv", |_, this| {
            Ok(this.0.borrow().control_status_registers.iv)
        });
        fields.add_field_method_set("iv", |_, this, value: u16| {
            this.0.borrow_mut().control_status_registers.iv = value;
            Ok(())
        });
        fields.add_field_method_get("ipc", |_, this| {
            Ok(this.0.borrow().control_status_registers.ipc)
        });
        fields.add_field_method_set("ipc", |_, this, value: u16| {
            this.0.borrow_mut().control_status_registers.ipc = value;
            Ok(())
        });
        fields.add_field_method_get("ic", |_, this| {
            Ok(this.0.borrow().control_status_registers.ic)
        });
        fields.add_field_method_set("ic", |_, this, value: u16| {
            this.0.borrow_mut().control_status_registers.ic = value;
            Ok(())
        });
        fields.add_field_method_get("mpc", |_, this| {
            Ok(LuaMemoryProtectionControlRegisters(this.0.clone()))
        });
        fields.add_field_method_get("mpa", |_, this| {
            Ok(LuaMemoryProtectionAddressRegisters(this.0.clone()))
        });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| &e.control_status_registers)[index])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| {
                    &mut e.control_status_registers
                })[index] = value;
                Ok(())
            },
        );
    }
}

impl UserData for LuaInterruptMaskRegisters {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| {
                &e.control_status_registers.im
            })[usize::from(index)])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| {
                    &mut e.control_status_registers.im
                })[usize::from(index)] = value;
                Ok(())
            },
        );
    }
}

impl UserData for LuaMemoryProtectionControlRegisters {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| {
                &e.control_status_registers.mpc
            })[usize::from(index)])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| {
                    &mut e.control_status_registers.mpc
                })[usize::from(index)] = value;
                Ok(())
            },
        );
    }
}

impl UserData for LuaMemoryProtectionAddressRegisters {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, index: u16| {
            Ok(Ref::map(this.0.borrow(), |e: &Emulator| {
                &e.control_status_registers.mpa
            })[usize::from(index)])
        });

        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (index, value): (u16, u16)| {
                RefMut::map(this.0.borrow_mut(), |e: &mut Emulator| {
                    &mut e.control_status_registers.mpa
                })[usize::from(index)] = value;
                Ok(())
            },
        );
    }
}

impl Into<Style> for LuaStyle {
    fn into(self) -> Style {
        self.0
    }
}

impl FromLua<'_> for LuaStyle {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        match value {
            Value::Table(t) => {
                let mut style = Style::default();

                style.fg = t
                    .get::<_, Table>("fg")
                    .and_then(|t| {
                        Ok(Some(Color::Rgb(
                            t.get("r").unwrap_or(0xFF),
                            t.get("g").unwrap_or(0xFF),
                            t.get("b").unwrap_or(0xFF),
                        )))
                    })
                    .unwrap_or_default();

                style.bg = t
                    .get::<_, Table>("bg")
                    .and_then(|t| {
                        Ok(Some(Color::Rgb(
                            t.get("r").unwrap_or_default(),
                            t.get("g").unwrap_or_default(),
                            t.get("b").unwrap_or_default(),
                        )))
                    })
                    .unwrap_or_default();

                // NOTE: i am undecided on whether or not i hate breaking parity with the field
                // names from ratatui::style::Style more than i hate american english
                style.underline_color = t
                    .get::<_, Table>("underline_color")
                    .and_then(|t| {
                        Ok(Some(Color::Rgb(
                            t.get("r").unwrap_or_default(),
                            t.get("g").unwrap_or_default(),
                            t.get("b").unwrap_or_default(),
                        )))
                    })
                    .unwrap_or_default();

                if t.get::<_, bool>("bold").unwrap_or_default() {
                    style = style.add_modifier(Modifier::BOLD);
                }

                if t.get::<_, bool>("dim").unwrap_or_default() {
                    style = style.add_modifier(Modifier::DIM);
                }

                if t.get::<_, bool>("italic").unwrap_or_default() {
                    style = style.add_modifier(Modifier::ITALIC);
                }

                if t.get::<_, bool>("underlined").unwrap_or_default() {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }

                if t.get::<_, bool>("slow_blink").unwrap_or_default() {
                    style = style.add_modifier(Modifier::SLOW_BLINK);
                }

                if t.get::<_, bool>("rapid_blink").unwrap_or_default() {
                    style = style.add_modifier(Modifier::RAPID_BLINK);
                }

                if t.get::<_, bool>("reversed").unwrap_or_default() {
                    style = style.add_modifier(Modifier::REVERSED);
                }

                if t.get::<_, bool>("hidden").unwrap_or_default() {
                    style = style.add_modifier(Modifier::HIDDEN);
                }

                if t.get::<_, bool>("crossed_out").unwrap_or_default() {
                    style = style.add_modifier(Modifier::CROSSED_OUT);
                }

                Ok(LuaStyle(style))
            }
            other => Err(FromLuaConversionError {
                from: other.type_name(),
                to: "LuaStyle",
                message: None,
            }),
        }
    }
}
