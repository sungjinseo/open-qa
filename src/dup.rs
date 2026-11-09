// Ported from SonarSource/sonarqube sonar-duplications (PmdBlockChunker.java).
//
// SonarQube
// Copyright (C) SonarSource Sàrl
// mailto:info AT sonarsource DOT com
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program; if not, write to the Free Software Foundation,
// Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
//

use crate::config::DupCfg;
use crate::model::{CloneGroup, ClonePart, Lang};
use crate::vue::Sfc;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use tree_sitter::{Language, Node, Parser, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenLine { pub line: u32, pub hash: u64, pub tokens: u32 }

#[derive(Debug, Default)]
pub struct DupOutcome {
    pub clones: Vec<CloneGroup>,
    pub total_lines: BTreeMap<String, u32>,
    pub unanalyzed: Vec<String>,
}

fn language(lang: Lang) -> Language {
    match lang {
        Lang::Java => tree_sitter_java::LANGUAGE.into(),
        Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Js => tree_sitter_javascript::LANGUAGE.into(),
        Lang::Ts | Lang::Vue => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

fn skip_node(n: &Node) -> bool {
    let k = n.kind();
    k.contains("comment") || (n.child_count() > 0 && (k.contains("import") || k.contains("package")))
}

fn add_token(n: &Node, src: &[u8], out: &mut BTreeMap<u32, (u64, u32)>) {
    if n.end_byte() == n.start_byte() { return; }
    let line = n.start_position().row as u32 + 1;
    let e = out.entry(line).or_insert((0xcbf29ce484222325, 0));
    for b in &src[n.start_byte()..n.end_byte()] {
        e.0 ^= *b as u64;
        e.0 = e.0.wrapping_mul(0x100000001b3);
    }
    e.0 ^= 0xff;
    e.0 = e.0.wrapping_mul(0x100000001b3);
    e.1 += 1;
}

/// 재귀하면 아주 깊은 구문 트리에서 스택이 넘치므로 커서로 순회한다.
fn collect(tree: &Tree, src: &[u8], out: &mut BTreeMap<u32, (u64, u32)>) {
    let mut cur = tree.walk();
    loop {
        let n = cur.node();
        let skip = skip_node(&n);
        if !skip && n.child_count() == 0 { add_token(&n, src, out); }
        if !skip && cur.goto_first_child() { continue; }
        while !cur.goto_next_sibling() {
            if !cur.goto_parent() { return; }
        }
    }
}

pub fn tokenize(lang: Lang, src: &str) -> Option<Vec<TokenLine>> {
    let (text, grammar) = if lang == Lang::Vue {
        match crate::vue::extract(src) {
            Sfc::Script(e) => { let g = if e.ext == "ts" { Lang::Ts } else { Lang::Js }; (e.text, g) }
            Sfc::NoScript => return Some(vec![]),
            Sfc::Malformed => return None,
        }
    } else {
        (src.to_string(), lang)
    };
    let mut p = Parser::new();
    p.set_language(&language(grammar)).ok()?;
    let tree = p.parse(&text, None)?;
    let mut m = BTreeMap::new();
    collect(&tree, text.as_bytes(), &mut m);
    Some(m.into_iter().map(|(line, (hash, tokens))| TokenLine { line, hash, tokens }).collect())
}

struct FileBlocks { file: String, lines: Vec<TokenLine>, hashes: Vec<u64> }

/// `PmdBlockChunker`: 같은 줄이 연속되면 처음과 마지막만 남기고 롤링 해시로 블록을 만든다.
fn chunk(lines: Vec<TokenLine>, block: usize) -> (Vec<TokenLine>, Vec<u64>) {
    let mut f: Vec<TokenLine> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let mut j = i + 1;
        while j < lines.len() && lines[j].hash == lines[i].hash { j += 1; }
        f.push(lines[i].clone());
        if i < j - 1 { f.push(lines[j - 1].clone()); }
        i = j;
    }
    if block == 0 || f.len() < block { return (f, vec![]); }
    let mut hashes = Vec::with_capacity(f.len() - block + 1);
    let mut power = 1u64;
    for _ in 0..block - 1 { power = power.wrapping_mul(31); }
    let mut h = 0u64;
    for t in &f[..block] { h = h.wrapping_mul(31).wrapping_add(t.hash); }
    hashes.push(h);
    for k in block..f.len() {
        h = h.wrapping_sub(f[k - block].hash.wrapping_mul(power));
        h = h.wrapping_mul(31).wrapping_add(f[k].hash);
        hashes.push(h);
    }
    (f, hashes)
}

const MAX_OCCURRENCES: usize = 200;

fn detect_group(files: &[FileBlocks], cfg: &DupCfg) -> Vec<CloneGroup> {
    let mut index: HashMap<u64, Vec<(usize, usize)>> = HashMap::new();
    for (fi, f) in files.iter().enumerate() {
        for (bi, h) in f.hashes.iter().enumerate() { index.entry(*h).or_default().push((fi, bi)); }
    }
    let part = |fi: usize, start: usize, run: usize| -> (ClonePart, u64) {
        let f = &files[fi];
        let end = start + run - 1 + cfg.block_lines - 1;
        let tokens: u64 = f.lines[start..=end].iter().map(|t| t.tokens as u64).sum();
        (ClonePart { file: f.file.clone(), start_line: f.lines[start].line, end_line: f.lines[end].line }, tokens)
    };
    let mut groups: BTreeMap<String, BTreeSet<ClonePart>> = BTreeMap::new();
    for occ in index.values() {
        if occ.len() < 2 || occ.len() > MAX_OCCURRENCES { continue; }
        for (x, &(fa, ba)) in occ.iter().enumerate() {
            for &(fb, bb) in &occ[x + 1..] {
                let same_prev = ba > 0 && bb > 0 && files[fa].hashes[ba - 1] == files[fb].hashes[bb - 1];
                if same_prev { continue; }
                let mut run = 1;
                while ba + run < files[fa].hashes.len() && bb + run < files[fb].hashes.len()
                    && files[fa].hashes[ba + run] == files[fb].hashes[bb + run] { run += 1; }
                let (pa, ta) = part(fa, ba, run);
                let (pb, tb) = part(fb, bb, run);
                if ta.min(tb) < cfg.min_tokens as u64 { continue; }
                if fa == fb && pa.end_line >= pb.start_line && pb.end_line >= pa.start_line { continue; }
                let mut sha = Sha256::new();
                for h in &files[fa].hashes[ba..ba + run] { sha.update(h.to_le_bytes()); }
                let key: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
                let e = groups.entry(key).or_default();
                e.insert(pa);
                e.insert(pb);
            }
        }
    }
    groups.into_iter().map(|(key, parts)| {
        let parts: Vec<ClonePart> = parts.into_iter().collect();
        let lines = parts.iter().map(|p| p.end_line - p.start_line + 1).max().unwrap_or(0);
        CloneGroup { key, parts, lines }
    }).collect()
}

pub fn detect(root: &Path, files: &[String], cfg: &DupCfg) -> DupOutcome {
    enum R { Ok(String, Lang, u32, Vec<TokenLine>), Bad(String) }
    let results: Vec<R> = files.par_iter().filter_map(|rel| {
        let lang = Lang::of(rel)?;
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else { return Some(R::Bad(rel.clone())); };
        match tokenize(lang, &text) {
            Some(t) => Some(R::Ok(rel.clone(), lang, text.lines().count() as u32, t)),
            None => Some(R::Bad(rel.clone())),
        }
    }).collect();
    let mut out = DupOutcome::default();
    let mut by_group: BTreeMap<&'static str, Vec<FileBlocks>> = BTreeMap::new();
    for r in results {
        match r {
            R::Bad(f) => out.unanalyzed.push(f),
            R::Ok(file, lang, n, lines) => {
                out.total_lines.insert(file.clone(), n);
                let (lines, hashes) = chunk(lines, cfg.block_lines);
                by_group.entry(lang.dup_group()).or_default().push(FileBlocks { file, lines, hashes });
            }
        }
    }
    for files in by_group.values() { out.clones.extend(detect_group(files, cfg)); }
    out.clones.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.key.cmp(&b.key)));
    out.unanalyzed.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("        int {prefix}{i} = compute({i}, \"v\") + other({i});\n")).collect()
    }

    fn java(name: &str, n: usize) -> String {
        format!("import java.util.List;\nclass {name} {{\n    void f() {{\n{}    }}\n}}\n", body("value", n))
    }

    #[test]
    fn tokenize_skips_comments_and_imports() {
        let a = tokenize(Lang::Java, "import a.B;\n// c\nclass A { int x; }\n").unwrap();
        assert_eq!(a.iter().map(|t| t.line).collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn tokenize_each_language() {
        for (l, s) in [
            (Lang::Kotlin, "import a.b\nfun f() { val x = 1 }\n"),
            (Lang::Python, "import os\ndef f():\n    return 1\n"),
            (Lang::Js, "import a from 'a';\nconst x = <div/>;\n"),
            (Lang::Ts, "import a from 'a';\nconst x: number = 1;\n"),
            (Lang::Tsx, "import a from 'a';\nconst x = <div/>;\n"),
            (Lang::Go, "package a\nimport \"fmt\"\nfunc f() { fmt.Println(1) }\n"),
            (Lang::Vue, "<template><p/></template>\n<script>\nconst x = 1\n</script>\n"),
        ] {
            let t = tokenize(l, s).unwrap();
            assert!(!t.is_empty(), "{l:?}");
            assert!(t.iter().all(|x| x.line > 1 || l == Lang::Python), "{l:?} {t:?}");
        }
    }

    #[test]
    fn vue_lines_keep_original_numbers() {
        let t = tokenize(Lang::Vue, "<template><p/></template>\n<script>\nconst x = 1\n</script>\n").unwrap();
        assert_eq!(t[0].line, 3);
    }

    #[test]
    fn identical_lines_produce_same_hash() {
        let a = tokenize(Lang::Go, "package a\nfunc f() { g(1) }\n").unwrap();
        let b = tokenize(Lang::Go, "package a\nfunc f()   {   g( 1 ) }\n").unwrap();
        assert_eq!(a[0].hash, b[0].hash);
    }

    fn cfg() -> DupCfg { DupCfg { block_lines: 5, min_tokens: 20 } }

    #[test]
    fn detects_clone_between_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.java"), java("A", 12)).unwrap();
        std::fs::write(d.path().join("B.java"), java("B", 12)).unwrap();
        let files = vec!["A.java".to_string(), "B.java".to_string()];
        let o = detect(d.path(), &files, &cfg());
        assert_eq!(o.clones.len(), 1, "{:?}", o.clones);
        let g = &o.clones[0];
        assert_eq!(g.parts.len(), 2);
        assert!(g.lines >= 12);
        assert_eq!(o.total_lines["A.java"], 17);
    }

    #[test]
    fn different_languages_are_not_compared() {
        let d = tempfile::tempdir().unwrap();
        let shared: String = (0..12).map(|i| format!("foo({i}, bar({i}), baz({i}), qux({i}));\n")).collect();
        std::fs::write(d.path().join("a.js"), &shared).unwrap();
        std::fs::write(d.path().join("b.kt"), format!("fun f() {{\n{shared}}}\n")).unwrap();
        let o = detect(d.path(), &["a.js".to_string(), "b.kt".to_string()], &cfg());
        assert!(o.clones.is_empty());
    }

    #[test]
    fn code_below_min_tokens_is_not_a_clone() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.java"), java("A", 2)).unwrap();
        std::fs::write(d.path().join("B.java"), java("B", 2)).unwrap();
        let strict = DupCfg { block_lines: 5, min_tokens: 100 };
        let o = detect(d.path(), &["A.java".to_string(), "B.java".to_string()], &strict);
        assert!(o.clones.is_empty());
    }

    #[test]
    fn clone_key_is_stable_when_lines_shift() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.java"), java("A", 12)).unwrap();
        std::fs::write(d.path().join("B.java"), java("B", 12)).unwrap();
        let files = vec!["A.java".to_string(), "B.java".to_string()];
        let k1 = detect(d.path(), &files, &cfg()).clones[0].key.clone();
        std::fs::write(d.path().join("A.java"), format!("\n\n{}", java("A", 12))).unwrap();
        let k2 = detect(d.path(), &files, &cfg()).clones[0].key.clone();
        assert_eq!(k1, k2);
    }

    #[test]
    fn unreadable_or_broken_file_is_reported() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.java"), [0xff, 0xfe, 0x00]).unwrap();
        let o = detect(d.path(), &["A.java".to_string(), "missing.go".to_string()], &cfg());
        assert_eq!(o.unanalyzed, vec!["A.java".to_string(), "missing.go".to_string()]);
    }

    #[test]
    fn deep_expression_does_not_overflow_the_stack() {
        let src = format!("const x = 1{};\n", " + 1".repeat(150_000));
        let t = tokenize(Lang::Js, &src).unwrap();
        assert_eq!(t.len(), 1);
        assert!(t[0].tokens > 300_000);
    }

    #[test]
    fn malformed_vue_is_unanalyzed_but_no_script_is_not() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Bad.vue"), "<template>\n<script>\nx()\n</script>\n").unwrap();
        std::fs::write(d.path().join("Plain.vue"), "<template><p/></template>\n").unwrap();
        let o = detect(d.path(), &["Bad.vue".to_string(), "Plain.vue".to_string()], &cfg());
        assert_eq!(o.unanalyzed, vec!["Bad.vue".to_string()]);
        assert!(o.total_lines.contains_key("Plain.vue"));
    }
}
