//! The Esc rule (PLAN 1.5): one press does the first of these that applies.

/// What is going on when Esc is pressed.
#[derive(Clone, Copy, Debug, Default)]
pub struct EscState {
    /// A tool operation is in progress.
    pub operation: bool,
    /// A menu, popup or window is open.
    pub ui_open: bool,
    /// An opened group or component is being edited.
    pub in_group: bool,
}

/// What Esc does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscAction {
    CancelOperation,
    CloseUi,
    ExitGroup,
    Nothing,
}

pub fn on_esc(s: EscState) -> EscAction {
    if s.operation {
        EscAction::CancelOperation
    } else if s.ui_open {
        EscAction::CloseUi
    } else if s.in_group {
        EscAction::ExitGroup
    } else {
        EscAction::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_that_applies_wins() {
        let all = EscState {
            operation: true,
            ui_open: true,
            in_group: true,
        };
        assert_eq!(on_esc(all), EscAction::CancelOperation);
        assert_eq!(
            on_esc(EscState {
                operation: false,
                ..all
            }),
            EscAction::CloseUi
        );
        assert_eq!(
            on_esc(EscState {
                in_group: true,
                ..Default::default()
            }),
            EscAction::ExitGroup
        );
        assert_eq!(on_esc(EscState::default()), EscAction::Nothing);
    }
}
