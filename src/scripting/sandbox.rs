//! VM acotada compartida por scripts MIDI y lectura de paquetes, siempre fuera de RT.
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, VmState};
use std::time::{Duration, Instant};

pub(super) const MAX_SOURCE_BYTES: usize = 256 * 1024;

pub(super) fn new() -> mlua::Result<Lua> {
    let lua = Lua::new_with(
        StdLib::MATH | StdLib::STRING | StdLib::TABLE,
        LuaOptions::default(),
    )?;
    // La biblioteca base se abre también con new_with. Retirar E/S, carga
    // dinámica y capturas de error que podrían anular el límite del hook.
    for name in [
        "dofile", "loadfile", "load", "require", "pcall", "xpcall", "print", "warn",
    ] {
        lua.globals().raw_set(name, mlua::Nil)?;
    }
    lua.set_memory_limit(8 * 1024 * 1024)?;
    let deadline = Instant::now() + Duration::from_millis(250);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1_000),
        move |_, _| {
            if Instant::now() >= deadline {
                Err(mlua::Error::runtime("El script Lua excedió 250 ms"))
            } else {
                Ok(VmState::Continue)
            }
        },
    )?;
    Ok(lua)
}
