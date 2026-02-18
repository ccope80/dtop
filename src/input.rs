use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    FocusNext,
    FocusPrev,
    SelectUp,
    SelectDown,
    Confirm,
    Back,
    ScrollUp,
    ScrollDown,
    SmartRefresh,
    CycleSort,
    CycleTheme,
    CyclePreset,
    CycleWindow,   // w: cycle history window (60s/5m/1h) in detail view
    ShowHelp,
    ViewProcessIO,
    ViewFilesystem,
    ViewVolume,
    ViewNfs,       // F5: NFS / network mount latency view
    Benchmark,     // b: run quick read benchmark on selected device
    SmartTest,     // x: schedule SMART short self-test on selected device
    FilterDevices, // f: cycle device type filter (All/NVMe/SSD/HDD)
    None,
}

pub fn handle_key(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _)
        | (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        (KeyCode::Tab, _)     => Action::FocusNext,
        (KeyCode::BackTab, _) => Action::FocusPrev,

        // Navigation — arrow keys and vim hjkl
        (KeyCode::Up,   _) | (KeyCode::Char('k'), _) => Action::SelectUp,
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => Action::SelectDown,

        (KeyCode::Enter, _)     => Action::Confirm,
        (KeyCode::Char('l'), _) => Action::Confirm,   // vim: l = enter/drill-down

        (KeyCode::Esc, _)       => Action::Back,
        (KeyCode::Char('h'), _) => Action::Back,      // vim: h = back/escape

        (KeyCode::PageUp,   _) => Action::ScrollUp,
        (KeyCode::PageDown, _) => Action::ScrollDown,

        // Feature keys
        (KeyCode::Char('s'), _) => Action::CycleSort,    // sort in process / SMART refresh in detail
        (KeyCode::Char('t'), _) => Action::CycleTheme,   // cycle color theme
        (KeyCode::Char('p'), _) => Action::CyclePreset,  // cycle layout preset
        (KeyCode::Char('w'), _) => Action::CycleWindow,  // cycle history window (detail view)
        (KeyCode::Char('?'), _)
        | (KeyCode::F(1), _)   => Action::ShowHelp,      // help overlay

        // View switching
        (KeyCode::F(2), _) => Action::ViewProcessIO,
        (KeyCode::F(3), _) => Action::ViewFilesystem,
        (KeyCode::F(4), _) => Action::ViewVolume,
        (KeyCode::F(5), _) => Action::ViewNfs,

        // Device actions (detail view)
        (KeyCode::Char('b'), _) => Action::Benchmark,
        (KeyCode::Char('x'), _) => Action::SmartTest,
        (KeyCode::Char('f'), _) => Action::FilterDevices,

        _ => Action::None,
    }
}
