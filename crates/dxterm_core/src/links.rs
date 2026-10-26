//! Detection ranges are UTF-8 byte offsets; rendering maps these back to cells.
use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalLink {
    File {
        path: String,
        line: u32,
        column: Option<u32>,
    },
    Url(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkMatch {
    pub start: usize,
    pub end: usize,
    pub link: TerminalLink,
}

pub fn file_links(text: &str) -> Vec<LinkMatch> {
    static FILE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?:^|[\s("'])([^\s<>"'():]+):(\d+)(?::(\d+))?"#).unwrap());
    FILE.captures_iter(text)
        .filter_map(|capture| {
            let path = capture.get(1)?;
            if text[..path.start()].ends_with("://") || path.as_str().contains("://") {
                return None;
            }
            let line = capture.get(2)?.as_str().parse::<u32>().ok()?;
            if line == 0 {
                return None;
            }
            let column = capture
                .get(3)
                .map(|m| m.as_str().parse::<u32>())
                .transpose()
                .ok()?;
            Some(LinkMatch {
                start: path.start(),
                end: capture.get(0)?.end(),
                link: TerminalLink::File {
                    path: path.as_str().into(),
                    line,
                    column,
                },
            })
        })
        .collect()
}

pub fn url_links(text: &str) -> Vec<LinkMatch> {
    static URL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?:https?|ftp|file)://[^\s<>"']+"#).unwrap());
    URL.find_iter(text)
        .filter_map(|m| {
            let mut url = m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?']);
            // Keep balanced parentheses in URLs such as Wikipedia article names.
            while url.ends_with(')') && url.matches(')').count() > url.matches('(').count() {
                url = &url[..url.len() - 1];
            }
            while url.ends_with(']') && url.matches(']').count() > url.matches('[').count() {
                url = &url[..url.len() - 1];
            }
            (!url.ends_with("://")).then(|| LinkMatch {
                start: m.start(),
                end: m.start() + url.len(),
                link: TerminalLink::Url(url.into()),
            })
        })
        .collect()
}

pub fn detect_links(text: &str) -> Vec<LinkMatch> {
    let mut links = url_links(text);
    let files = file_links(text)
        .into_iter()
        .filter(|file| {
            !links
                .iter()
                .any(|url| file.start < url.end && file.end > url.start)
        })
        .collect::<Vec<_>>();
    links.extend(files);
    links.sort_by_key(|link| link.start);
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_paths_and_columns() {
        assert_eq!(
            file_links("error (src/main.rs:12:4): bad"),
            vec![LinkMatch {
                start: 7,
                end: 23,
                link: TerminalLink::File {
                    path: "src/main.rs".into(),
                    line: 12,
                    column: Some(4)
                }
            }]
        );
        assert_eq!(file_links("./foo.rs:3 ../lib.rs:9").len(), 2);
        assert!(file_links("https://example.com:123 src/main.rs:0").is_empty());
        assert!(file_links("src/main.rs:99999999999 src/a:1:99999999999").is_empty());
        assert_eq!(file_links("a:2:0").len(), 1);
    }
    #[test]
    fn urls_and_unicode_offsets() {
        let links = detect_links("界 (https://example.com/a_(b)). src/lib.rs:8");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].start, 5);
        assert_eq!(
            links[0].link,
            TerminalLink::Url("https://example.com/a_(b)".into())
        );
    }
}
