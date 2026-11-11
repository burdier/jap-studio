use super::support::{Fixture, NUMBER, altered, source};
use elevate::scripting::packages::PackagePhase;

fn rejects(body: &str, field: &str) {
    let fixture = Fixture::new();
    let path = fixture.write("bad.lua", altered(body));
    let error = fixture.load().unwrap_err();
    assert_eq!(error.phase, PackagePhase::Descriptor, "{body}: {error}");
    assert_eq!(error.path, path);
    assert!(error.id.is_some(), "{body}");
    assert_eq!(error.field.as_deref(), Some(field), "{body}: {error}");
}

#[test]
fn schema_rejects_missing_unknown_and_coerced_fields() {
    for (body, field) in [
        ("p.api_version=nil", "api_version"),
        ("p.api_version='1'", "api_version"),
        ("p.api_version=2", "api_version"),
        ("p.api_version=1.5", "api_version"),
        ("p.api_version=0/0", "api_version"),
        ("p.name=nil", "name"),
        ("p.name=123", "name"),
        ("p.description=false", "description"),
        ("p.extra=true", "extra"),
        ("p.run=nil", "run"),
        ("p.run={}", "run"),
        ("p.run=math.sin", "run"),
        ("p.parameters=nil", "parameters"),
        ("p.parameters=false", "parameters"),
    ] {
        rejects(body, field);
    }
}

#[test]
fn identifiers_and_reserved_namespace_are_enforced() {
    for id in [
        "hay.user",
        "a",
        "abc",
        "a.",
        ".a",
        "a..b",
        "User.name",
        "user.9x",
        "user.bad-key",
        "user.ñ",
        "user. ",
    ] {
        rejects(&format!("p.id='{id}'"), "id");
    }
    rejects("p.id=string.rep('a',64)..'.b'", "id");
    let fixture = Fixture::new();
    fixture.write("ok.lua", source("u.a_1", ""));
    assert!(fixture.load().is_ok());
}

#[test]
fn descriptive_text_limits_measure_utf8_bytes_and_reject_blank_or_multiline() {
    for (body, field) in [
        ("p.name=''", "name"),
        ("p.name='  '", "name"),
        ("p.name='a\\nb'", "name"),
        ("p.name='a\\rb'", "name"),
        ("p.name='a\\0b'", "name"),
        ("p.name=string.char(255)", "name"),
        ("p.name=string.rep('é',65)", "name"),
        ("p.description=string.rep('x',513)", "description"),
        (
            "p.parameters[1].label=string.rep('x',129)",
            "parameters[1].label",
        ),
        (
            "p.parameters[1].hint=string.rep('x',257)",
            "parameters[1].hint",
        ),
        (
            "p.parameters[1].unit=string.rep('x',33)",
            "parameters[1].unit",
        ),
    ] {
        rejects(body, field);
    }
    let fixture = Fixture::new();
    fixture.write("ok.lua", altered("p.name=string.rep('é',64); p.description=string.rep('x',512); p.parameters[1].hint=''; p.parameters[1].unit=''"));
    assert!(fixture.load().is_ok());
}

#[test]
fn metatables_sparse_arrays_and_duplicate_parameter_keys_are_rejected() {
    for (body, field) in [
        (
            "setmetatable(p, {__index=function() while true do end end})",
            "",
        ),
        ("setmetatable(p.parameters, {})", "parameters"),
        ("setmetatable(p.parameters[1], {})", "parameters[1]"),
        ("p.parameters[3]=p.parameters[1]", "parameters"),
        ("p.parameters[0]=p.parameters[1]", "parameters"),
        ("p.parameters.extra=true", "parameters"),
        ("p.parameters[1]=false", "parameters[1]"),
        ("p.parameters[2]=p.parameters[1]", "parameters[2].key"),
        ("p.parameters[1].key='bad-key'", "parameters[1].key"),
        (
            "p.parameters[1].key=string.rep('x',65)",
            "parameters[1].key",
        ),
        ("p.parameters[1].kind='select'", "parameters[1].kind"),
    ] {
        rejects(body, field);
    }
}

#[test]
fn number_ranges_steps_defaults_and_exact_types_are_validated() {
    for (assignment, field) in [
        ("min='0'", "min"),
        ("max=math.huge", "max"),
        ("default=0/0", "default"),
        ("step=nil", "step"),
        ("step=0", "step"),
        ("step=-1", "step"),
        ("max=0", "max"),
        ("min=200", "max"),
        ("default=127", "default"),
        ("default=-1", "default"),
        ("default='12'", "default"),
        ("default=12.5", "default"),
        ("min=0.5", "min"),
        ("max=126.5", "max"),
        ("step=0.5", "step"),
        ("whole=1", "whole"),
        ("min_exclusive='true'", "min_exclusive"),
    ] {
        rejects(
            &format!("p.parameters[1].{assignment}"),
            &format!("parameters[1].{field}"),
        );
    }
    rejects(
        "p.parameters[1].min_exclusive=true; p.parameters[1].default=0",
        "parameters[1].default",
    );
    let fixture = Fixture::new();
    fixture.write(
        "ok.lua",
        altered("p.parameters[1].whole=nil; p.parameters[1].default=0.3; p.parameters[1].step=0.5"),
    );
    assert!(fixture.load().is_ok(), "step no cuantiza el valor inicial");
}

#[test]
fn text_and_bool_defaults_are_strict_and_reject_fields_of_other_kinds() {
    for parameter in [
        "{key='t',label='Texto',kind='text',default=''}",
        "{key='t',label='Texto',kind='text',default='   '}",
        "{key='t',label='Texto',kind='text',default=12}",
        "{key='t',label='Texto',kind='text',default='a\\nb'}",
        "{key='t',label='Texto',kind='text',default=string.rep('x',1025)}",
        "{key='b',label='Bool',kind='bool',default='true'}",
        "{key='b',label='Bool',kind='bool',default=1}",
        "{key='b',label='Bool',kind='bool',default=false,min=0}",
        "{key='t',label='Texto',kind='text',default='x',whole=false}",
    ] {
        let fixture = Fixture::new();
        fixture.write("bad.lua", source("user.invalid", parameter));
        assert!(fixture.load().is_err(), "{parameter}");
    }
    let fixture = Fixture::new();
    fixture.write(
        "ok.lua",
        source(
            "user.valid",
            &format!(
                "{NUMBER}, {{key='t',label='Texto',kind='text',default=string.rep('x',1024)}}"
            ),
        ),
    );
    assert!(fixture.load().is_ok());
}

#[test]
fn isolated_descriptor_evaluation_rejects_errors_and_unavailable_capabilities() {
    let fixture = Fixture::new();
    for body in [
        "return nil",
        "return 1",
        "this is not Lua",
        "error('fallo deliberado')",
        "return notes[1]",
        "io.open('file')",
        "os.execute('true')",
        "require('tool')",
        "load('return {}')",
        "pcall(function() end)",
    ] {
        fixture.write("bad.lua", body);
        assert!(fixture.load().is_err(), "{body}");
    }
    let checks = "assert(io==nil and os==nil and debug==nil and package==nil and require==nil and load==nil and loadfile==nil and dofile==nil and pcall==nil and xpcall==nil and print==nil and warn==nil and notes==nil and controls==nil)";
    fixture.write("bad.lua", format!("{checks}; {}", source("user.safe", "")));
    assert!(fixture.load().is_ok());
}
