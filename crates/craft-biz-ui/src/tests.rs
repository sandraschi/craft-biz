//! Headless UI tests: real egui frames, simulated mouse and keyboard.

use crate::kanban::{self, Board, Card, Column, Event, Lane};
use crate::{shell, theme, Tone};
use egui::{Key, Modifiers, Pos2};
use egui_kittest::Harness;

struct St {
    cards: Vec<Card>,
    events: Vec<Event>,
    selected: Option<String>,
    lanes: bool,
}

fn card(id: &str, col: &str, lane: &str) -> Card {
    Card {
        id: id.into(),
        column: col.into(),
        lane: lane.into(),
        key: id.to_uppercase(),
        title: format!("Task {id} with a title long enough to wrap on a card"),
        tags: vec![("backend".into(), Tone::Info)],
        assignee: Some("Anna Mockova".into()),
        meta: vec!["due 12.11.".into()],
        ..Default::default()
    }
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("todo", "To do", Tone::Neutral),
        Column::new("doing", "In progress", Tone::Accent).wip(2),
        Column::new("review", "Review", Tone::Violet),
        Column::new("done", "Done", Tone::Ok),
    ]
}

/// Workflow: nothing jumps from To do straight to Done.
fn rule(c: &Card, to: &str) -> Result<(), String> {
    if c.column == "todo" && to == "done" {
        Err("start the work first (To do -> In progress)".into())
    } else {
        Ok(())
    }
}

fn harness(lanes: bool) -> Harness<'static, St> {
    let st = St {
        cards: vec![
            card("a", "todo", ""),
            card("b", "todo", ""),
            card("c", "doing", ""),
        ],
        events: Vec::new(),
        selected: None,
        lanes,
    };
    let cols = columns();
    let mut fonts_ready = false;
    let lane_list = vec![Lane {
        id: "".into(),
        title: "Everyone".into(),
    }];
    Harness::builder()
        .with_size([1300.0, 800.0])
        .build_ui_state(
            move |ui, st: &mut St| {
                // Fonts take effect on the next frame, so the first frame draws nothing.
                if !fonts_ready {
                    theme::install_fonts(ui.ctx());
                    theme::apply(ui.ctx(), false);
                    fonts_ready = true;
                    return;
                }
                let r: kanban::Rule = &rule;
                let mut b = Board::new("test", &cols, &st.cards)
                    .rule(r)
                    .selected(st.selected.as_deref());
                if st.lanes {
                    b = b.lanes(&lane_list);
                }
                let evs = b.show(ui);
                for e in evs {
                    match &e {
                        Event::Moved { .. } => kanban::apply_move(&mut st.cards, &e),
                        Event::Select(id) => st.selected = Some(id.clone()),
                        _ => {}
                    }
                    st.events.push(e);
                }
            },
            st,
        )
}

fn drag(h: &mut Harness<'static, St>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(1);
    for i in 1..=8 {
        let f = i as f32 / 8.0;
        h.hover_at(from + (to - from) * f);
        h.run_steps(1);
    }
    h.drop_at(to);
    h.run_steps(3);
}

#[test]
fn drag_card_to_allowed_column_moves_it() {
    let mut h = harness(false);
    h.run_steps(2);
    let a = kanban::card_rect(&h.ctx, "test", "a").unwrap();
    let review = kanban::cell_rect(&h.ctx, "test", "review", "").unwrap();
    drag(&mut h, a.center(), review.center());
    let st = h.state();
    assert!(
        st.events
            .iter()
            .any(|e| matches!(e, Event::Moved { card, to, .. } if card == "a" && to == "review")),
        "{:?}",
        st.events
    );
    assert_eq!(
        st.cards
            .iter()
            .find(|c| c.id == "a")
            .map(|c| c.column.as_str()),
        Some("review")
    );
}

#[test]
fn forbidden_drop_is_refused_with_reason_and_nothing_moves() {
    let mut h = harness(false);
    h.run_steps(2);
    let a = kanban::card_rect(&h.ctx, "test", "a").unwrap();
    let done = kanban::cell_rect(&h.ctx, "test", "done", "").unwrap();
    drag(&mut h, a.center(), done.center());
    let st = h.state();
    assert!(st.events.iter().any(|e| matches!(e, Event::Refused { card, to, reason } if card == "a" && to == "done" && reason.contains("start the work"))), "{:?}", st.events);
    assert_eq!(
        st.cards
            .iter()
            .find(|c| c.id == "a")
            .map(|c| c.column.as_str()),
        Some("todo")
    );
}

#[test]
fn drop_position_sets_order_within_a_column() {
    let mut h = harness(false);
    h.run_steps(2);
    let b = kanban::card_rect(&h.ctx, "test", "b").unwrap();
    let a = kanban::card_rect(&h.ctx, "test", "a").unwrap();
    drag(&mut h, b.center(), Pos2::new(a.center().x, a.top() + 4.0));
    let order: Vec<&str> = h
        .state()
        .cards
        .iter()
        .filter(|c| c.column == "todo")
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(order, vec!["b", "a"], "{:?}", h.state().events);
}

#[test]
fn click_selects_and_keyboard_moves_respect_rules() {
    let mut h = harness(true);
    h.run_steps(2);
    let c = kanban::card_rect(&h.ctx, "test", "c").unwrap();
    h.hover_at(c.center());
    h.run_steps(1);
    h.drag_at(c.center());
    h.run_steps(1);
    h.drop_at(c.center());
    h.run_steps(2);
    assert_eq!(
        h.state().selected.as_deref(),
        Some("c"),
        "{:?}",
        h.state().events
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(
        h.state()
            .cards
            .iter()
            .find(|x| x.id == "c")
            .map(|x| x.column.as_str()),
        Some("review")
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(2);
    assert!(h.state().events.contains(&Event::Undo));
}

#[test]
fn ctrl_wheel_zooms_and_is_clamped() {
    let mut s = shell::UiSettings::default();
    let mut h = Harness::builder().with_size([400.0, 300.0]).build_ui_state(
        move |ui, z: &mut f32| {
            if let Some(n) = shell::handle_zoom(ui.ctx(), &mut s) {
                *z = n;
            }
            ui.label("zoom");
        },
        1.0f32,
    );
    h.run_steps(1);
    h.event(egui::Event::Zoom(1.25));
    h.run_steps(2);
    assert!((h.state() - 1.25).abs() < 1e-3, "zoom {}", h.state());
    for _ in 0..20 {
        h.event(egui::Event::Zoom(2.0));
        h.run_steps(1);
    }
    assert!((h.state() - shell::ZOOM_MAX).abs() < 1e-3);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
    h.run_steps(2);
    assert!((h.state() - 1.0).abs() < 1e-3, "Ctrl+0 resets");
}

#[test]
fn apply_move_inserts_at_index() {
    let mut cards = vec![
        card("a", "todo", ""),
        card("b", "todo", ""),
        card("c", "doing", ""),
    ];
    kanban::apply_move(
        &mut cards,
        &Event::Moved {
            card: "c".into(),
            from: "doing".into(),
            to: "todo".into(),
            from_lane: "".into(),
            to_lane: "".into(),
            index: 1,
        },
    );
    let order: Vec<&str> = cards
        .iter()
        .filter(|c| c.column == "todo")
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(order, vec!["a", "c", "b"]);
}
