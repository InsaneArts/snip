//! Capture Text's result (OCR-02, OCR-05): its lines joined into one, and the
//! links in it.

/// One line (OCR-02 off): a paragraph's lines joined with spaces, a
/// hyphenated word mended across its break; paragraphs stay apart.
pub fn one_line(text: &str) -> String {
    text.split("\n\n")
        .map(|para| {
            let mut out = String::new();
            for line in para.lines().map(str::trim).filter(|l| !l.is_empty()) {
                if out.ends_with('-') && line.starts_with(|c: char| c.is_lowercase()) {
                    out.pop();
                } else if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(line);
            }
            out
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Web links in `text` (OCR-05): http(s) URLs and bare www. addresses, with
/// trailing punctuation dropped.
pub fn links(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|w| {
            let w = w.trim_start_matches(['(', '<', '[', '"', '\'']).trim_end_matches(['.', ',', ';', ':', ')', '>', ']', '"', '\'', '!', '?']);
            let lower = w.to_ascii_lowercase();
            if (lower.starts_with("https://") || lower.starts_with("http://")) && w.len() > 10 {
                Some(w.to_string())
            } else if lower.starts_with("www.") && w[4..].contains('.') {
                Some(format!("https://{w}"))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_join_and_paragraphs_stay() {
        assert_eq!(one_line("The quick brown\nfox jumps over\nthe lazy dog\n\nNew para-\ngraph here"), "The quick brown fox jumps over the lazy dog\n\nNew paragraph here");
        assert_eq!(one_line("  one  \n\n\n two "), "one\n\ntwo");
        assert_eq!(one_line("A-\nB"), "A- B", "a capital after a hyphen is a new word");
    }

    #[test]
    fn links_are_found() {
        assert_eq!(links("see https://example.com/a?b=1, or (www.omarchy.org)."), vec!["https://example.com/a?b=1", "https://www.omarchy.org"]);
        assert!(links("no links here: http:// www. example").is_empty());
    }
}
