//! Compila una tabla Lua de pasos a una regla musical sin VM en audio.
use crate::musical::weave::{MAX_STEPS, WeaveRule};
use mlua::{Value, chunk::ChunkMode};

pub fn compile(source: &str) -> Result<WeaveRule, String> {
    if source.is_empty() || source.len() > 16 * 1024 {
        return Err("Tejer: fuente Lua vacía o mayor de 16 KiB".into());
    }
    let lua = super::sandbox::new().map_err(|error| error.to_string())?;
    let descriptor: mlua::Table = lua
        .load(source)
        .set_mode(ChunkMode::Text)
        .eval()
        .map_err(|error| error.to_string())?;
    for pair in descriptor.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|error| error.to_string())?;
        if !matches!(key, Value::String(ref text) if text.to_str().ok().as_deref() == Some("steps"))
        {
            return Err("Tejer: solo se admite el campo steps".into());
        }
    }
    let steps: mlua::Table = descriptor
        .raw_get("steps")
        .map_err(|_| "Tejer: falta steps")?;
    let len = steps.raw_len();
    if len == 0 || len > MAX_STEPS {
        return Err("Tejer: se requieren 1–64 pasos".into());
    }
    let mut indices = Vec::with_capacity(len);
    for index in 1..=len {
        let value: Value = steps.raw_get(index).map_err(|error| error.to_string())?;
        let Value::Integer(branch) = value else {
            return Err("Tejer: los pasos deben ser enteros".into());
        };
        let zero_based = branch
            .checked_sub(1)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or("Tejer: rama fuera de 1–16")?;
        indices.push(zero_based);
    }
    if steps.pairs::<Value, Value>().count() != len {
        return Err("Tejer: steps debe ser una lista densa".into());
    }
    WeaveRule::new(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_declares_the_branch_order_and_rejects_sparse_or_invalid_steps() {
        let rule = compile(include_str!("../../assets/scripts/hilos/tejer.lua")).unwrap();
        assert_eq!(rule.steps(), &[0, 1, 2, 0, 1, 2]);
        assert!(
            compile("return { steps = {1, 3, 2} }")
                .unwrap()
                .valid_for(3)
        );
        assert!(compile("return { steps = {[1]=1, [3]=2} }").is_err());
        assert!(compile("return { steps = {1, 17} }").is_err());
        assert!(compile("return { steps = {1}, extra = 1 }").is_err());
    }
}
