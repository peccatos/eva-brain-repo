use std::io::{self, IsTerminal, Write};
use std::path::Path;

use crate::tui::events::{parse_event, TuiEvent};
use crate::tui::state::load_tui_state_from_project_root;
use crate::tui::ui::{render_screen, TuiScreen};

pub fn run_tui(project_root: &str, memory_root: &str) -> Result<String, String> {
    let root = Path::new(project_root);
    let _ = memory_root;
    let state = load_tui_state_from_project_root(root);

    if !io::stdin().is_terminal() {
        return Ok(render_tui_snapshot(&state));
    }

    run_interactive_tui(root)?;
    Ok("tui_status: closed".to_string())
}

pub fn render_tui_snapshot(state: &crate::contracts::TuiState) -> String {
    render_screen(state, TuiScreen::Dashboard, terminal_width())
}

pub fn run_interactive_tui(root: &Path) -> Result<(), String> {
    let mut state = load_tui_state_from_project_root(root);
    let mut screen = TuiScreen::Dashboard;

    loop {
        print!(
            "\x1b[2J\x1b[H{}",
            render_screen(&state, screen, terminal_width())
        );
        print!("\ncommand> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("failed to flush tui: {error}"))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("failed to read tui input: {error}"))?;
        match parse_event(&input) {
            TuiEvent::Quit => return Ok(()),
            TuiEvent::Refresh => state = load_tui_state_from_project_root(root),
            TuiEvent::Switch(next) => screen = next,
            TuiEvent::None => {}
        }
    }
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
}
