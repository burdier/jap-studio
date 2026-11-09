//! Tokenización común de argumentos, sin shell ni evaluación de código.
pub fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut bracket = false;
    for ch in input.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                token.push(ch);
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch.is_whitespace() && !bracket {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            if ch == '[' {
                bracket = true;
            }
            if ch == ']' {
                bracket = false;
            }
            token.push(ch);
        }
    }
    if quote.is_some() || bracket {
        return Err("Comillas o lista de formatos sin cerrar".into());
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}
