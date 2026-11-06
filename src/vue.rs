#[derive(Debug, PartialEq, Eq)]
pub struct Extracted { pub text: String, pub ext: &'static str }

pub fn extract(src: &str) -> Option<Extracted> {
    let mut keep: Vec<(usize, usize)> = Vec::new();
    let mut ts = false;
    let mut pos = 0;
    while pos < src.len() {
        let rest = &src[pos..];
        let next_t = rest.find("<template");
        let next_s = rest.find("<script");
        match (next_t, next_s) {
            (None, None) => break,
            (Some(t), s) if s.map_or(true, |s| t < s) => {
                pos = skip_template(src, pos + t);
            }
            (_, Some(s)) => {
                let tag_start = pos + s;
                let open_end = tag_start + src[tag_start..].find('>')? + 1;
                let tag = &src[tag_start..open_end];
                if tag.contains("lang=\"ts\"") || tag.contains("lang='ts'") || tag.contains("lang=\"tsx\"") { ts = true; }
                let close = open_end + src[open_end..].find("</script>")?;
                keep.push((open_end, close));
                pos = close + "</script>".len();
            }
            (Some(_), None) => unreachable!(),
        }
    }
    if keep.is_empty() { return None; }
    let mut text = String::with_capacity(src.len());
    for (i, ch) in src.char_indices() {
        let inside = keep.iter().any(|(a, b)| i >= *a && i < *b);
        if inside || ch == '\n' || ch == '\r' { text.push(ch); } else { text.push(' '); }
    }
    Some(Extracted { text, ext: if ts { "ts" } else { "js" } })
}

fn skip_template(src: &str, start: usize) -> usize {
    let mut depth = 0usize;
    let mut pos = start;
    loop {
        let rest = &src[pos..];
        let open = rest.find("<template");
        let close = rest.find("</template>");
        match (open, close) {
            (Some(o), Some(c)) if o < c => { depth += 1; pos += o + "<template".len(); }
            (_, Some(c)) => {
                depth = depth.saturating_sub(1);
                pos += c + "</template>".len();
                if depth == 0 { return pos; }
            }
            _ => return src.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_line_numbers_and_blanks_template() {
        let src = "<template>\n  <div>{{a}}</div>\n</template>\n<script>\nexport default {}\n</script>\n";
        let e = extract(src).unwrap();
        let lines: Vec<&str> = e.text.lines().collect();
        assert_eq!(lines[4], "export default {}");
        assert!(lines[1].trim().is_empty());
        assert_eq!(e.text.lines().count(), src.lines().count());
        assert_eq!(e.ext, "js");
    }

    #[test]
    fn detects_ts_and_script_setup() {
        let src = "<script setup lang=\"ts\">\nconst a: number = 1\n</script>\n";
        let e = extract(src).unwrap();
        assert_eq!(e.ext, "ts");
        assert!(e.text.contains("const a: number = 1"));
        assert!(!e.text.contains("<script"));
    }

    #[test]
    fn merges_two_script_blocks() {
        let src = "<script>\nconst a = 1\n</script>\n<script setup>\nconst b = 2\n</script>\n";
        let e = extract(src).unwrap();
        assert!(e.text.contains("const a = 1") && e.text.contains("const b = 2"));
    }

    #[test]
    fn ignores_script_inside_template() {
        let src = "<template><template #x><script>bad()</script></template></template>\n<script>\ngood()\n</script>\n";
        let e = extract(src).unwrap();
        assert!(!e.text.contains("bad()"));
        assert!(e.text.contains("good()"));
    }

    #[test]
    fn no_script_returns_none() {
        assert_eq!(extract("<template><div/></template>\n"), None);
    }
}
