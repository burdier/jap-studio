//! Lector de los árboles XML de Hydrogen, sin expansión de DTD ni recursos externos.
#[derive(Default)]
pub(super) struct Element {
    pub tag: String,
    text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn child(&self, tag: &str) -> Option<&Self> {
        self.children.iter().find(|node| node.tag == tag)
    }

    pub fn value(&self, tag: &str) -> Option<&str> {
        self.child(tag).map(Self::text)
    }

    pub fn text(&self) -> &str {
        self.text.trim()
    }
}

pub(super) fn parse(mut input: &str) -> Result<Element, String> {
    let mut stack = vec![Element::default()];
    while !input.is_empty() {
        let Some((text, rest)) = input.split_once('<') else {
            stack.last_mut().unwrap().text.push_str(&entities(input)?);
            break;
        };
        stack.last_mut().unwrap().text.push_str(&entities(text)?);
        input = rest;
        if let Some(comment) = input.strip_prefix("!--") {
            input = comment
                .split_once("-->")
                .ok_or("Comentario XML incompleto")?
                .1;
            continue;
        }
        if let Some(cdata) = input.strip_prefix("![CDATA[") {
            let (value, rest) = cdata.split_once("]]>").ok_or("CDATA incompleto")?;
            stack.last_mut().unwrap().text.push_str(value);
            input = rest;
            continue;
        }
        if let Some(header) = input.strip_prefix('?') {
            input = header.split_once("?>").ok_or("Cabecera XML incompleta")?.1;
            continue;
        }
        if input.starts_with('!') {
            return Err("Las declaraciones DTD no son compatibles".into());
        }
        let mut quote = None;
        let end = input
            .char_indices()
            .find_map(|(i, c)| match (quote, c) {
                (Some(delimiter), c) if c == delimiter => {
                    quote = None;
                    None
                }
                (None, '\'' | '"') => {
                    quote = Some(c);
                    None
                }
                (None, '>') => Some(i),
                _ => None,
            })
            .ok_or("Etiqueta XML incompleta")?;
        let token = input[..end].trim();
        input = &input[end + 1..];
        if let Some(closing) = token.strip_prefix('/') {
            if stack.len() < 2 || stack.last().unwrap().tag != local_name(closing.trim()) {
                return Err("Etiquetas XML desparejadas".into());
            }
            let node = stack.pop().unwrap();
            stack.last_mut().unwrap().children.push(node);
        } else {
            let empty = token.ends_with('/');
            let name = local_name(
                token
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .ok_or("Etiqueta XML vacía")?,
            );
            let node = Element {
                tag: name.into(),
                ..Default::default()
            };
            if empty {
                stack.last_mut().unwrap().children.push(node);
            } else if stack.len() < 64 {
                stack.push(node);
            } else {
                return Err("El árbol XML supera 63 niveles".into());
            }
        }
    }
    let doc = stack.pop().unwrap();
    if stack.len() != 0 || doc.children.len() != 1 {
        return Err("Documento XML incompleto".into());
    }
    Ok(doc.children.into_iter().next().unwrap())
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn entities(mut text: &str) -> Result<String, String> {
    let mut value = String::new();
    while let Some((literal, tail)) = text.split_once('&') {
        value.push_str(literal);
        let (entity, rest) = tail.split_once(';').ok_or("Entidad XML incompleta")?;
        let ch = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    entity.strip_prefix('#').and_then(|n| n.parse().ok())
                };
                code.and_then(char::from_u32)
                    .ok_or("Entidad XML desconocida")?
            }
        };
        value.push(ch);
        text = rest;
    }
    value.push_str(text);
    Ok(value)
}
