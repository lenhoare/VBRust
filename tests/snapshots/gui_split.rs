// Resize-only split panes. Drag the divider; the panes stay put.
// Nested Split is a grid inside a pane.

use iced::widget::{column, text};
use iced::Element;

struct Panes {
    status: String,
    __pg0: iced::widget::pane_grid::State<u8>,
    __pg1: iced::widget::pane_grid::State<u8>,
}

impl Default for Panes {
    fn default() -> Self {
        let status = "drag the divider".to_string();
        let __pg0 = iced::widget::pane_grid::State::with_configuration(
            iced::widget::pane_grid::Configuration::Split {
                axis: iced::widget::pane_grid::Axis::Vertical,
                ratio: 0.32f32,
                a: Box::new(iced::widget::pane_grid::Configuration::Pane(0u8)),
                b: Box::new(iced::widget::pane_grid::Configuration::Pane(1u8)),
            },
        );
        let __pg1 = iced::widget::pane_grid::State::with_configuration(
            iced::widget::pane_grid::Configuration::Split {
                axis: iced::widget::pane_grid::Axis::Horizontal,
                ratio: 0.45f32,
                a: Box::new(iced::widget::pane_grid::Configuration::Pane(0u8)),
                b: Box::new(iced::widget::pane_grid::Configuration::Pane(1u8)),
            },
        );
        Panes {
            status,
            __pg0,
            __pg1,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SplitResized0(iced::widget::pane_grid::ResizeEvent),
    SplitResized1(iced::widget::pane_grid::ResizeEvent),
}

fn update(state: &mut Panes, message: Message) {
    match message {
        Message::SplitResized0(iced::widget::pane_grid::ResizeEvent { split, ratio }) => {
            state.__pg0.resize(split, ratio);
        }
        Message::SplitResized1(iced::widget::pane_grid::ResizeEvent { split, ratio }) => {
            state.__pg1.resize(split, ratio);
        }
    }
}

fn view(state: &Panes) -> Element<'_, Message> {
    column![
        iced::widget::container({
            let el: Element<'_, Message> = iced::widget::PaneGrid::new(&state.__pg0, |_, slot, _| {
                iced::widget::pane_grid::Content::new(match *slot {
                    0 => iced::widget::container(column![
                    text("Left"),
                    text("sidebar"),
                ]).padding(10).width(iced::Length::Fill).style(|theme: &iced::Theme| iced::widget::container::bordered_box(theme).background(theme.extended_palette().background.base.color)).into(),
                    1 => {
                    let el: Element<'_, Message> = iced::widget::PaneGrid::new(&state.__pg1, |_, slot, _| {
                        iced::widget::pane_grid::Content::new(match *slot {
                            0 => iced::widget::container(column![
                            text("Top"),
                            text("edit"),
                        ]).padding(10).width(iced::Length::Fill).style(|theme: &iced::Theme| iced::widget::container::bordered_box(theme).background(theme.extended_palette().background.base.color)).into(),
                            1 => iced::widget::container(column![
                            text("Bottom"),
                            text(format!("{}", state.status)),
                        ]).padding(10).width(iced::Length::Fill).style(|theme: &iced::Theme| iced::widget::container::bordered_box(theme).background(theme.extended_palette().background.base.color)).into(),
                            _ => { let empty: Element<'_, Message> = iced::widget::text("").into(); empty },
                        })
                    }).spacing(4).on_resize(10, Message::SplitResized1).into();
                    el
                },
                    _ => { let empty: Element<'_, Message> = iced::widget::text("").into(); empty },
                })
            }).spacing(4).on_resize(10, Message::SplitResized0).into();
            el
        }).height(iced::Length::Fill),
        text(format!("{}", state.status)),
    ].padding(16).into()
}

fn main() -> iced::Result {
    iced::application("Panes", update, view)
        .theme(|_| iced::Theme::custom_with_fn(String::from("JellyFish"), iced::theme::Palette { background: iced::Color::from_rgb8(11, 19, 43), text: iced::Color::from_rgb8(234, 246, 255), primary: iced::Color::from_rgb8(255, 110, 180), success: iced::Color::from_rgb8(94, 234, 212), danger: iced::Color::from_rgb8(255, 93, 143) }, |p| { let mut e = iced::theme::palette::Extended::generate(p); let ink = iced::Color::BLACK; e.primary.base.text = ink; e.primary.strong.text = ink; e.primary.weak.text = ink; let navy = p.background; let paper = p.text; e.secondary.base.color = navy; e.secondary.base.text = paper; e.secondary.weak.color = navy; e.secondary.weak.text = paper; e.secondary.strong.color = iced::Color::from_rgb8(22, 36, 72); e.secondary.strong.text = paper; e }))
        .run()
}
