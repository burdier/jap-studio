use super::{Package, PackagePhase, descriptor};
use crate::{
    musical::{model::Pattern, time::Tick},
    scripting::actions::MidiOptions,
};
use std::{path::Path, sync::Arc};

#[test]
fn changed_metadata_rejects_execution_before_calling_run() {
    let source = "return {api_version=1,id='user.unstable',name='Nuevo',description='Cambio',parameters={},run=function() error('no debe ejecutarse') end}";
    let path = Path::new("unstable.lua");
    let mut metadata = descriptor::read(source, path, false).unwrap();
    // Simula la diferencia que produce un descriptor no determinista al reevaluarlo.
    metadata.name = "Nombre registrado".into();
    let package = Arc::new(Package {
        path: path.into(),
        source: source.into(),
        descriptor: metadata,
    });
    let invocation = package.invoke(&[]).unwrap();
    let pattern = Pattern::new(Tick(960)).unwrap();
    let error = invocation
        .edit_midi(&pattern, &[], MidiOptions::default())
        .err()
        .unwrap();
    assert_eq!(error.phase, PackagePhase::Descriptor);
    assert_eq!(error.id.as_deref(), Some("user.unstable"));
    assert_eq!(error.field.as_deref(), Some("descriptor"));
    assert_eq!(error.path, path);
}
