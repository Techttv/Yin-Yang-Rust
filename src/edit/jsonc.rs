//! Changing one top-level string in a JSON-with-comments file (VS Code's `settings.json`) without
//! parsing it into a value, so comments, trailing commas, ordering and indentation survive.

use anyhow::{bail, Result};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Str,
    Punct(u8),
    Other,
}

struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

/// Splits the text into strings, punctuation and bare words, dropping whitespace and comments.
fn tokenize(text: &str) -> Result<Vec<Token>> {
    let bytes = text.as_bytes();
    let mut i = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let mut tokens = Vec::new();

    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => match text[i + 2..].find("*/") {
                Some(at) => i += 2 + at + 2,
                None => bail!("unterminated comment"),
            },
            b'"' => {
                let start = i;
                i += 1;
                loop {
                    match bytes.get(i) {
                        None | Some(b'\n') => bail!("unterminated string"),
                        Some(b'\\') => i += 2,
                        Some(b'"') => {
                            i += 1;
                            break;
                        }
                        Some(_) => i += 1,
                    }
                }
                tokens.push(Token { kind: Kind::Str, start, end: i });
            }
            c @ (b'{' | b'}' | b'[' | b']' | b':' | b',') => {
                tokens.push(Token { kind: Kind::Punct(c), start: i, end: i + 1 });
                i += 1;
            }
            _ => {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b'{' | b'}' | b'[' | b']' | b':' | b',' | b'"' | b'/')
                {
                    i += 1;
                }
                tokens.push(Token { kind: Kind::Other, start, end: i });
            }
        }
    }
    Ok(tokens)
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes")
}

/// Sets the string value of the first top-level `key`, adding the key when it is missing.
/// Fails, changing nothing, when the text has no well-formed top-level object, or when the
/// current value is an object or array.
pub fn set_top_level_string(text: &str, key: &str, value: &str) -> Result<String> {
    let tokens = tokenize(text)?;
    if tokens.first().map(|t| t.kind) != Some(Kind::Punct(b'{')) {
        bail!("the top-level value is not an object");
    }

    let mut stack = Vec::new();
    let mut close = None;
    let mut value_token = None;
    for (i, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct(open @ (b'{' | b'[')) => stack.push(open),
            Kind::Punct(end @ (b'}' | b']')) => {
                let expected = if end == b'}' { b'{' } else { b'[' };
                if stack.pop() != Some(expected) {
                    bail!("mismatched brackets");
                }
                if stack.is_empty() {
                    close = Some(i);
                    break;
                }
            }
            Kind::Str if stack.len() == 1 && value_token.is_none() => {
                let is_key = tokens.get(i + 1).map(|t| t.kind) == Some(Kind::Punct(b':'));
                let name = serde_json::from_str::<String>(&text[token.start..token.end]).ok();
                if is_key && name.as_deref() == Some(key) {
                    value_token = Some(i + 2);
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        bail!("the top-level object is not closed");
    };
    let new_value = json_string(value);

    if let Some(index) = value_token {
        let Some(old) = tokens.get(index).filter(|t| matches!(t.kind, Kind::Str | Kind::Other)) else {
            bail!("the current value of {key} is not a string");
        };
        return Ok(format!("{}{new_value}{}", &text[..old.start], &text[old.end..]));
    }

    let member = format!("{}: {new_value}", json_string(key));
    let open_end = tokens[0].end;
    let close_start = tokens[close].start;

    if close > 1 {
        // Members exist: put the new one first, in the file's own layout.
        let first = tokens[1].start;
        let multi_line = text[open_end..first].contains('\n');
        let insertion = if multi_line {
            let line_start = text[..first].rfind('\n').map_or(0, |at| at + 1);
            let indent = &text[line_start..first];
            let indent = if indent.trim().is_empty() { indent } else { "    " };
            format!("\n{indent}{member},")
        } else {
            format!("{member}, ")
        };
        return Ok(format!("{}{insertion}{}", &text[..open_end], &text[open_end..]));
    }

    let inner = &text[open_end..close_start];
    if inner.trim().is_empty() {
        let layout = if inner.contains('\n') { format!("\n    {member}\n") } else { member };
        return Ok(format!("{}{layout}{}", &text[..open_end], &text[close_start..]));
    }

    // Only comments inside the braces: add the member after them, before the closing brace.
    let at = close_start - (inner.len() - inner.trim_end().len());
    let insertion = if inner.contains('\n') { format!("\n    {member}") } else { format!(" {member}") };
    Ok(format!("{}{insertion}{}", &text[..at], &text[at..]))
}
