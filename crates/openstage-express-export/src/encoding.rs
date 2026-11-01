// SPDX-License-Identifier: Apache-2.0
//! Small text and byte helpers the exports share: escaping, base64, the hash a page's security rule names, and the small
//! Markdown subset descriptions are written in (bold, italic, links, bullet and numbered lists).

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};

pub fn base64_encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// The value a Content-Security-Policy gives to allow exactly one inline script: `'sha256-...'`.
pub fn csp_script_hash(script: &str) -> String {
    format!("'sha256-{}'", STANDARD.encode(Sha256::digest(script.as_bytes())))
}

/// Escape text for HTML, inside an element or inside a quoted attribute.
pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape text so Markdown shows it as written (for headings, alt text and link text).
pub fn markdown_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' | '*' | '_' | '[' | ']' | '`' | '<' | '>' | '#' | '|' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// True for a link address the exports will make clickable. Anything else (scripts, files, data) is shown as plain text.
pub fn is_safe_link(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:"))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '<' || c == '>')
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// One piece of a line of a description: the little Markdown subset descriptions are written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    /// A link whose address passed [`is_safe_link`]. Any other address is left as the plain text it was written as.
    Link {
        url: String,
        label: Vec<Inline>,
    },
}

/// One block of a description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// The lines of one paragraph (a line break inside a paragraph is kept).
    Paragraph(Vec<Vec<Inline>>),
    List {
        numbered: bool,
        items: Vec<Vec<Inline>>,
    },
}

fn push_text(out: &mut Vec<Inline>, c: char) {
    match out.last_mut() {
        Some(Inline::Text(text)) => text.push(c),
        _ => out.push(Inline::Text(c.to_string())),
    }
}

/// Inline Markdown (bold, italic, links) to pieces. Everything else is plain text.
pub fn parse_inline(text: &str) -> Vec<Inline> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // **bold**
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find_closing(&chars, i + 2, &['*', '*']) {
                if end > i + 2 {
                    let inner: String = chars[i + 2..end].iter().collect();
                    out.push(Inline::Strong(parse_inline(&inner)));
                    i = end + 2;
                    continue;
                }
            }
        }
        // *italic* and _italic_ (an underscore only at the edge of a word, so snake_case stays as written)
        if c == '*' || c == '_' {
            let at_word_start = c == '*' || i == 0 || !is_word_char(chars[i - 1]);
            if at_word_start && chars.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != c) {
                if let Some(end) = find_closing(&chars, i + 1, &[c]) {
                    let after_ok = c == '*' || chars.get(end + 1).is_none_or(|n| !is_word_char(*n));
                    if end > i + 1 && !chars[end - 1].is_whitespace() && after_ok {
                        let inner: String = chars[i + 1..end].iter().collect();
                        out.push(Inline::Emphasis(parse_inline(&inner)));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        // [text](url)
        if c == '[' {
            if let Some(close) = find_closing(&chars, i + 1, &[']']) {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = find_closing(&chars, close + 2, &[')']) {
                        let label: String = chars[i + 1..close].iter().collect();
                        let url: String = chars[close + 2..end].iter().collect();
                        if is_safe_link(&url) {
                            out.push(Inline::Link { url: url.trim().to_string(), label: parse_inline(&label) });
                            i = end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        push_text(&mut out, c);
        i += 1;
    }
    out
}

fn find_closing(chars: &[char], from: usize, pattern: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(pattern.len() - 1)).find(|&j| chars[j..j + pattern.len()] == *pattern)
}

fn bullet_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).map(str::trim)
}

fn numbered_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    trimmed[digits..].strip_prefix(". ").map(str::trim)
}

/// A description's Markdown subset as blocks: paragraphs (a line break inside one is kept), bullet lists and numbered lists,
/// with bold, italic and safe links inside them.
pub fn parse_markdown(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<Vec<Inline>> = Vec::new();
    let mut list: Option<(bool, Vec<Vec<Inline>>)> = None;
    fn flush_paragraph(blocks: &mut Vec<Block>, paragraph: &mut Vec<Vec<Inline>>) {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(std::mem::take(paragraph)));
        }
    }
    fn flush_list(blocks: &mut Vec<Block>, list: &mut Option<(bool, Vec<Vec<Inline>>)>) {
        if let Some((numbered, items)) = list.take() {
            blocks.push(Block::List { numbered, items });
        }
    }
    for line in text.lines() {
        if line.trim().is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_list(&mut blocks, &mut list);
        } else if let Some(item) = bullet_text(line).or_else(|| numbered_text(line)) {
            let numbered = numbered_text(line).is_some();
            flush_paragraph(&mut blocks, &mut paragraph);
            if list.as_ref().is_some_and(|(n, _)| *n != numbered) {
                flush_list(&mut blocks, &mut list);
            }
            list.get_or_insert((numbered, Vec::new())).1.push(parse_inline(item));
        } else {
            flush_list(&mut blocks, &mut list);
            paragraph.push(parse_inline(line.trim()));
        }
    }
    flush_paragraph(&mut blocks, &mut paragraph);
    flush_list(&mut blocks, &mut list);
    blocks
}

fn inlines_to_html(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(&html_escape(text)),
            Inline::Strong(inner) => out.push_str(&format!("<strong>{}</strong>", inlines_to_html(inner))),
            Inline::Emphasis(inner) => out.push_str(&format!("<em>{}</em>", inlines_to_html(inner))),
            Inline::Link { url, label } => out.push_str(&format!(
                "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
                html_escape(url),
                inlines_to_html(label)
            )),
        }
    }
    out
}

/// The Markdown subset of a description to HTML: paragraphs (a line break inside one becomes `<br>`), bullet lists and
/// numbered lists, with bold, italic and safe links inside them. Everything is escaped, so no HTML gets through.
pub fn markdown_to_html(text: &str) -> String {
    let mut out = String::new();
    for block in parse_markdown(text) {
        match block {
            Block::Paragraph(lines) => {
                let body: Vec<String> = lines.iter().map(|line| inlines_to_html(line)).collect();
                out.push_str(&format!("<p>{}</p>\n", body.join("<br>\n")));
            }
            Block::List { numbered, items } => {
                let tag = if numbered { "ol" } else { "ul" };
                out.push_str(&format!("<{tag}>\n"));
                for item in items {
                    out.push_str(&format!("<li>{}</li>\n", inlines_to_html(&item)));
                }
                out.push_str(&format!("</{tag}>\n"));
            }
        }
    }
    out
}

/// The plain words of some inline pieces: no marks, a link shown as `label (address)`.
pub fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(text),
            Inline::Strong(inner) | Inline::Emphasis(inner) => out.push_str(&inlines_to_plain(inner)),
            Inline::Link { url, label } => {
                let label = inlines_to_plain(label);
                if label.trim() == url.as_str() || label.trim().is_empty() {
                    out.push_str(url);
                } else {
                    out.push_str(&format!("{label} ({url})"));
                }
            }
        }
    }
    out
}

/// A description as plain paragraphs, for places that cannot show marks (speaker notes, slide text): one string for each
/// paragraph, list item or line, with `• ` or `1. ` in front of list items.
pub fn markdown_to_plain_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for block in parse_markdown(text) {
        match block {
            Block::Paragraph(lines) => out.extend(lines.iter().map(|line| inlines_to_plain(line))),
            Block::List { numbered, items } => {
                for (n, item) in items.iter().enumerate() {
                    let marker = if numbered { format!("{}. ", n + 1) } else { "\u{2022} ".to_string() };
                    out.push(format!("{marker}{}", inlines_to_plain(item)));
                }
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_script_hash_is_a_real_sha256() {
        // SHA-256 of "abc" is ba7816bf 8f01cfea 414140de 5dae2223 b00361a3 96177a9c b410ff61 f20015ad.
        assert_eq!(csp_script_hash("abc"), "'sha256-ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0='");
    }

    #[test]
    fn html_is_escaped_and_control_characters_are_dropped() {
        assert_eq!(
            html_escape("<a href=\"x\">Tom & 'Jerry'</a>"),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; &#39;Jerry&#39;&lt;/a&gt;"
        );
        assert_eq!(html_escape("a\u{0}b\u{7}c\nd\te"), "abc\nd\te");
        assert_eq!(html_escape("caf\u{e9} \u{65e5}\u{672c}"), "caf\u{e9} \u{65e5}\u{672c}");
    }

    #[test]
    fn markdown_special_characters_are_escaped() {
        assert_eq!(markdown_escape("Click \"Save\""), "Click \"Save\"");
        assert_eq!(markdown_escape("a*b_c [d] `e` <f> #g | h\\"), "a\\*b\\_c \\[d\\] \\`e\\` \\<f\\> \\#g \\| h\\\\");
        assert_eq!(markdown_escape("line\nbreak\ttab"), "line break tab");
    }

    #[test]
    fn only_web_and_mail_links_are_clickable() {
        for ok in ["https://example.com/a?b=c", "http://example.com", "mailto:a@example.com", "HTTPS://EXAMPLE.COM"] {
            assert!(is_safe_link(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///C:/x",
            "//example.com",
            "example.com",
            "https://a b",
            "https://a\"b",
            "ftp://x",
        ] {
            assert!(!is_safe_link(bad), "{bad}");
        }
    }

    #[test]
    fn bold_italic_and_links() {
        assert_eq!(
            markdown_to_html("a **bold** and *italic* and _also_ word"),
            "<p>a <strong>bold</strong> and <em>italic</em> and <em>also</em> word</p>\n"
        );
        assert_eq!(
            markdown_to_html("see [the site](https://example.com/x)"),
            "<p>see <a href=\"https://example.com/x\" rel=\"noopener noreferrer\">the site</a></p>\n"
        );
        assert_eq!(markdown_to_html("**a *b* c**"), "<p><strong>a <em>b</em> c</strong></p>\n");
    }

    #[test]
    fn a_description_becomes_blocks_that_every_export_can_draw() {
        let blocks = parse_markdown(
            "First **line**\nsecond line\n\n- one\n- two with [a link](https://example.com)\n\n1. step\n2. step",
        );
        assert_eq!(blocks.len(), 3);
        assert_eq!(
            blocks[0],
            Block::Paragraph(vec![
                vec![Inline::Text("First ".into()), Inline::Strong(vec![Inline::Text("line".into())])],
                vec![Inline::Text("second line".into())],
            ])
        );
        let Block::List { numbered: false, items } = &blocks[1] else { panic!("{:?}", blocks[1]) };
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[1],
            vec![
                Inline::Text("two with ".into()),
                Inline::Link { url: "https://example.com".into(), label: vec![Inline::Text("a link".into())] }
            ]
        );
        assert!(matches!(&blocks[2], Block::List { numbered: true, items } if items.len() == 2));
        assert_eq!(parse_markdown(""), vec![]);
        assert_eq!(parse_markdown("  \n\n "), vec![]);
        assert_eq!(
            parse_inline("a_b and 2*3*4"),
            vec![
                Inline::Text("a_b and 2".into()),
                Inline::Emphasis(vec![Inline::Text("3".into())]),
                Inline::Text("4".into())
            ]
        );
        assert_eq!(
            parse_inline("[x](javascript:alert(1))"),
            vec![Inline::Text("[x](javascript:alert(1))".into())],
            "an unsafe link stays text"
        );
    }

    #[test]
    fn plain_lines_drop_the_marks_and_spell_out_links_and_list_numbers() {
        assert_eq!(markdown_to_plain_lines(""), Vec::<String>::new());
        assert_eq!(markdown_to_plain_lines("a **bold** and *italic* word"), ["a bold and italic word"]);
        assert_eq!(
            markdown_to_plain_lines("see [the site](https://example.com/x) now"),
            ["see the site (https://example.com/x) now"]
        );
        assert_eq!(
            markdown_to_plain_lines("[https://example.com](https://example.com)"),
            ["https://example.com"],
            "no repeating an address"
        );
        assert_eq!(markdown_to_plain_lines("[](https://example.com)"), ["https://example.com"]);
        assert_eq!(markdown_to_plain_lines("line one\nline two\n\nnext"), ["line one", "line two", "next"]);
        assert_eq!(markdown_to_plain_lines("- a\n- b\n\n1. c\n2. d"), ["\u{2022} a", "\u{2022} b", "1. c", "2. d"]);
        assert_eq!(
            markdown_to_plain_lines("<b>not html</b> & more"),
            ["<b>not html</b> & more"],
            "plain text is not escaped: the caller does that"
        );
    }

    #[test]
    fn plain_text_that_only_looks_like_markdown_is_left_alone() {
        assert_eq!(markdown_to_html("snake_case_name and 2*3*4"), "<p>snake_case_name and 2<em>3</em>4</p>\n");
        assert_eq!(markdown_to_html("5 * 3 = 15 and a_b"), "<p>5 * 3 = 15 and a_b</p>\n");
        assert_eq!(markdown_to_html("**unclosed and *also"), "<p>**unclosed and *also</p>\n");
        assert_eq!(markdown_to_html("[not a link] (x)"), "<p>[not a link] (x)</p>\n");
    }

    #[test]
    fn no_html_gets_through_and_unsafe_links_become_text() {
        assert_eq!(markdown_to_html("<script>alert(1)</script>"), "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>\n");
        assert_eq!(
            markdown_to_html("[click](javascript:alert(1)) and [x](https://e.com\" onclick=\"y)"),
            "<p>[click](javascript:alert(1)) and [x](https://e.com&quot; onclick=&quot;y)</p>\n"
        );
        assert_eq!(markdown_to_html("**<b>x</b>**"), "<p><strong>&lt;b&gt;x&lt;/b&gt;</strong></p>\n");
    }

    #[test]
    fn paragraphs_lists_and_line_breaks() {
        let text = "First line\nsecond line\n\n- one\n- **two**\n\n1. a\n2. b\n\nlast";
        assert_eq!(
            markdown_to_html(text),
            "<p>First line<br>\nsecond line</p>\n<ul>\n<li>one</li>\n<li><strong>two</strong></li>\n</ul>\n<ol>\n<li>a</li>\n<li>b</li>\n</ol>\n<p>last</p>\n"
        );
        assert_eq!(markdown_to_html(""), "");
        assert_eq!(markdown_to_html("- a\n1. b"), "<ul>\n<li>a</li>\n</ul>\n<ol>\n<li>b</li>\n</ol>\n");
        assert_eq!(markdown_to_html("a\n- b"), "<p>a</p>\n<ul>\n<li>b</li>\n</ul>\n");
    }
}
