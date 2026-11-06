pub mod config;
pub mod coverage;
pub mod fsutil;
pub mod gate;
pub mod model;
pub mod newcode;
pub mod rules;
pub mod vue;

#[cfg(test)]
mod grammars {
    use tree_sitter::{Language, Parser};

    fn parses(lang: Language, src: &str) {
        let mut p = Parser::new();
        p.set_language(&lang).unwrap();
        let t = p.parse(src, None).unwrap();
        assert!(!t.root_node().has_error(), "{src}");
    }

    #[test]
    fn all_grammars_load() {
        parses(tree_sitter_java::LANGUAGE.into(), "class A { int f() { return 1; } }");
        parses(tree_sitter_kotlin_ng::LANGUAGE.into(), "fun f(): Int { return 1 }");
        parses(tree_sitter_python::LANGUAGE.into(), "def f():\n    return 1\n");
        parses(tree_sitter_javascript::LANGUAGE.into(), "const a = <div>{1}</div>;");
        parses(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), "const a: number = 1;");
        parses(tree_sitter_typescript::LANGUAGE_TSX.into(), "const a = <div>{1 as number}</div>;");
        parses(tree_sitter_go::LANGUAGE.into(), "package a\nfunc f() int { return 1 }\n");
    }
}
