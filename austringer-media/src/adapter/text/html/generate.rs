//! The html writer: spec -> self-contained single-file HTML5 via maud
//! (compile-time templates, automatic text escaping, zero external
//! assets). Byte-deterministic: html carries no clock fields.

use maud::{DOCTYPE, Markup, Render, html};

use super::spec::{HtmlBlock, HtmlSpec};
use crate::error::MediaError;
use crate::generate::GenerationSpec;

/// Generate deterministic self-contained html5 bytes from a validated
/// spec: `<!DOCTYPE html>` + `<meta charset="utf-8">` + one `<title>`
/// when named + the body blocks. Text is maud-escaped; nothing outside
/// the block subset is ever emitted.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the html
/// contract (see [`HtmlSpec::from_generation`]).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let doc = HtmlSpec::from_generation(spec)?;
    let title = &doc.title;
    let blocks = &doc.blocks;
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                @if let Some(text) = title { title { (text) } }
            }
            body {
                @for block in blocks { (BlockView(block)) }
            }
        }
    };
    Ok(markup.into_string().into_bytes())
}

struct BlockView<'a>(&'a HtmlBlock);

impl Render for BlockView<'_> {
    fn render(&self) -> Markup {
        match self.0 {
            HtmlBlock::Heading { level, text } => {
                let view = heading_view(*level, text);
                html! { (view) }
            }
            HtmlBlock::Paragraph { text } => html! { p { (text) } },
            HtmlBlock::List { ordered, items } => {
                if *ordered {
                    html! { ol { @for item in items { li { (item) } } } }
                } else {
                    html! { ul { @for item in items { li { (item) } } } }
                }
            }
            HtmlBlock::Code { text } => html! { pre { code { (text) } } },
            HtmlBlock::Table { header, rows } => html! {
                table {
                    thead { tr { @for cell in header { th { (cell) } } } }
                    tbody {
                        @for row in rows { tr { @for cell in row { td { (cell) } } } }
                    }
                }
            },
        }
    }
}

// maud element names are static tokens, so the six heading levels are
// six arms; `level` is validated 1..=6, the catch-all only satisfies
// exhaustiveness.
fn heading_view(level: u8, text: &str) -> Markup {
    match level {
        1 => html! { h1 { (text) } },
        2 => html! { h2 { (text) } },
        3 => html! { h3 { (text) } },
        4 => html! { h4 { (text) } },
        5 => html! { h5 { (text) } },
        _ => html! { h6 { (text) } },
    }
}
