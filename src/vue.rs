#[derive(Debug, PartialEq, Eq)]
pub struct Extracted { pub text: String, pub ext: &'static str }

/// `NoScript`는 분석할 코드가 없다는 뜻이고 `Malformed`는 구조를 읽지 못했다는 뜻이다.
/// 둘을 섞으면 읽지 못한 파일의 이슈가 "고쳐짐"으로 닫힌다.
#[derive(Debug, PartialEq, Eq)]
pub enum Sfc { Script(Extracted), NoScript, Malformed }

pub fn extract(src: &str) -> Sfc {
    match blocks(src) {
        None => Sfc::Malformed,
        Some((keep, _)) if keep.is_empty() => Sfc::NoScript,
        Some((keep, ts)) => {
            let mut text = String::with_capacity(src.len());
            for (i, ch) in src.char_indices() {
                let inside = keep.iter().any(|(a, b)| i >= *a && i < *b);
                if inside || ch == '\n' || ch == '\r' { text.push(ch); } else { text.push(' '); }
            }
            Sfc::Script(Extracted { text, ext: if ts { "ts" } else { "js" } })
        }
    }
}

/// 최상위의 script 본문 범위와 TS 여부. 구조가 깨졌으면 `None`.
fn blocks(src: &str) -> Option<(Vec<(usize, usize)>, bool)> {
    let mut keep = vec![];
    let mut ts = false;
    let mut pos = 0;
    while let Some(i) = src[pos..].find('<') {
        let at = pos + i;
        let rest = &src[at..];
        if rest.starts_with("<!--") {
            pos = skip_comment(src, at)?;
        } else if is_tag(rest, "<template") {
            let (end, self_closing) = tag_end(src, at)?;
            pos = if self_closing { end } else { skip_template(src, end)? };
        } else if is_tag(rest, "<script") {
            let (end, self_closing) = tag_end(src, at)?;
            pos = end;
            if self_closing { continue; }
            let tag = &src[at..end];
            if ["lang=\"ts\"", "lang='ts'", "lang=\"tsx\"", "lang='tsx'"].iter().any(|l| tag.contains(l)) { ts = true; }
            let close = end + src[end..].find("</script")?;
            keep.push((end, close));
            pos = close + src[close..].find('>')? + 1;
        } else {
            pos = at + 1;
        }
    }
    Some((keep, ts))
}

fn is_tag(rest: &str, open: &str) -> bool {
    rest.starts_with(open) && rest.as_bytes().get(open.len()).is_none_or(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
}

fn skip_comment(src: &str, at: usize) -> Option<usize> {
    Some(at + 4 + src[at + 4..].find("-->")? + 3)
}

/// 시작 태그가 끝나는 위치와 자체 닫힘 여부. 속성값 안의 `>`는 건너뛴다.
fn tag_end(src: &str, start: usize) -> Option<(usize, bool)> {
    let mut quote = None;
    for (i, b) in src.bytes().enumerate().skip(start) {
        match (quote, b) {
            (None, b'"' | b'\'') => quote = Some(b),
            (Some(q), _) if q == b => quote = None,
            (None, b'>') => return Some((i + 1, src.as_bytes()[i - 1] == b'/')),
            _ => {}
        }
    }
    None
}

/// 여는 태그 다음(`from`)부터 짝이 맞는 `</template>` 다음 위치까지 건너뛴다.
fn skip_template(src: &str, from: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut pos = from;
    loop {
        let at = pos + src[pos..].find('<')?;
        let rest = &src[at..];
        if rest.starts_with("<!--") {
            pos = skip_comment(src, at)?;
        } else if is_tag(rest, "<template") {
            let (end, self_closing) = tag_end(src, at)?;
            if !self_closing { depth += 1; }
            pos = end;
        } else if rest.starts_with("</template") {
            pos = at + rest.find('>')? + 1;
            depth -= 1;
            if depth == 0 { return Some(pos); }
        } else {
            pos = at + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(src: &str) -> Extracted {
        match extract(src) {
            Sfc::Script(e) => e,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn keeps_line_numbers_and_blanks_template() {
        let src = "<template>\n  <div>{{a}}</div>\n</template>\n<script>\nexport default {}\n</script>\n";
        let e = script(src);
        let lines: Vec<&str> = e.text.lines().collect();
        assert_eq!(lines[4], "export default {}");
        assert!(lines[1].trim().is_empty());
        assert_eq!(e.text.lines().count(), src.lines().count());
        assert_eq!(e.ext, "js");
    }

    #[test]
    fn detects_ts_and_script_setup() {
        let src = "<script setup lang=\"ts\">\nconst a: number = 1\n</script>\n";
        let e = script(src);
        assert_eq!(e.ext, "ts");
        assert!(e.text.contains("const a: number = 1"));
        assert!(!e.text.contains("<script"));
    }

    #[test]
    fn merges_two_script_blocks() {
        let src = "<script>\nconst a = 1\n</script>\n<script setup>\nconst b = 2\n</script>\n";
        let e = script(src);
        assert!(e.text.contains("const a = 1") && e.text.contains("const b = 2"));
    }

    #[test]
    fn ignores_script_inside_template() {
        let src = "<template><template #x><script>bad()</script></template></template>\n<script>\ngood()\n</script>\n";
        let e = script(src);
        assert!(!e.text.contains("bad()"));
        assert!(e.text.contains("good()"));
    }

    #[test]
    fn no_script_is_distinct_from_malformed() {
        assert_eq!(extract("<template><div/></template>\n"), Sfc::NoScript);
        assert_eq!(extract("<script src=\"./a.js\"/>\n"), Sfc::NoScript);
        assert_eq!(extract("<template>\n<script>good()</script>\n"), Sfc::Malformed);
        assert_eq!(extract("<script>\ngood()\n"), Sfc::Malformed);
        assert_eq!(extract("<script lang=\"ts\"\n"), Sfc::Malformed);
        assert_eq!(extract("<!-- open\n<script>good()</script>\n"), Sfc::Malformed);
    }

    #[test]
    fn self_closing_template_is_followed_by_script() {
        let e = script("<template src=\"./x.html\"/>\n<script>\ngood()\n</script>\n");
        assert!(e.text.contains("good()"));
    }

    #[test]
    fn nested_self_closing_template_does_not_add_depth() {
        let e = script("<template><template #x/></template>\n<script>\ngood()\n</script>\n");
        assert!(e.text.contains("good()"));
    }

    #[test]
    fn tags_in_comments_are_ignored() {
        let e = script("<!-- <template> old -->\n<template><p/></template>\n<!-- <script>bad()</script> -->\n<script>\ngood()\n</script>\n");
        assert!(e.text.contains("good()") && !e.text.contains("bad()"));
        let e = script("<template><!-- </template> <template> --><p/></template>\n<script>\ngood()\n</script>\n");
        assert!(e.text.contains("good()"));
    }

    #[test]
    fn gt_in_attribute_does_not_end_tag() {
        let e = script("<script setup lang=\"ts\" generic=\"T extends A<B>\">\nconst a = 1\n</script>\n");
        assert_eq!(e.ext, "ts");
        assert!(e.text.contains("const a = 1") && !e.text.contains("extends"));
    }

    #[test]
    fn similar_tag_names_are_not_blocks() {
        let e = script("<templates>\n<scripts>\n<script>\ngood()\n</script>\n");
        assert!(e.text.contains("good()"));
    }
}
