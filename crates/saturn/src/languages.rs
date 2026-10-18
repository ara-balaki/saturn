//! Fills in highlight queries the editor's bundled languages ship without. TSX gets only
//! TypeScript's own additions, which leave keywords and strings to JavaScript's query; Swift gets
//! no query at all. Both grammars' crates carry the full queries (MIT), so Saturn uses those.

use gpui_kit::component::highlighter::LanguageRegistry;

pub fn register() {
    let registry = LanguageRegistry::singleton();
    if let Some(mut tsx) = registry.language("tsx") {
        // TypeScript's patterns first, so they win where both match, as tree-sitter's own config
        // for TSX lists them.
        tsx.highlights = [
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        ]
        .join("\n")
        .into();
        tsx.injections = tree_sitter_javascript::INJECTIONS_QUERY.into();
        registry.register("tsx", &tsx);
    }
    if let Some(mut swift) = registry.language("swift") {
        swift.highlights = tree_sitter_swift::HIGHLIGHTS_QUERY.into();
        swift.injections = tree_sitter_swift::INJECTIONS_QUERY.into();
        swift.locals = tree_sitter_swift::LOCALS_QUERY.into();
        registry.register("swift", &swift);
    }
}
