use super::{defaults, fixtures::NUMBER, invoke, pattern, registry};
use elevate::{
    actions::{self, ParameterValue, ParameterValues},
    scripting::{
        actions::MidiOptions,
        packages::{PackagePhase, ToolRequest},
    },
};

const EXTRA: &str = ", {key='text',label='Texto',kind='text',default='Nota'}, {key='enabled',label='Activo',kind='bool',default=true}";

#[test]
fn form_and_command_share_typed_values_without_source_interpolation() {
    let body = "assert(type(params.amount)=='number' and type(params.enabled)=='boolean'); assert(params.text == [[a'; error('inyectado') --]]); if params.enabled then for _,n in ipairs(context.notes) do if n.selected then n.velocity=n.velocity+params.amount end end end";
    let (_fixture, registry) = registry(body, &format!("{NUMBER}{EXTRA}"));
    let mut values = defaults(&registry);
    values.insert(
        "text".into(),
        ParameterValue::Text("a'; error('inyectado') --".into()),
    );
    let tool = registry.get("user.midi.accent").unwrap();
    let command = tool.action_spec().command_with(&values).unwrap();
    let from_command = invoke(&registry, &command);
    let ToolRequest::Form(form) = registry
        .request("midi.tool user.midi.accent")
        .unwrap()
        .unwrap()
    else {
        panic!()
    };
    let from_form = form.submit(&registry, &values).unwrap();
    let original = pattern();
    let selection = [original.notes()[0].id];
    let a = from_form
        .edit_midi(&original, &selection, MidiOptions::default())
        .unwrap();
    let b = from_command
        .edit_midi(&original, &selection, MidiOptions::default())
        .unwrap();
    assert_eq!(a.pattern, b.pattern);
    assert_eq!(a.pattern.notes()[0].velocity, 102);
    assert_eq!(a.pattern.notes()[1], original.notes()[1]);
    assert_eq!(a.pattern.controls(), original.controls());
}

#[test]
fn both_paths_reject_out_of_range_and_overlong_text() {
    let (_fixture, registry) = registry("", &format!("{NUMBER}{EXTRA}"));
    let form = registry.form("user.midi.accent").unwrap();
    for (key, value) in [
        ("amount", ParameterValue::Number(127.0)),
        ("amount", ParameterValue::Number(1.5)),
        ("amount", ParameterValue::Number(f64::NAN)),
        ("amount", ParameterValue::Bool(true)),
        ("enabled", ParameterValue::Text("true".into())),
        ("text", ParameterValue::Text("é".repeat(513))),
        ("text", ParameterValue::Text("".into())),
        ("text", ParameterValue::Text("a\nb".into())),
    ] {
        let mut values = defaults(&registry);
        values.insert(key.into(), value);
        let error = form.submit(&registry, &values).unwrap_err();
        assert_eq!(error.phase, PackagePhase::Arguments);
        assert_eq!(error.field.as_deref(), Some(key));
        assert_eq!(error.id.as_deref(), Some("user.midi.accent"));
    }
    for args in [
        "127 Nota true".to_string(),
        "1.5 Nota true".into(),
        "12 Nota 1".into(),
        format!("12 '{}' true", "é".repeat(513)),
    ] {
        assert!(
            registry
                .request(&format!("midi.tool user.midi.accent {args}"))
                .unwrap()
                .is_err()
        );
    }
    let mut values = defaults(&registry);
    values.remove("text");
    assert!(form.submit(&registry, &values).is_err());
    let mut values = defaults(&registry);
    values.insert("unknown".into(), ParameterValue::Bool(false));
    assert!(form.submit(&registry, &values).is_err());
}

#[test]
fn command_resolution_handles_reload_forms_exact_arity_and_unknown_ids() {
    let (_fixture, registry) = registry("", NUMBER);
    assert!(matches!(
        registry.request("MIDI.TOOLS.RELOAD").unwrap().unwrap(),
        ToolRequest::Reload
    ));
    assert!(matches!(
        registry
            .request("midi.tool user.midi.accent")
            .unwrap()
            .unwrap(),
        ToolRequest::Form(_)
    ));
    assert!(matches!(
        registry
            .request("midi.tool user.midi.accent 12")
            .unwrap()
            .unwrap(),
        ToolRequest::Invoke(_)
    ));
    for command in [
        "midi.tools.reload extra",
        "midi.tool",
        "midi.tool user.missing",
        "midi.tool hay.midi.accent",
        "midi.tool user.midi.accent 12 13",
        "midi.tool 'bad quote",
    ] {
        assert_eq!(
            registry.request(command).unwrap().unwrap_err().phase,
            PackagePhase::Arguments,
            "{command}"
        );
    }
    assert!(registry.request("midi.script.accent").is_none());
    assert!(registry.request("midi.tools.other").is_none());
    let (_fixture, empty) = super::registry("assert(next(params)==nil)", "");
    assert!(matches!(
        empty
            .request("midi.tool user.midi.accent")
            .unwrap()
            .unwrap(),
        ToolRequest::Invoke(_)
    ));
}

#[test]
fn owned_native_specs_keep_validation_quoting_flags_and_switches() {
    let native = actions::find("fx.rename").unwrap();
    let owned = native.to_owned();
    let text = "Nombre con 'comillas' y \\";
    let values = ParameterValues::from([("name", ParameterValue::Text(text.into()))]);
    assert_eq!(
        owned.command_with(&values).unwrap(),
        native.command_with(&values).unwrap()
    );
    assert_eq!(
        actions::tokenize(&owned.command_with(&values).unwrap()).unwrap()[1],
        text
    );
    let export = actions::find("export").unwrap();
    let values: ParameterValues = export
        .parameters
        .iter()
        .map(|p| (p.key, p.parse(p.default).unwrap()))
        .collect();
    assert_eq!(
        export.command_with(&values).unwrap(),
        export.to_owned().command_with(&values).unwrap()
    );
    let spec = actions::find("utility.gain").unwrap().to_owned();
    assert!(
        spec.command_with(&ParameterValues::from([(
            "gain",
            ParameterValue::Number(3.0)
        )]))
        .is_err()
    );
}
