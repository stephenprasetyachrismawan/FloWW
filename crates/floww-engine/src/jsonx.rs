//! Pull a JSON object out of an AI reply.
//!
//! Prompts ask for a fenced ```json block. Models sometimes add prose around
//! it, so this takes the last fenced block, or the last balanced `{...}`.

use serde::de::DeserializeOwned;

pub fn extract<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let candidates = fenced_blocks(text)
        .into_iter()
        .rev()
        .chain(balanced_objects(text).into_iter().rev());
    let mut last_err = String::from("no JSON object found in the reply");
    for c in candidates {
        match serde_json::from_str::<T>(c.trim()) {
            Ok(v) => return Ok(v),
            Err(e) => last_err = format!("the reply had JSON that did not match: {e}"),
        }
    }
    Err(last_err)
}

fn fenced_blocks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after[body_start..];
        let Some(end) = body.find("```") else { break };
        out.push(&body[..end]);
        rest = &body[end + 3..];
    }
    out
}

fn balanced_objects(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some(end) = match_brace(bytes, i)
        {
            out.push(&text[i..=end]);
            i = end + 1;
            continue;
        }
        i += 1;
    }
    out
}

fn match_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct A {
        a: i32,
    }

    #[test]
    fn fenced_block_wins() {
        let t = "Here you go:\n```json\n{\"a\": 2}\n```\nthanks {\"a\": 9}";
        // Last fenced block is preferred over loose objects.
        assert_eq!(extract::<A>(t).unwrap(), A { a: 2 });
    }

    #[test]
    fn loose_object_and_braces_in_strings() {
        let t = "result {\"a\": 3, \"note\": \"a } brace\"} done";
        #[derive(Deserialize)]
        struct B {
            a: i32,
            note: String,
        }
        let b: B = extract(t).unwrap();
        assert_eq!((b.a, b.note.as_str()), (3, "a } brace"));
    }

    #[test]
    fn missing_json_is_an_error() {
        assert!(extract::<A>("no json here").is_err());
    }
}
