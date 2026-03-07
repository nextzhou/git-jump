use std::ops::Range;

/// A candidate that passed the filter, with match highlight ranges.
#[derive(Debug, Clone)]
pub struct FilterMatch {
    /// Index into the original candidate list.
    pub index: usize,
    /// Byte ranges in the candidate string that matched tokens.
    /// Sorted by start position, non-overlapping (merged if necessary).
    pub highlight_ranges: Vec<Range<usize>>,
}

/// Filter candidates by a multi-token substring query.
///
/// The query is split by whitespace into tokens. A candidate matches only if
/// it contains **all** tokens as substrings (AND logic, case-insensitive).
/// Token order does not matter.
///
/// An empty (or whitespace-only) query matches all candidates.
///
/// Returns matches preserving original order, with highlight ranges for each
/// token's first occurrence in the candidate.
pub fn filter_candidates(candidates: &[String], query: &str) -> Vec<FilterMatch> {
    let tokens: Vec<&str> = query.split_whitespace().collect();

    if tokens.is_empty() {
        return candidates
            .iter()
            .enumerate()
            .map(|(index, _)| FilterMatch {
                index,
                highlight_ranges: Vec::new(),
            })
            .collect();
    }

    let lower_tokens: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();

    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            match_candidate(candidate, &lower_tokens).map(|highlight_ranges| FilterMatch {
                index,
                highlight_ranges,
            })
        })
        .collect()
}

fn match_candidate(candidate: &str, lower_tokens: &[String]) -> Option<Vec<Range<usize>>> {
    let lower_candidate = candidate.to_lowercase();
    let mut ranges = Vec::new();

    for token in lower_tokens {
        let pos = lower_candidate.find(token.as_str())?;
        ranges.push(pos..pos + token.len());
    }

    Some(merge_ranges(ranges))
}

fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if ranges.is_empty() {
        return ranges;
    }

    ranges.sort_by_key(|r| r.start);

    let mut merged = vec![ranges[0].clone()];
    for r in &ranges[1..] {
        let last = merged.last_mut().unwrap();
        if r.start <= last.end {
            last.end = last.end.max(r.end);
        } else {
            merged.push(r.clone());
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<String> {
        vec![
            "org/api-gateway".into(),
            "org/api-service".into(),
            "org/api-docs".into(),
            "platform/api-proxy".into(),
            "team/web-gateway".into(),
        ]
    }

    #[test]
    fn empty_query_returns_all() {
        let c = candidates();
        let result = filter_candidates(&c, "");
        assert_eq!(result.len(), 5);
        for (i, m) in result.iter().enumerate() {
            assert_eq!(m.index, i);
            assert!(m.highlight_ranges.is_empty());
        }
    }

    #[test]
    fn whitespace_only_query_returns_all() {
        let c = candidates();
        let result = filter_candidates(&c, "   ");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn single_token_substring_match() {
        let c = candidates();
        let result = filter_candidates(&c, "gate");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].index, 0); // org/api-gateway
        assert_eq!(result[1].index, 4); // team/web-gateway
    }

    #[test]
    fn single_token_case_insensitive() {
        let c = candidates();
        let result = filter_candidates(&c, "GATE");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn multi_token_and_logic() {
        let c = candidates();
        let result = filter_candidates(&c, "api gate");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].index, 0); // org/api-gateway
    }

    #[test]
    fn multi_token_order_independence() {
        let c = candidates();
        let result1 = filter_candidates(&c, "api gate");
        let result2 = filter_candidates(&c, "gate api");
        assert_eq!(result1.len(), result2.len());
        assert_eq!(result1[0].index, result2[0].index);
    }

    #[test]
    fn multi_token_no_match() {
        let c = candidates();
        let result = filter_candidates(&c, "gate foo");
        assert!(result.is_empty());
    }

    #[test]
    fn highlight_ranges_single_token() {
        let c = candidates();
        let result = filter_candidates(&c, "gate");
        // "org/api-gateway" -- "gate" starts at index 8
        assert_eq!(result[0].highlight_ranges, vec![8..12]);
    }

    #[test]
    fn highlight_ranges_multi_token() {
        let c = candidates();
        let result = filter_candidates(&c, "api gate");
        let ranges = &result[0].highlight_ranges;
        // "org/api-gateway": "api" at 4..7, "gate" at 8..12
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], 4..7);
        assert_eq!(ranges[1], 8..12);
    }

    #[test]
    fn highlight_ranges_overlapping_merged() {
        let c = vec!["abcdef".into()];
        // "bcd" at 1..4, "cde" at 2..5 -> merged to 1..5
        let result = filter_candidates(&c, "bcd cde");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].highlight_ranges, vec![1..5]);
    }

    #[test]
    fn preserves_original_order() {
        let c = candidates();
        let result = filter_candidates(&c, "api");
        let indices: Vec<usize> = result.iter().map(|m| m.index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn no_match_returns_empty() {
        let c = candidates();
        let result = filter_candidates(&c, "zzzzz");
        assert!(result.is_empty());
    }

    #[test]
    fn matches_full_display_text() {
        let c = candidates();
        // "org" is part of the display path, not just the project name
        let result = filter_candidates(&c, "org");
        assert_eq!(result.len(), 3); // org/api-gateway, org/api-service, org/api-docs
    }

    #[test]
    fn merge_ranges_no_overlap() {
        let ranges = vec![0..3, 5..8];
        assert_eq!(merge_ranges(ranges), vec![0..3, 5..8]);
    }

    #[test]
    fn merge_ranges_adjacent() {
        let ranges = vec![0..3, 3..6];
        assert_eq!(merge_ranges(ranges), vec![0..6]);
    }

    #[test]
    fn merge_ranges_overlap() {
        let ranges = vec![0..5, 3..8];
        assert_eq!(merge_ranges(ranges), vec![0..8]);
    }

    #[test]
    fn merge_ranges_contained() {
        let ranges = vec![0..10, 3..5];
        assert_eq!(merge_ranges(ranges), vec![0..10]);
    }

    #[test]
    fn merge_ranges_empty() {
        let ranges: Vec<Range<usize>> = vec![];
        assert_eq!(merge_ranges(ranges), Vec::<Range<usize>>::new());
    }
}
