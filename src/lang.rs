#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Rust,
    Swift,
}

pub fn from_path(rel: &str) -> Option<Lang> {
    match std::path::Path::new(rel).extension()?.to_str()? {
        "ts" | "mts" | "cts" => Some(Lang::TypeScript),
        "tsx" => Some(Lang::Tsx),
        "js" | "jsx" | "mjs" | "cjs" => Some(Lang::JavaScript),
        "py" | "pyi" => Some(Lang::Python),
        "go" => Some(Lang::Go),
        "rs" => Some(Lang::Rust),
        "swift" => Some(Lang::Swift),
        _ => None,
    }
}

impl Lang {
    pub const ALL: [Self; 7] = [
        Self::TypeScript,
        Self::Tsx,
        Self::JavaScript,
        Self::Python,
        Self::Go,
        Self::Rust,
        Self::Swift,
    ];

    pub fn language(self) -> tree_sitter::Language {
        match self {
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Swift => tree_sitter_swift::LANGUAGE.into(),
        }
    }

    pub fn query_source(self) -> &'static str {
        match self {
            Self::TypeScript => include_str!("../queries/typescript.scm"),
            Self::Tsx => include_str!("../queries/tsx.scm"),
            Self::JavaScript => include_str!("../queries/javascript.scm"),
            Self::Python => include_str!("../queries/python.scm"),
            Self::Go => include_str!("../queries/go.scm"),
            Self::Rust => include_str!("../queries/rust.scm"),
            Self::Swift => include_str!("../queries/swift.scm"),
        }
    }

    pub fn query(self) -> &'static tree_sitter::Query {
        use std::sync::OnceLock;
        static QUERIES: [OnceLock<tree_sitter::Query>; Lang::ALL.len()] =
            [const { OnceLock::new() }; Lang::ALL.len()];
        let index = Self::ALL
            .iter()
            .position(|lang| *lang == self)
            .expect("language missing from ALL");
        // Queries are embedded program assets, validated by all_grammars_load.
        QUERIES[index].get_or_init(|| {
            tree_sitter::Query::new(&self.language(), self.query_source())
                .expect("invalid embedded query")
        })
    }

    pub fn scope_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "struct_item",
                "enum_item",
                "trait_item",
                "impl_item",
                "function_item",
                "function_signature_item",
            ],
            Self::TypeScript | Self::Tsx => &[
                "class_declaration",
                "abstract_class_declaration",
                "class",
                "interface_declaration",
                "function_declaration",
                "generator_function_declaration",
                "method_definition",
                "enum_declaration",
                "internal_module",
                "module",
            ],
            Self::JavaScript => &[
                "class_declaration",
                "class",
                "function_declaration",
                "generator_function_declaration",
                "method_definition",
            ],
            Self::Python => &["class_definition", "function_definition"],
            Self::Go => &["function_declaration", "method_declaration", "type_spec"],
            Self::Swift => &[
                "class_declaration",
                "protocol_declaration",
                "function_declaration",
                "init_declaration",
            ],
        }
    }
}
