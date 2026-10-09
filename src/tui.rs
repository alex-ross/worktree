use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};
use worktree_core::{Error, Manager, Result, Worktree};

enum Mode {
    Search,
    Create,
    Fork(Worktree),
    Remove(Worktree),
}

struct Picker {
    trees: Vec<Worktree>,
    query: String,
    input: String,
    selected: usize,
    list: ListState,
    mode: Mode,
    message: String,
    rows: Rect,
}

fn fuzzy(query: &str, candidate: &str) -> Option<usize> {
    let candidate: Vec<_> = candidate.to_lowercase().chars().collect();
    let mut position = 0;
    let mut cost = 0;
    for needle in query.to_lowercase().chars() {
        let gap = candidate[position..].iter().position(|c| *c == needle)?;
        cost += gap;
        position += gap + 1;
    }
    Some(cost)
}

impl Picker {
    fn filtered(&self) -> Vec<usize> {
        let mut matches: Vec<_> = self
            .trees
            .iter()
            .enumerate()
            .filter_map(|(index, tree)| {
                fuzzy(
                    &self.query,
                    &format!("{} {} {}", tree.project, tree.key, tree.path.display()),
                )
                .map(|score| (score, index))
            })
            .collect();
        matches.sort_by_key(|&(score, index)| (score, index));
        matches.into_iter().map(|(_, index)| index).collect()
    }

    fn move_selection(&mut self, down: bool) {
        let count = self.filtered().len();
        if count == 0 {
            self.selected = 0;
        } else if down {
            self.selected = (self.selected + 1) % count;
        } else {
            self.selected = (self.selected + count - 1) % count;
        }
    }

    fn chosen(&self) -> Option<Worktree> {
        self.filtered()
            .get(self.selected)
            .map(|&index| self.trees[index].clone())
    }

    fn render(&mut self, frame: &mut Frame) {
        let areas = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
            Constraint::Length(3),
        ])
        .split(frame.area());
        let title = Line::from(vec![
            Span::styled(
                " WORKTREE ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   projects / branches"),
        ]);
        frame.render_widget(
            Paragraph::new(title).block(Block::default().borders(Borders::BOTTOM)),
            areas[0],
        );
        let filtered = self.filtered();
        self.selected = self.selected.min(filtered.len().saturating_sub(1));
        let items: Vec<_> = filtered
            .iter()
            .map(|&index| {
                let tree = &self.trees[index];
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<20} ", clean(&tree.project)),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{}  ", clean(&tree.key)),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        clean(&tree.path.to_string_lossy()),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(if tree.locked {
                        " [locked]"
                    } else if tree.branch.is_none() {
                        " [detached]"
                    } else {
                        ""
                    }),
                ]))
            })
            .collect();
        self.list.select(if filtered.is_empty() {
            None
        } else {
            Some(self.selected)
        });
        self.rows = areas[1];
        if items.is_empty() {
            frame.render_widget(
                Paragraph::new(" No matches. Ctrl-N creates a worktree in the current project."),
                areas[1],
            );
        } else {
            frame.render_stateful_widget(
                List::new(items)
                    .highlight_symbol(" › ")
                    .highlight_style(Style::default().bg(Color::Rgb(30, 45, 55))),
                areas[1],
                &mut self.list,
            );
        }
        let hint = if self.message.is_empty() {
            " ↑↓ navigate · enter/click open · ctrl-n new · ctrl-f fork · ctrl-d remove · esc quit"
        } else {
            &self.message
        };
        frame.render_widget(
            Paragraph::new(clean(hint)).style(Style::default().fg(Color::DarkGray)),
            areas[2],
        );
        let (label, value) = match &self.mode {
            Mode::Search => (
                format!(" Search · {} / {} ", filtered.len(), self.trees.len()),
                self.query.as_str(),
            ),
            Mode::Create => (
                " New branch · enter to create · esc cancels ".into(),
                self.input.as_str(),
            ),
            Mode::Fork(tree) => (
                format!(" Fork {} · new branch · enter to create ", clean(&tree.key)),
                self.input.as_str(),
            ),
            Mode::Remove(tree) => (
                format!(" Remove {}? Type yes · enter confirms ", clean(&tree.key)),
                self.input.as_str(),
            ),
        };
        frame.render_widget(
            Paragraph::new(format!(" › {}", clean(value))).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(label)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
            areas[3],
        );
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stderr(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

pub fn select(manager: &Manager, cwd: &Path) -> Result<Option<PathBuf>> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(Error(
            "The picker needs a terminal; use 'worktree list' or 'worktree open KEY'".into(),
        ));
    }
    let mut picker = Picker {
        trees: manager.discover(cwd)?,
        query: String::new(),
        input: String::new(),
        selected: 0,
        list: ListState::default(),
        mode: Mode::Search,
        message: String::new(),
        rows: Rect::default(),
    };
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stderr(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stderr()))?;
    loop {
        terminal.draw(|frame| picker.render(frame))?;
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let search = matches!(picker.mode, Mode::Search);
                match key.code {
                    KeyCode::Esc if search => return Ok(None),
                    KeyCode::Esc => {
                        picker.mode = Mode::Search;
                        picker.input.clear();
                        picker.message.clear();
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None);
                    }
                    KeyCode::Up if search => picker.move_selection(false),
                    KeyCode::Down if search => picker.move_selection(true),
                    KeyCode::Char('n')
                        if search && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        picker.mode = Mode::Create;
                        picker.input.clear();
                    }
                    KeyCode::Char('f')
                        if search && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        if let Some(tree) = picker.chosen() {
                            picker.mode = Mode::Fork(tree);
                            picker.input.clear();
                        }
                    }
                    KeyCode::Char('d')
                        if search && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        if let Some(tree) = picker.chosen() {
                            picker.mode = Mode::Remove(tree);
                            picker.input.clear();
                        }
                    }
                    KeyCode::Enter => {
                        let result = match &picker.mode {
                            Mode::Search => {
                                if let Some(tree) = picker.chosen() {
                                    return Ok(Some(tree.path));
                                }
                                continue;
                            }
                            Mode::Create => manager
                                .create(cwd, &picker.input)
                                .map(|tree| Some(tree.path)),
                            Mode::Fork(tree) => manager
                                .fork(&tree.path, &picker.input)
                                .map(|tree| Some(tree.path)),
                            Mode::Remove(tree) if picker.input == "yes" => {
                                manager.remove(tree, false).map(|()| None)
                            }
                            Mode::Remove(_) => {
                                picker.message = " Type yes to confirm, or Esc to cancel".into();
                                continue;
                            }
                        };
                        match result {
                            Ok(Some(path)) => return Ok(Some(path)),
                            Ok(None) => {
                                picker.trees = manager.discover(cwd)?;
                                picker.mode = Mode::Search;
                                picker.message = " Worktree removed; branch kept".into();
                            }
                            Err(error) => {
                                picker.message = error.to_string();
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if search {
                            picker.query.pop();
                            picker.selected = 0;
                        } else {
                            picker.input.pop();
                        }
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if search {
                            picker.query.push(c);
                            picker.selected = 0;
                        } else {
                            picker.input.push(c);
                        }
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) if matches!(picker.mode, Mode::Search) => match mouse.kind {
                MouseEventKind::ScrollDown => picker.move_selection(true),
                MouseEventKind::ScrollUp => picker.move_selection(false),
                MouseEventKind::Down(MouseButton::Left)
                    if picker.rows.contains((mouse.column, mouse.row).into()) =>
                {
                    let index = picker.list.offset() + (mouse.row - picker.rows.y) as usize;
                    if let Some(&tree) = picker.filtered().get(index) {
                        return Ok(Some(picker.trees[tree].path.clone()));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_matches_subsequences_case_insensitively_and_unicode() {
        assert!(fuzzy("wt", "worktree").is_some());
        assert!(fuzzy("ÅB", "återbygg").is_some());
        assert!(fuzzy("xyz", "worktree").is_none());
        assert!(
            fuzzy("foo", "foo").unwrap() < fuzzy("foo", "far away from other objects").unwrap()
        );
        assert_eq!(fuzzy("", "anything"), Some(0));
    }
    #[test]
    fn terminal_control_characters_are_not_rendered() {
        assert_eq!(clean("name\n\x1b[2J"), "name  [2J");
    }
}
