//! In-block `Sub` helpers: a `Sub` inside a Screen/Window/Page lowers to a method
//! on the state struct (direct field access), and a call to it — from an event or
//! another helper — routes to the state receiver. This lets events share logic
//! without hoisting it to a module `Sub` and threading every state field.

use vbr::compile;

const SCREEN: &str = r#"Screen Board
    State
        Dim log As String = ""
        Dim moves As Long = 0
    End State
    Sub Note(ByVal text As String)
        log = log & text
    End Sub
    Sub Play(ByVal who As String)
        moves = moves + 1
        Note(who)
    End Sub
    View
        Text log
    End View
    On Key "x" MoveX
    On Key "q" Quit
    Event MoveX
        Play("X")
    End Event
End Screen
Function Main()
    Board.Run
End Function
"#;

/// Whitespace-stripped Rust, so assertions don't depend on rustfmt.
fn packed(rust: &str) -> String {
    rust.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn sub_lowers_to_a_state_method() {
    let c = compile(SCREEN);
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    let r = packed(&c.rust);
    // The helpers are methods on the state struct, with field access via `self`.
    assert!(r.contains("fnnote(&mutself,text:&str)"), "note method: {}", c.rust);
    assert!(r.contains("fnplay(&mutself,who:&str)"), "play method: {}", c.rust);
    // State fields are reached through `self` inside a helper.
    assert!(r.contains("self.moves=self.moves+1"), "field access in a sub: {}", c.rust);
}

#[test]
fn a_sub_can_call_another_sub() {
    let c = compile(SCREEN);
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    // `Note(who)` inside `Play` routes to the receiver: `self.note(...)`.
    assert!(packed(&c.rust).contains("self.note("), "sub-to-sub call: {}", c.rust);
}

#[test]
fn an_event_call_routes_to_the_state() {
    let c = compile(SCREEN);
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    // `Play("X")` in the `MoveX` event → `state.play("X")` (TUI receiver).
    assert!(packed(&c.rust).contains("state.play("), "event-to-sub call: {}", c.rust);
}
