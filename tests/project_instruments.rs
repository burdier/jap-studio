use elevate::{
    musical::model::InstrumentId,
    project::{ProjectDoc, ProjectPiano},
};

#[test]
fn piano_records_roundtrip_and_invalid_edits_preserve_previous_state() {
    let doc = ProjectDoc::new("Piano").unwrap();
    let piano = ProjectPiano {
        id: InstrumentId(44),
        source: "/piano.sf2".into(),
        preset: 3,
        parameters: vec![0.7, 127., 2345.],
    };
    doc.set_pianos(&[piano.clone()]).unwrap();
    let restored = ProjectDoc::import(&doc.snapshot().unwrap()).unwrap();
    assert_eq!(restored.pianos().unwrap(), vec![piano.clone()]);
    let mut invalid = piano.clone();
    invalid.parameters[0] = f32::NAN;
    assert!(doc.set_pianos(&[invalid]).is_err());
    assert!(doc.set_pianos(&[piano.clone(), piano.clone()]).is_err());
    assert_eq!(doc.pianos().unwrap(), vec![piano]);
}

#[test]
fn old_documents_without_piano_state_remain_readable() {
    let doc = ProjectDoc::new("Anterior").unwrap();
    assert!(
        ProjectDoc::import(&doc.snapshot().unwrap())
            .unwrap()
            .pianos()
            .unwrap()
            .is_empty()
    );
}
