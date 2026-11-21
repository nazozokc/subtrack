//! Interactive line-based prompts.
//!
//! The native CLI used to skip every guided flow because the port only
//! implemented the flag paths. These helpers restore the prompts while
//! staying dependency-free and cross-platform: they read whole lines from
//! stdin instead of switching the terminal into raw mode.
//!
//! Every prompt flushes the buffered logger first (`crate::flush()`), so the
//! banner and any content printed before the question is visible before the
//! user is asked to type.

use std::io::{self, BufRead, IsTerminal, Write};

use crate::ansi;

/// A selectable entry. `value` is what the caller receives; `label` is shown.
#[derive(Clone, Debug)]
pub struct Choice {
    pub label: String,
    pub value: String,
}

impl Choice {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
        }
    }

    /// Choice whose label and value are the same string.
    pub fn plain(value: &str) -> Self {
        Self::new(value, value)
    }
}

/// True when prompts may read from the terminal.
///
/// `SUBTRACK_ASSUME_TTY=1` forces prompting for automated tests (a test can
/// pipe answers in); without it only a real TTY is considered interactive so
/// the scriptable paths keep their old non-blocking behavior.
pub fn interactive() -> bool {
    if std::env::var_os("SUBTRACK_ASSUME_TTY").is_some() {
        return true;
    }
    io::stdin().is_terminal()
}

fn out(text: &str) {
    // flush pending logger output before drawing the prompt
    crate::flush();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(text.as_bytes());
    let _ = handle.flush();
}

fn ask(message: &str) -> Option<String> {
    out(&format!("{} {} \u{203a} ", ansi::cyan("?"), message));
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_string()),
    }
}

/// Confirm a field without echoing an empty (optional) value.
fn finish_only(message: &str) {
    out(&format!("{} {}\n", ansi::green("\u{2714}"), message));
}

/// Long choice lists are capped: a short list is scannable, a long one is
/// searched by typing part of a label (see `select`).
const MAX_VISIBLE_CHOICES: usize = 10;

fn choice_matches(choice: &Choice, needle: &str) -> bool {
    choice.label.to_lowercase().contains(needle) || choice.value.to_lowercase().contains(needle)
}

fn choice_equals(choice: &Choice, needle: &str) -> bool {
    choice.value.to_lowercase() == needle || choice.label.to_lowercase() == needle
}

fn finish(message: &str, value: &str) {
    out(&format!(
        "{} {} {}\n",
        ansi::green("\u{2714}"),
        ansi::dim(message),
        ansi::green(value)
    ));
}

fn cancel(message: &str) {
    out(&format!("{} {}\n", ansi::red("\u{2716}"), message));
}

/// Render a numbered choice list.
///
/// Numbers are dimmed and right-aligned so they line up on long lists, and
/// the pre-selected choice carries a `›` pointer — the "(default)" suffix in
/// the question leaves the default discoverable without scanning list text.
/// `visible` holds indices into `choices`; `highlight` is a position in
/// `visible`, so filtered lists can be renumbered from 1.
fn render_numbered_choices(choices: &[Choice], visible: &[usize], highlight: Option<usize>) {
    let number_width = visible.len().to_string().len();
    for (position, &index) in visible.iter().enumerate() {
        let choice = &choices[index];
        let is_pointer = Some(position) == highlight;
        let gutter = if is_pointer {
            format!("{} ", ansi::cyan("\u{203a}"))
        } else {
            "  ".to_string()
        };
        let label = if is_pointer {
            ansi::bold(&choice.label)
        } else {
            choice.label.clone()
        };
        let number = format!(
            "{number:>width$}",
            number = position + 1,
            width = number_width + 1
        );
        out(&format!("{gutter}{}) {}\n", ansi::dim(&number), label));
    }
}

/// Full list used by `checkbox` (short lists: subscriptions, trials, fields).
fn render_choices(choices: &[Choice], highlight: Option<usize>) {
    let indices: Vec<usize> = (0..choices.len()).collect();
    render_numbered_choices(choices, &indices, highlight);
}

/// Render the (possibly filtered, capped) choices used by `select`.
fn render_select_choices(
    choices: &[Choice],
    visible: &[usize],
    highlight: Option<usize>,
    filtered: bool,
) {
    let shown = visible.len().min(MAX_VISIBLE_CHOICES);
    render_numbered_choices(choices, &visible[..shown], highlight);
    if visible.len() > MAX_VISIBLE_CHOICES {
        out(&format!(
            "  … {} more — type to narrow down\n",
            visible.len() - MAX_VISIBLE_CHOICES
        ));
    }
    if filtered && visible.len() <= MAX_VISIBLE_CHOICES {
        out(&format!("  ({} matching)\n", visible.len()));
    }
}

/// Prompt for a line of text.
///
/// `default` is returned when the user submits an empty line. Validation
/// failures re-ask instead of aborting. `None` means EOF with no default, so
/// the caller can abort the workflow quietly (matching the piped-in behavior
/// of the TypeScript implementation).
pub type Validator = fn(&str) -> Result<(), String>;

pub fn input(message: &str, default: Option<&str>, validate: Option<Validator>) -> Option<String> {
    if !interactive() {
        return default.map(|d| d.to_string());
    }

    let hint = match default {
        Some(d) if !d.is_empty() => format!("{} [{}]", message, d),
        _ => message.to_string(),
    };

    loop {
        let answer = ask(&hint)?;
        let value = if answer.is_empty() {
            match default {
                Some(d) => d.to_string(),
                None => {
                    cancel("A value is required");
                    continue;
                }
            }
        } else {
            answer
        };

        if let Some(check) = validate
            && let Err(error) = check(&value)
        {
            cancel(&error);
            continue;
        }

        if value.is_empty() {
            finish_only(message);
        } else {
            finish(message, &value);
        }
        return Some(value);
    }
}

/// Pick one entry from a numbered list.
///
/// Short lists are shown in full; long ones are capped at
/// `MAX_VISIBLE_CHOICES` and can be narrowed by typing part of a label. An
/// exact value/label (or a unique substring match) selects immediately; a
/// query matching several entries re-renders just those entries, numbered
/// from 1. Empty input clears the filter, then falls back to `default`.
pub fn select(message: &str, choices: &[Choice], default: Option<usize>) -> Option<String> {
    if choices.is_empty() {
        return None;
    }
    if !interactive() {
        return default
            .filter(|i| *i < choices.len())
            .map(|i| choices[i].value.clone());
    }

    let mut filter: Option<String> = None;

    loop {
        let visible: Vec<usize> = match &filter {
            Some(needle) => choices
                .iter()
                .enumerate()
                .filter(|(_, c)| choice_matches(c, needle))
                .map(|(i, _)| i)
                .collect(),
            None => (0..choices.len()).collect(),
        };
        if visible.is_empty() {
            filter = None;
            cancel("No matches — showing all choices");
            continue;
        }

        let highlight = if filter.is_none() {
            default.filter(|i| *i < choices.len())
        } else {
            None
        };
        render_select_choices(choices, &visible, highlight, filter.is_some());

        let hint = match &filter {
            Some(needle) => format!(
                "{} [{} matching \"{}\"] (empty = show all)",
                message,
                visible.len(),
                needle
            ),
            None => match highlight {
                Some(i) => format!(
                    "{} [1-{}] (default {}, type to filter)",
                    message,
                    choices.len(),
                    i + 1
                ),
                None => format!("{} [1-{}] (type to filter)", message, choices.len()),
            },
        };

        let answer = ask(&hint)?;
        let trimmed = answer.trim();

        if trimmed.is_empty() {
            if filter.is_some() {
                filter = None;
                continue;
            }
            if let Some(i) = highlight {
                finish(message, &choices[i].label);
                return Some(choices[i].value.clone());
            }
            cancel("Please choose a number");
            continue;
        }

        if let Ok(number) = trimmed.parse::<usize>() {
            if let Some(&index) = visible.get(number.saturating_sub(1)) {
                let choice = &choices[index];
                finish(message, &choice.label);
                return Some(choice.value.clone());
            }
            cancel("Invalid choice");
            continue;
        }

        let needle = trimmed.to_lowercase();
        if let Some(index) = choices.iter().position(|c| choice_equals(c, &needle)) {
            let choice = &choices[index];
            finish(message, &choice.label);
            return Some(choice.value.clone());
        }

        let matches: Vec<usize> = choices
            .iter()
            .enumerate()
            .filter(|(_, c)| choice_matches(c, &needle))
            .map(|(i, _)| i)
            .collect();
        match matches.len() {
            1 => {
                let choice = &choices[matches[0]];
                finish(message, &choice.label);
                return Some(choice.value.clone());
            }
            n if n > 1 => filter = Some(needle),
            _ => cancel("Invalid choice"),
        }
    }
}

/// Yes/no confirmation. `None` means EOF/cancel, never an implicit answer.
pub fn confirm(message: &str, default: bool) -> Option<bool> {
    if !interactive() {
        return None;
    }
    let hint = if default {
        format!("{} (Y/n)", message)
    } else {
        format!("{} (y/N)", message)
    };

    loop {
        let answer = ask(&hint)?;
        let value = match answer.trim().to_lowercase().as_str() {
            "" => default,
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => {
                cancel("Please answer y or n");
                continue;
            }
        };
        finish(message, if value { "yes" } else { "no" });
        return Some(value);
    }
}

/// Pick zero or more entries by number (`1,3`), name, or `all`.
///
/// An empty answer returns an empty selection (caller treats it as a cancel).
pub fn checkbox(message: &str, choices: &[Choice]) -> Option<Vec<String>> {
    if choices.is_empty() {
        return Some(Vec::new());
    }
    if !interactive() {
        return None;
    }

    render_choices(choices, None);

    loop {
        let answer = ask(&format!("{} (numbers, 'all', empty = cancel)", message))?;
        let trimmed = answer.trim();
        if trimmed.is_empty() {
            finish(message, "none");
            return Some(Vec::new());
        }
        if trimmed.eq_ignore_ascii_case("all") {
            let values = choices.iter().map(|c| c.value.clone()).collect();
            finish(message, "all");
            return Some(values);
        }

        let mut selected: Vec<String> = Vec::new();
        let mut ok = true;
        for part in trimmed.split([',', ' ']).filter(|p| !p.trim().is_empty()) {
            let part = part.trim();
            let found = if let Ok(number) = part.parse::<usize>() {
                if number >= 1 && number <= choices.len() {
                    Some(&choices[number - 1])
                } else {
                    None
                }
            } else {
                let needle = part.to_lowercase();
                choices
                    .iter()
                    .find(|c| c.value.to_lowercase() == needle || c.label.to_lowercase() == needle)
            };
            match found {
                Some(choice) => {
                    if !selected.contains(&choice.value) {
                        selected.push(choice.value.clone());
                    }
                }
                None => {
                    cancel(&format!("Invalid choice: {}", part));
                    ok = false;
                    break;
                }
            }
        }

        if !ok {
            continue;
        }
        let labels: Vec<String> = choices
            .iter()
            .filter(|c| selected.contains(&c.value))
            .map(|c| c.label.clone())
            .collect();
        finish(message, &labels.join(", "));
        return Some(selected);
    }
}

/// Convenience: a labelled subscription picker used by several commands.
pub fn pick_subscription(
    message: &str,
    subs: &[crate::models::Subscription],
    include_archived: bool,
) -> Option<i64> {
    let choices: Vec<Choice> = subs
        .iter()
        .filter(|s| include_archived || !crate::domain::is_archived(&s.status))
        .map(|s| {
            let tags = if s.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", s.tags.join(", "))
            };
            let status = if crate::domain::is_active(&s.status) {
                String::new()
            } else {
                format!(" ({})", s.status)
            };
            Choice::new(
                &format!(
                    "#{} {} \u{2014} {}/{}{}{}",
                    s.id,
                    s.name,
                    crate::price::format_price(s.price, &s.currency),
                    s.cycle,
                    tags,
                    status
                ),
                &s.id.to_string(),
            )
        })
        .collect();
    if choices.is_empty() {
        crate::logger::info("No subscriptions found \u{2014} try `subtrack add`");
        return None;
    }
    let value = select(message, &choices, None)?;
    value.parse::<i64>().ok()
}
