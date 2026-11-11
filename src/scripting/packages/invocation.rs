//! Preparación sin E/S: argumentos tipados y formularios ligados a una generación.
use super::{Package, PackageError, PackagePhase, PackageRegistry};
use crate::actions::{ActionSpec, OwnedActionSpec, ParameterValue, ParameterValues};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ToolInvocation {
    pub(super) package: Arc<Package>,
    pub(super) values: ParameterValues<String>,
}

impl ToolInvocation {
    pub fn package(&self) -> &Arc<Package> {
        &self.package
    }
}

#[derive(Clone, Debug)]
pub struct PackageForm {
    package: Arc<Package>,
    generation: u64,
}

impl PackageForm {
    pub fn package(&self) -> &Arc<Package> {
        &self.package
    }

    pub fn validate_generation(&self, registry: &PackageRegistry) -> Result<(), PackageError> {
        if self.generation != registry.generation()
            || registry
                .get(&self.package.descriptor.id)
                .is_none_or(|current| !Arc::ptr_eq(current, &self.package))
        {
            return Err(self.package.error(
                PackagePhase::Stale,
                "generation",
                "El registro cambió; vuelve a abrir el formulario",
            ));
        }
        Ok(())
    }

    pub fn submit(
        &self,
        registry: &PackageRegistry,
        values: &ParameterValues<String>,
    ) -> Result<ToolInvocation, PackageError> {
        self.validate_generation(registry)?;
        self.package.prepare(values)
    }
}

impl PackageRegistry {
    pub fn form(&self, id: &str) -> Option<PackageForm> {
        self.get(id).map(|package| PackageForm {
            package: package.clone(),
            generation: self.generation(),
        })
    }
}

impl Package {
    pub fn action_spec(&self) -> OwnedActionSpec {
        ActionSpec {
            command: format!("midi.tool {}", self.descriptor.id),
            description: self.descriptor.description.clone(),
            parameters: self.descriptor.parameters.clone(),
        }
    }

    pub fn error(&self, phase: PackagePhase, field: &str, message: impl ToString) -> PackageError {
        PackageError {
            phase,
            path: self.path.clone(),
            id: Some(self.descriptor.id.clone()),
            field: Some(field.into()),
            message: message.to_string(),
        }
    }

    /// Una invocación explícita exige todos los valores, también en consumidores headless.
    pub fn invoke(self: &Arc<Self>, arguments: &[String]) -> Result<ToolInvocation, PackageError> {
        if arguments.len() != self.descriptor.parameters.len() {
            return Err(self.error(
                PackagePhase::Arguments,
                "parameters",
                format!(
                    "Se esperaban {} valores; se recibieron {}",
                    self.descriptor.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        let values = self
            .descriptor
            .parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, raw)| {
                parameter
                    .parse(raw)
                    .map(|value| (parameter.key.clone(), value))
                    .map_err(|e| self.error(PackagePhase::Arguments, &parameter.key, e))
            })
            .collect::<Result<_, _>>()?;
        self.prepare(&values)
    }

    fn prepare(
        self: &Arc<Self>,
        values: &ParameterValues<String>,
    ) -> Result<ToolInvocation, PackageError> {
        for key in values.keys() {
            if !self.descriptor.parameters.iter().any(|p| &p.key == key) {
                return Err(self.error(PackagePhase::Arguments, key, "Parámetro desconocido"));
            }
        }
        let mut checked = ParameterValues::new();
        for parameter in &self.descriptor.parameters {
            let error =
                |message: String| self.error(PackagePhase::Arguments, &parameter.key, message);
            let value = values
                .get(&parameter.key)
                .ok_or_else(|| error("Falta el parámetro".into()))?;
            let value = parameter.validate(value).map_err(error)?;
            if matches!(&value, ParameterValue::Text(text) if text.len() > 1024) {
                return Err(error("El texto supera 1024 bytes UTF-8".into()));
            }
            checked.insert(parameter.key.clone(), value);
        }
        Ok(ToolInvocation {
            package: self.clone(),
            values: checked,
        })
    }
}
