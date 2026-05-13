use crate::tui::ui::TuiScreen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiEvent {
    Quit,
    Refresh,
    Switch(TuiScreen),
    None,
}

pub fn parse_event(input: &str) -> TuiEvent {
    match input.trim() {
        "q" | "Q" | "\u{1b}" => TuiEvent::Quit,
        "r" | "R" => TuiEvent::Refresh,
        "1" => TuiEvent::Switch(TuiScreen::Dashboard),
        "2" => TuiEvent::Switch(TuiScreen::Runs),
        "3" => TuiEvent::Switch(TuiScreen::Candidates),
        "4" => TuiEvent::Switch(TuiScreen::Metrics),
        "5" => TuiEvent::Switch(TuiScreen::Release),
        "6" => TuiEvent::Switch(TuiScreen::Logs),
        "7" | "h" | "H" => TuiEvent::Switch(TuiScreen::Help),
        _ => TuiEvent::None,
    }
}
