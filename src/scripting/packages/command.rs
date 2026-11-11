//! El catálogo y el minibuffer resuelven la misma invocación sin consultar archivos.
use super::{PackageError, PackageForm, PackagePhase, PackageRegistry, ToolInvocation};

#[derive(Debug)]
pub enum ToolRequest {
    Reload,
    Form(PackageForm),
    Invoke(ToolInvocation),
}

impl PackageRegistry {
    pub fn request(&self, command: &str) -> Option<Result<ToolRequest, PackageError>> {
        let name = command.split_whitespace().next()?;
        if !name.eq_ignore_ascii_case("midi.tool")
            && !name.eq_ignore_ascii_case("midi.tools.reload")
        {
            return None;
        }
        Some(self.resolve(command, name.eq_ignore_ascii_case("midi.tools.reload")))
    }

    fn resolve(&self, command: &str, reload: bool) -> Result<ToolRequest, PackageError> {
        let mut parts =
            crate::actions::tokenize(command).map_err(|e| request_error(None, "command", e))?;
        if reload {
            return if parts.len() == 1 {
                Ok(ToolRequest::Reload)
            } else {
                Err(request_error(None, "command", "Uso: midi.tools.reload"))
            };
        }
        if parts.len() < 2 {
            return Err(request_error(
                None,
                "id",
                "Uso: midi.tool <id> [valores...]",
            ));
        }
        let id = parts.remove(1);
        let package = self.get(&id).ok_or_else(|| {
            request_error(
                Some(&id),
                "id",
                "Herramienta no registrada; usa midi.tools.reload",
            )
        })?;
        if parts.len() == 1 && !package.descriptor.parameters.is_empty() {
            return Ok(ToolRequest::Form(
                self.form(&id).expect("paquete registrado"),
            ));
        }
        package.invoke(&parts[1..]).map(ToolRequest::Invoke)
    }
}

fn request_error(id: Option<&str>, field: &str, message: impl ToString) -> PackageError {
    PackageError {
        phase: PackagePhase::Arguments,
        path: "<registro>".into(),
        id: id.map(str::to_owned),
        field: Some(field.into()),
        message: message.to_string(),
    }
}
