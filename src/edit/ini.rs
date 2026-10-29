//! Line-based editing of INI-style files (KDE `*rc`, Konsole profiles, OnlyOffice): comments,
//! blank lines, ordering, spacing around `=` and line endings are kept as they are. Keys are
//! case-sensitive and, as in KDE, the last copy of a key in a section is the effective one.

struct Line {
    body: String,
    eol: String,
}

fn split(text: &str) -> Vec<Line> {
    text.split_inclusive('\n')
        .map(|raw| {
            let (body, eol) = if let Some(body) = raw.strip_suffix("\r\n") {
                (body, "\r\n")
            } else if let Some(body) = raw.strip_suffix('\n') {
                (body, "\n")
            } else {
                (raw, "")
            };
            Line { body: body.to_string(), eol: eol.to_string() }
        })
        .collect()
}

fn join(lines: &[Line]) -> String {
    lines.iter().map(|l| format!("{}{}", l.body, l.eol)).collect()
}

/// The line ending the file uses, for lines we add.
fn eol_style(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| l.eol.as_str())
        .find(|eol| !eol.is_empty())
        .unwrap_or("\n")
        .to_string()
}

fn header_name(body: &str) -> Option<&str> {
    body.trim().strip_prefix('[')?.strip_suffix(']')
}

/// `(key, value)` of a `key = value` line; comments, headers and other lines have none.
fn key_value(body: &str) -> Option<(&str, &str)> {
    let trimmed = body.trim_start();
    if trimmed.is_empty() || trimmed.starts_with(['#', ';', '[']) {
        return None;
    }
    let (key, value) = body.split_once('=')?;
    Some((key.trim(), value))
}

/// For each line, the section it belongs to (a header line belongs to its own section).
fn section_names(lines: &[Line]) -> Vec<String> {
    let mut current = String::new();
    lines
        .iter()
        .map(|line| {
            if let Some(name) = header_name(&line.body) {
                current = name.to_string();
            }
            current.clone()
        })
        .collect()
}

fn is_blank(line: &Line) -> bool {
    line.body.trim().is_empty()
}

/// The effective value of `key` in `section`.
pub fn get_value(text: &str, section: &str, key: &str) -> Option<String> {
    let lines = split(text);
    let sections = section_names(&lines);
    lines
        .iter()
        .zip(&sections)
        .filter(|(_, s)| s.as_str() == section)
        .filter_map(|(line, _)| key_value(&line.body))
        .filter(|(k, _)| *k == key)
        .map(|(_, value)| value.trim().to_string())
        .next_back()
}

/// Sets `key` in `section`: replaces the value in place (every copy in that section), or adds the
/// key at the end of the section, or adds the whole section at the end of the file.
pub fn set_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines = split(text);
    let sections = section_names(&lines);
    let style = eol_style(&lines);

    let mut replaced = false;
    for (line, line_section) in lines.iter_mut().zip(&sections) {
        if line_section != section || header_name(&line.body).is_some() {
            continue;
        }
        if key_value(&line.body).is_some_and(|(k, _)| k == key) {
            let equals = line.body.find('=').expect("a key line has an equals sign");
            let after = &line.body[equals + 1..];
            let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
            line.body = format!("{}{value}", &line.body[..equals + 1 + spaces]);
            replaced = true;
        }
    }
    if replaced {
        return join(&lines);
    }

    let last_header = lines.iter().rposition(|l| header_name(&l.body) == Some(section));
    match last_header {
        Some(header) => {
            let end = lines[header + 1..]
                .iter()
                .position(|l| header_name(&l.body).is_some())
                .map_or(lines.len(), |p| header + 1 + p);
            let after_last_content = (header..end)
                .rev()
                .find(|&i| !is_blank(&lines[i]))
                .map_or(header + 1, |i| i + 1);
            if lines[after_last_content - 1].eol.is_empty() {
                lines[after_last_content - 1].eol = style.clone();
            }
            lines.insert(after_last_content, Line { body: format!("{key}={value}"), eol: style });
        }
        None => {
            if let Some(last) = lines.last_mut() {
                if last.eol.is_empty() {
                    last.eol = style.clone();
                }
                if !is_blank(last) {
                    lines.push(Line { body: String::new(), eol: style.clone() });
                }
            }
            lines.push(Line { body: format!("[{section}]"), eol: style.clone() });
            lines.push(Line { body: format!("{key}={value}"), eol: style });
        }
    }
    join(&lines)
}

/// Removes every copy of `key` from `section`.
pub fn remove_key(text: &str, section: &str, key: &str) -> String {
    let lines = split(text);
    let sections = section_names(&lines);
    let kept: Vec<Line> = lines
        .into_iter()
        .zip(sections)
        .filter(|(line, line_section)| {
            !(line_section == section
                && header_name(&line.body).is_none()
                && key_value(&line.body).is_some_and(|(k, _)| k == key))
        })
        .map(|(line, _)| line)
        .collect();
    join(&kept)
}

/// Removes `section` when it holds nothing but blank lines (comments count as content). When the
/// removed section was the last one, the blank line that separated it from the previous one goes too.
pub fn remove_section_if_empty(text: &str, section: &str) -> String {
    let mut lines = split(text);
    let headers: Vec<usize> = (0..lines.len())
        .filter(|&i| header_name(&lines[i].body) == Some(section))
        .collect();

    for &header in headers.iter().rev() {
        let end = lines[header + 1..]
            .iter()
            .position(|l| header_name(&l.body).is_some())
            .map_or(lines.len(), |p| header + 1 + p);
        if !lines[header + 1..end].iter().all(is_blank) {
            continue;
        }
        let start = if end == lines.len() && header > 0 && is_blank(&lines[header - 1]) {
            header - 1
        } else {
            header
        };
        lines.drain(start..end);
    }
    join(&lines)
}
