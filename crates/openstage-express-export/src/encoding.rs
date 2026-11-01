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

/// Inline Markdown (bold, italic, links) to HTML. Everything else is escaped text.
fn inline_to_html(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // **bold**
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            if let Some(end) = find_closing(&chars, i + 2, &['*', '*']) {
                if end > i + 2 {
                    let inner: String = chars[i + 2..end].iter().collect();
                    out.push_str(&format!("<strong>{}</strong>", inline_to_html(&inner)));
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
                        out.push_str(&format!("<em>{}</em>", inline_to_html(&inner)));
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
                            out.push_str(&format!(
                                "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
                                html_escape(url.trim()),
                                inline_to_html(&label)
                            ));
                            i = end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        out.push_str(&html_escape(&c.to_string()));
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

/// The Markdown subset of a description to HTML: paragraphs (a line break inside one becomes `<br>`), bullet lists and
/// numbered lists, with bold, italic and safe links inside them. Everything is escaped, so no HTML gets through.
pub fn markdown_to_html(text: &str) -> String {
    let mut out = String::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut list: Option<(bool, Vec<String>)> = None; // (numbered, items)
    fn flush_paragraph(out: &mut String, paragraph: &mut Vec<&str>) {
        if !paragraph.is_empty() {
            let body: Vec<String> = paragraph.iter().map(|l| inline_to_html(l.trim())).collect();
            out.push_str(&format!("<p>{}</p>\n", body.join("<br>\n")));
            paragraph.clear();
        }
    }
    fn flush_list(out: &mut String, list: &mut Option<(bool, Vec<String>)>) {
        if let Some((numbered, items)) = list.take() {
            let tag = if numbered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>\n"));
            for item in items {
                out.push_str(&format!("<li>{item}</li>\n"));
            }
            out.push_str(&format!("</{tag}>\n"));
        }
    }
    for line in text.lines() {
        if line.trim().is_empty() {
            flush_paragraph(&mut out, &mut paragraph);
            flush_list(&mut out, &mut list);
        } else if let Some(item) = bullet_text(line).or_else(|| numbered_text(line)) {
            let numbered = numbered_text(line).is_some();
            flush_paragraph(&mut out, &mut paragraph);
            if list.as_ref().is_some_and(|(n, _)| *n != numbered) {
                flush_list(&mut out, &mut list);
            }
            list.get_or_insert((numbered, Vec::new())).1.push(inline_to_html(item));
        } else {
            flush_list(&mut out, &mut list);
            paragraph.push(line);
        }
    }
    flush_paragraph(&mut out, &mut paragraph);
    flush_list(&mut out, &mut list);
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
