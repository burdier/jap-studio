//! Registro de sesión y recarga explícita en un worker; sin escanear durante render.
use super::DawApp;
use elevate::scripting::packages::{PackageDirectories, PackageRegistry, ToolRequest};
use gpui::{AppContext, Context, Task, Window};

#[derive(Default)]
pub(super) struct LuaTools {
    pub registry: PackageRegistry,
    reload_task: Option<Task<()>>,
}

impl LuaTools {
    pub fn new() -> Self {
        let directories = PackageDirectories {
            bundled: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/scripts/tools/midi"),
            user: dirs::config_dir().map(|root| root.join("hay-daw/tools/midi")),
        };
        let mut registry = PackageRegistry::default();
        if registry.reload(&directories).is_err() {
            let bundled_only = PackageDirectories {
                bundled: directories.bundled,
                user: None,
            };
            let _ = registry.reload(&bundled_only);
        }
        Self {
            registry,
            reload_task: None,
        }
    }
}

impl DawApp {
    pub(super) fn execute_tool_command(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(request) = self.lua_tools.registry.request(command) else {
            return false;
        };
        match request {
            Ok(ToolRequest::Reload) => self.reload_lua_tools(cx),
            Ok(ToolRequest::Form(form)) => self.open_tool_action_form(form, window, cx),
            Ok(ToolRequest::Invoke(invocation)) => self.start_tool_invocation(invocation, cx),
            Err(error) => self.status_message = error.to_string(),
        }
        cx.notify();
        true
    }

    fn reload_lua_tools(&mut self, cx: &mut Context<Self>) {
        if self.lua_tools.reload_task.is_some() {
            self.status_message = "Lua: recarga en curso".into();
            return;
        }
        let directories = PackageDirectories {
            bundled: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/scripts/tools/midi"),
            user: dirs::config_dir().map(|root| root.join("hay-daw/tools/midi")),
        };
        let no_user_directory = directories.user.is_none();
        let mut candidate = self.lua_tools.registry.clone();
        self.status_message = "Recargando herramientas Lua…".into();
        let work =
            cx.background_spawn(async move { candidate.reload(&directories).map(|()| candidate) });
        self.lua_tools.reload_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.lua_tools.reload_task = None;
                match result {
                    Ok(registry) => {
                        let count = registry.packages().len();
                        this.palette.replace_tools(elevate::catalog::tools::entries(&registry));
                        this.lua_tools.registry = registry;
                        this.cmd_selected = None;
                        this.status_message = format!("Lua · {count} herramientas registradas");
                    }
                    Err(error) => this.status_message = format!("{error} · Registro anterior conservado"),
                }
                if no_user_directory {
                    this.status_message.push_str(" · No hay directorio de configuración de usuario; solo se usa la raíz distribuida");
                }
                this.sync_windows(cx);
                cx.notify();
            });
        }));
    }
}
