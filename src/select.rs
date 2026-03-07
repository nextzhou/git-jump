use std::io::{self, IsTerminal};
use std::ops::Range;

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

use crate::error::{Error, Result};
use crate::filter;
use crate::resolve::{self, DisplayCandidate};

pub struct SelectResult {
    pub index: usize,
    pub final_query: String,
}

pub fn select<F>(
    candidates: &[DisplayCandidate],
    initial_filter: &str,
    scorer: F,
) -> Result<SelectResult>
where
    F: Fn(usize, &str) -> (f64, f64),
{
    let display_texts: Vec<String> = candidates.iter().map(|c| c.display_text.clone()).collect();

    let initial_matches = if initial_filter.is_empty() {
        None
    } else {
        let matches = filter::filter_candidates(&display_texts, initial_filter);
        let deduped = resolve::dedup_filter_matches(candidates, &matches);
        let mut sorted = deduped;
        sort_by_score_dc(&mut sorted, candidates, initial_filter, &scorer);
        Some(sorted)
    };

    if let Some(ref matches) = initial_matches {
        if matches.len() == 1 {
            let dc = &candidates[matches[0].index];
            return Ok(SelectResult {
                index: dc.project_index,
                final_query: initial_filter.to_string(),
            });
        }
    }

    if !io::stderr().is_terminal() {
        return match initial_matches {
            Some(matches) if matches.is_empty() => Err(Error::NoMatch {
                pattern: initial_filter.to_string(),
            }),
            Some(matches) => {
                let dc = &candidates[matches[0].index];
                Ok(SelectResult {
                    index: dc.project_index,
                    final_query: initial_filter.to_string(),
                })
            }
            None => {
                if candidates.is_empty() {
                    Err(Error::NoMatch {
                        pattern: String::new(),
                    })
                } else {
                    let deduped = all_candidates_deduped(candidates);
                    if deduped.is_empty() {
                        Err(Error::NoMatch {
                            pattern: String::new(),
                        })
                    } else {
                        Ok(SelectResult {
                            index: candidates[deduped[0].index].project_index,
                            final_query: String::new(),
                        })
                    }
                }
            }
        };
    }

    run_interactive(candidates, &display_texts, initial_filter, &scorer)
}

fn all_candidates_deduped(candidates: &[DisplayCandidate]) -> Vec<filter::FilterMatch> {
    let display_texts: Vec<String> = candidates.iter().map(|c| c.display_text.clone()).collect();
    let all: Vec<filter::FilterMatch> = display_texts
        .iter()
        .enumerate()
        .map(|(index, _)| filter::FilterMatch {
            index,
            highlight_ranges: Vec::new(),
        })
        .collect();
    resolve::dedup_filter_matches(candidates, &all)
}

fn sort_by_score_dc<F>(
    matches: &mut Vec<filter::FilterMatch>,
    candidates: &[DisplayCandidate],
    query: &str,
    scorer: &F,
) where
    F: Fn(usize, &str) -> (f64, f64),
{
    let mut scored: Vec<(filter::FilterMatch, f64, f64)> = matches
        .drain(..)
        .map(|m| {
            let dc = &candidates[m.index];
            let (ps, gs) = scorer(dc.project_index, query);
            (m, ps, gs)
        })
        .collect();

    scored.sort_by(|(a, a_ps, a_gs), (b, b_ps, b_gs)| {
        b_ps.partial_cmp(a_ps)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b_gs.partial_cmp(a_gs).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.index.cmp(&b.index))
    });

    matches.extend(scored.into_iter().map(|(m, _, _)| m));
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
}

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        io::stderr().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        terminal::disable_raw_mode().ok();
        io::stderr().execute(LeaveAlternateScreen).ok();
    }
}

fn run_interactive<F>(
    candidates: &[DisplayCandidate],
    display_texts: &[String],
    initial_filter: &str,
    scorer: &F,
) -> Result<SelectResult>
where
    F: Fn(usize, &str) -> (f64, f64),
{
    let mut guard = TerminalGuard::new()?;
    event_loop(
        &mut guard.terminal,
        candidates,
        display_texts,
        initial_filter,
        scorer,
    )
}

struct AppState {
    filter_input: String,
    filtered: Vec<filter::FilterMatch>,
    list_state: ListState,
    no_color: bool,
}

impl AppState {
    fn new<F>(
        candidates: &[DisplayCandidate],
        display_texts: &[String],
        initial_filter: &str,
        scorer: &F,
    ) -> Self
    where
        F: Fn(usize, &str) -> (f64, f64),
    {
        let filter_input = initial_filter.to_string();
        let matches = filter::filter_candidates(display_texts, &filter_input);
        let mut deduped = resolve::dedup_filter_matches(candidates, &matches);
        sort_by_score_dc(&mut deduped, candidates, &filter_input, scorer);
        let mut list_state = ListState::default();
        if !deduped.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            filter_input,
            filtered: deduped,
            list_state,
            no_color: std::env::var_os("NO_COLOR").is_some(),
        }
    }

    fn update_filter<F>(
        &mut self,
        candidates: &[DisplayCandidate],
        display_texts: &[String],
        scorer: &F,
    ) where
        F: Fn(usize, &str) -> (f64, f64),
    {
        let matches = filter::filter_candidates(display_texts, &self.filter_input);
        self.filtered = resolve::dedup_filter_matches(candidates, &matches);
        sort_by_score_dc(&mut self.filtered, candidates, &self.filter_input, scorer);
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn selected_project_index(&self, candidates: &[DisplayCandidate]) -> Option<usize> {
        self.list_state
            .selected()
            .map(|i| candidates[self.filtered[i].index].project_index)
    }

    fn move_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
            }
        }
    }

    fn move_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected + 1 < self.filtered.len() {
                self.list_state.select(Some(selected + 1));
            }
        }
    }
}

fn event_loop<F>(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    candidates: &[DisplayCandidate],
    display_texts: &[String],
    initial_filter: &str,
    scorer: &F,
) -> Result<SelectResult>
where
    F: Fn(usize, &str) -> (f64, f64),
{
    let mut state = AppState::new(candidates, display_texts, initial_filter, scorer);

    loop {
        terminal.draw(|frame| render(frame, candidates, &mut state))?;

        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event::read()?
        else {
            continue;
        };
        if kind != KeyEventKind::Press {
            continue;
        }

        match (code, modifiers) {
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                return Err(Error::Interrupted);
            }
            (KeyCode::Char('d'), m)
                if m.contains(KeyModifiers::CONTROL) && state.filter_input.is_empty() =>
            {
                return Err(Error::Cancelled);
            }
            (KeyCode::Esc, _) => return Err(Error::Cancelled),
            (KeyCode::Enter, _) => {
                if let Some(pi) = state.selected_project_index(candidates) {
                    return Ok(SelectResult {
                        index: pi,
                        final_query: state.filter_input,
                    });
                }
                if state.filtered.is_empty() {
                    return Err(Error::NoMatch {
                        pattern: state.filter_input.clone(),
                    });
                }
            }
            (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
                state.filter_input.clear();
                state.update_filter(candidates, display_texts, scorer);
            }
            (KeyCode::Backspace, _) => {
                state.filter_input.pop();
                state.update_filter(candidates, display_texts, scorer);
            }
            (KeyCode::Up | KeyCode::BackTab, _) => state.move_up(),
            (KeyCode::Down | KeyCode::Tab, _) => state.move_down(),
            (KeyCode::Char(c), m) if !m.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                state.filter_input.push(c);
                state.update_filter(candidates, display_texts, scorer);
            }
            _ => {}
        }
    }
}

fn render(frame: &mut Frame, candidates: &[DisplayCandidate], state: &mut AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_filter_input(frame, state, chunks[0]);
    render_candidate_list(frame, candidates, state, chunks[1]);
}

fn render_filter_input(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let prompt_style = if state.no_color {
        Style::default()
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    };
    let input_line = Line::from(vec![
        Span::styled("> ", prompt_style),
        Span::raw(state.filter_input.clone()),
    ]);
    frame.render_widget(Paragraph::new(input_line), area);

    let cursor_x =
        (area.x + 2 + state.filter_input.len() as u16).min(area.x + area.width.saturating_sub(1));
    frame.set_cursor_position(Position::new(cursor_x, area.y));
}

fn render_candidate_list(
    frame: &mut Frame,
    candidates: &[DisplayCandidate],
    state: &mut AppState,
    area: ratatui::layout::Rect,
) {
    let indicator_width: usize = 2;
    let max_text_width = (area.width as usize).saturating_sub(indicator_width);

    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|fm| {
            let dc = &candidates[fm.index];
            build_list_item(
                &dc.display_text,
                &fm.highlight_ranges,
                dc.disambiguation.as_deref(),
                max_text_width,
                state.no_color,
            )
        })
        .collect();

    let highlight_style = if state.no_color {
        Style::default()
    } else {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    };

    let list = List::new(items)
        .highlight_symbol("> ")
        .highlight_style(highlight_style)
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

const ELLIPSIS: &str = "...";

fn build_list_item(
    text: &str,
    ranges: &[Range<usize>],
    disambiguation: Option<&str>,
    max_width: usize,
    no_color: bool,
) -> ListItem<'static> {
    let suffix_len = disambiguation.map(|s| s.len() + 2).unwrap_or(0);
    let text_budget = max_width.saturating_sub(suffix_len);
    let (display_text, truncated) = truncate_text(text, text_budget);
    let clipped = clip_ranges(ranges, display_text.len());

    if no_color {
        build_no_color_item(display_text, &clipped, truncated, disambiguation)
    } else {
        build_color_item(display_text, &clipped, truncated, disambiguation)
    }
}

fn truncate_text(text: &str, max_width: usize) -> (&str, bool) {
    if text.len() <= max_width {
        return (text, false);
    }
    let mut end = max_width.saturating_sub(ELLIPSIS.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

fn clip_ranges(ranges: &[Range<usize>], max_len: usize) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter(|r| r.start < max_len)
        .map(|r| r.start..r.end.min(max_len))
        .filter(|r| !r.is_empty())
        .collect()
}

fn build_color_item(
    text: &str,
    ranges: &[Range<usize>],
    truncated: bool,
    disambiguation: Option<&str>,
) -> ListItem<'static> {
    let match_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut pos = 0;

    for range in ranges {
        if range.start > pos {
            spans.push(Span::raw(text[pos..range.start].to_string()));
        }
        spans.push(Span::styled(
            text[range.start..range.end].to_string(),
            match_style,
        ));
        pos = range.end;
    }

    if pos < text.len() {
        spans.push(Span::raw(text[pos..].to_string()));
    }

    if truncated {
        spans.push(Span::raw(ELLIPSIS.to_string()));
    }

    if let Some(suffix) = disambiguation {
        spans.push(Span::styled(format!("  {suffix}"), dim_style));
    }

    ListItem::new(Line::from(spans))
}

fn build_no_color_item(
    text: &str,
    ranges: &[Range<usize>],
    truncated: bool,
    disambiguation: Option<&str>,
) -> ListItem<'static> {
    let mut result = build_no_color_string(text, ranges, truncated);
    if let Some(suffix) = disambiguation {
        result.push_str("  ");
        result.push_str(suffix);
    }
    ListItem::new(result)
}

fn build_no_color_string(text: &str, ranges: &[Range<usize>], truncated: bool) -> String {
    let mut result = String::with_capacity(text.len() + ranges.len() * 2);
    let mut pos = 0;

    for range in ranges {
        result.push_str(&text[pos..range.start]);
        result.push('[');
        result.push_str(&text[range.start..range.end]);
        result.push(']');
        pos = range.end;
    }

    result.push_str(&text[pos..]);

    if truncated {
        result.push_str(ELLIPSIS);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_text_short() {
        let (text, truncated) = truncate_text("hello", 10);
        assert_eq!(text, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_text_exact() {
        let (text, truncated) = truncate_text("hello", 5);
        assert_eq!(text, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_text_over() {
        let (text, truncated) = truncate_text("hello world", 8);
        assert_eq!(text, "hello");
        assert!(truncated);
    }

    #[test]
    fn clip_ranges_within_bounds() {
        let ranges = vec![2..5, 8..12];
        let clipped = clip_ranges(&ranges, 20);
        assert_eq!(clipped, vec![2..5, 8..12]);
    }

    #[test]
    fn clip_ranges_partial() {
        let ranges = vec![2..5, 8..12];
        let clipped = clip_ranges(&ranges, 10);
        assert_eq!(clipped, vec![2..5, 8..10]);
    }

    #[test]
    fn clip_ranges_fully_outside() {
        let ranges = vec![8..12];
        let clipped = clip_ranges(&ranges, 5);
        assert!(clipped.is_empty());
    }

    #[test]
    fn no_color_string_with_brackets() {
        let result = build_no_color_string("org/api-gateway", &[4..7, 8..12], false);
        assert_eq!(result, "org/[api]-[gate]way");
    }

    #[test]
    fn no_color_string_truncated() {
        let result = build_no_color_string("hello", &[], true);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn no_color_string_no_ranges() {
        let result = build_no_color_string("org/api-gateway", &[], false);
        assert_eq!(result, "org/api-gateway");
    }

    #[test]
    fn sort_by_score_dc_orders_correctly() {
        let candidates = vec![
            DisplayCandidate {
                project_index: 0,
                display_text: "foo-bar".into(),
                is_alias: false,
                alias_source_path: None,
                disambiguation: None,
            },
            DisplayCandidate {
                project_index: 1,
                display_text: "foo".into(),
                is_alias: false,
                alias_source_path: None,
                disambiguation: None,
            },
            DisplayCandidate {
                project_index: 2,
                display_text: "foo-baz".into(),
                is_alias: false,
                alias_source_path: None,
                disambiguation: None,
            },
        ];

        let mut matches: Vec<filter::FilterMatch> = vec![
            filter::FilterMatch {
                index: 0,
                highlight_ranges: vec![],
            },
            filter::FilterMatch {
                index: 1,
                highlight_ranges: vec![],
            },
            filter::FilterMatch {
                index: 2,
                highlight_ranges: vec![],
            },
        ];

        let scorer = |idx: usize, _query: &str| -> (f64, f64) {
            match idx {
                0 => (3.0 / 7.0, 0.0),
                1 => (1.0, 0.0),
                2 => (3.0 / 7.0, 0.0),
                _ => (0.0, 0.0),
            }
        };

        sort_by_score_dc(&mut matches, &candidates, "foo", &scorer);

        assert_eq!(candidates[matches[0].index].project_index, 1);
        assert_eq!(candidates[matches[1].index].project_index, 0);
        assert_eq!(candidates[matches[2].index].project_index, 2);
    }
}
