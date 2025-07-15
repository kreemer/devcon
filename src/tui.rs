// MIT License
//
// Copyright (c) 2025 DevCon Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crossterm::event::{self, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListState};

use crate::config::AppConfig;

pub struct TuiApp {
    list_state: ListState,
    pub should_quit: bool,
    pub selected: bool,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            should_quit: false,
            selected: false,
        }
    }

    pub fn run(&mut self, config: &AppConfig) -> Result<Option<usize>, Box<dyn std::error::Error>> {
        if config.recent_paths.is_empty() {
            return Err("No recent paths found in the configuration.".into());
        }

        let mut terminal = ratatui::init();

        let result = loop {
            terminal.draw(|frame| self.render(frame, config))?;

            if let Some(key) = event::read()?.as_key_press_event() {
                match self.handle_key_event(key.code) {
                    Some(selection) => break Ok(Some(selection)),
                    None if self.should_quit => break Ok(None),
                    None => continue,
                }
            }
        };

        ratatui::restore();
        result
    }

    fn handle_key_event(&mut self, key_code: KeyCode) -> Option<usize> {
        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.list_state.select_next();
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.list_state.select_previous();
                None
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                None
            }
            KeyCode::Enter => {
                self.selected = true;
                self.list_state.selected()
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, config: &AppConfig) {
        let constraints = [Constraint::Length(3), Constraint::Fill(1)];
        let layout = Layout::vertical(constraints).split(frame.area());

        self.render_header(frame, layout[0]);
        self.render_list(frame, layout[1], config);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from_iter([Span::from("DevCon - Development Container Manager").bold()]);

        let help = Line::from_iter([
            Span::from("Use "),
            Span::from("↑/k").bold(),
            Span::from(" and "),
            Span::from("↓/j").bold(),
            Span::from(" to navigate, "),
            Span::from("Enter").bold(),
            Span::from(" to select, "),
            Span::from("q/Esc").bold(),
            Span::from(" to quit"),
        ]);

        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        frame.render_widget(title.centered(), layout[0]);
        frame.render_widget(help.centered(), layout[1]);
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect, config: &AppConfig) {
        let items: Vec<String> = config
            .recent_paths
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let display_name = path.to_string_lossy();

                format!("{:2}. {}", i + 1, display_name)
            })
            .collect();

        let list = List::new(items)
            .style(Color::White)
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_app_creation() {
        let app = TuiApp::new();
        assert!(!app.should_quit);
        assert!(!app.selected);
    }

    #[test]
    fn test_handle_key_event_navigation() {
        let mut app = TuiApp::new();

        // Test down navigation
        assert_eq!(app.handle_key_event(KeyCode::Down), None);
        assert!(!app.should_quit);

        // Test up navigation
        assert_eq!(app.handle_key_event(KeyCode::Up), None);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_handle_key_event_quit() {
        let mut app = TuiApp::new();

        assert_eq!(app.handle_key_event(KeyCode::Char('q')), None);
        assert!(app.should_quit);
    }

    #[test]
    fn test_run_with_empty_config() {
        let mut app = TuiApp::new();
        let empty_config = AppConfig {
            recent_paths: vec![],
            dotfiles_repo: None,
            additional_features: std::collections::HashMap::new(),
            env: vec![],
        };

        let result = app.run(&empty_config);
        assert!(result.is_err());
    }
}
