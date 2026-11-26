//! Plain substring search over a whole document, independent of rendering.
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Matches beyond this count are not collected; [`FindSummary::capped`] reports it.
pub const MAX_FIND_MATCHES: usize = 10_000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindQuery {
    pub text: String,
    pub case_sensitive: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindSummary {
    pub count: usize,
    pub capped: bool,
}

/// Non-overlapping, sorted UTF-8 **byte** ranges of `query` in `text`, and
/// whether the result stopped at [`MAX_FIND_MATCHES`]. Case-insensitive mode
/// uses ASCII folding, which keeps byte offsets valid. Empty queries and
/// queries spanning lines match nothing.
pub fn find_matches(text: &str, query: &FindQuery) -> (Vec<Range<usize>>, bool) {
    if query.case_sensitive || !searchable(&query.text) {
        find_in(text, &query.text)
    } else {
        find_in(&text.to_ascii_lowercase(), &query.text.to_ascii_lowercase())
    }
}

pub(crate) fn searchable(needle: &str) -> bool {
    !needle.is_empty() && !needle.contains(['\n', '\r'])
}

/// Searches an already folded haystack for an equally folded needle.
pub(crate) fn find_in(haystack: &str, needle: &str) -> (Vec<Range<usize>>, bool) {
    if !searchable(needle) {
        return (Vec::new(), false);
    }
    let mut matches: Vec<_> = haystack
        .match_indices(needle)
        .map(|(start, found)| start..start + found.len())
        .take(MAX_FIND_MATCHES + 1)
        .collect();
    let capped = matches.len() > MAX_FIND_MATCHES;
    matches.truncate(MAX_FIND_MATCHES);
    (matches, capped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matches_cases() {
        let query = |text: &str, case_sensitive| FindQuery {
            text: text.into(),
            case_sensitive,
        };
        let many = "ab".repeat(MAX_FIND_MATCHES + 5);
        type Case = (&'static str, FindQuery, &'static [(usize, usize)]);
        let cases: [Case; 7] = [
            ("Foo foo FOO", query("foo", true), &[(4, 7)]),
            (
                "Foo foo FOO",
                query("fOo", false),
                &[(0, 3), (4, 7), (8, 11)],
            ),
            ("café Été x", query("x", false), &[(12, 13)]),
            ("ÄÄ é Ä", query("é", false), &[(5, 7)]),
            ("aaaa", query("aa", true), &[(0, 2), (2, 4)]),
            ("a\nb", query("a\nb", true), &[]),
            ("abc", query("", false), &[]),
        ];
        for (text, query, expected) in cases {
            let (found, was_capped) = find_matches(text, &query);
            let pairs: Vec<_> = found.iter().map(|r| (r.start, r.end)).collect();
            assert_eq!(pairs, expected, "{text:?} / {query:?}");
            assert!(!was_capped);
            assert!(
                found
                    .iter()
                    .all(|r| text.is_char_boundary(r.start) && text.is_char_boundary(r.end))
            );
        }
        let (found, capped) = find_matches(&many, &query("B", false));
        assert_eq!((found.len(), capped), (MAX_FIND_MATCHES, true));
    }
}
