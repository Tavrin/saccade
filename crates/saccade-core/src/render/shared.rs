//! The design system every HTML page embeds: tokens, components and the shared
//! script. Pages are self-contained, so these are inlined at render time.

/// Colour, type, space and shape tokens (light and dark).
pub(crate) const TOKENS_CSS: &str = include_str!("../../assets/tokens.css");
/// Buttons, segmented controls, chips, panels, tables, popovers, kbd, toasts,
/// the command palette, the help dialog and the status bar.
pub(crate) const COMPONENTS_CSS: &str = include_str!("../../assets/components.css");
/// Element builder, action registry, command palette, popovers, full screen and
/// the zoom math the image stages share.
pub(crate) const UI_JS: &str = include_str!("../../assets/ui.js");

/// The stylesheet of one page: the shared design system followed by `parts`
/// (the page's own CSS, in cascade order).
pub(crate) fn page_css(parts: &[&str]) -> String {
    let mut css = String::with_capacity(
        TOKENS_CSS.len()
            + COMPONENTS_CSS.len()
            + parts.iter().map(|p| p.len() + 1).sum::<usize>()
            + 2,
    );
    css.push_str(TOKENS_CSS);
    css.push('\n');
    css.push_str(COMPONENTS_CSS);
    css.push_str(include_str!("../../assets/perf.css"));
    for part in parts {
        css.push('\n');
        css.push_str(part);
    }
    css
}

/// The script of one page: the shared script followed by `parts`.
pub(crate) fn page_js(parts: &[&str]) -> String {
    let mut js = String::from(UI_JS);
    js.push_str(include_str!("../../assets/a11y-display.js"));
    js.push_str(include_str!("../../assets/perf.js"));
    for part in parts {
        js.push('\n');
        js.push_str(part);
    }
    js
}

/// Applies the shared stylesheet to auxiliary served pages which have no page renderer.
pub(crate) fn complete_html(body: String) -> String {
    if body.contains(TOKENS_CSS) {
        return body;
    }
    body.replacen(
        "</head>",
        &format!("<style>{}</style></head>", page_css(&[])),
        1,
    )
}
