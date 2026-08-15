// A Sketch is a window that is the drawing — no buttons, no View.

use iced::Element;

struct Circles;

impl Default for Circles {
    fn default() -> Self { Circles }
}

fn update(_state: &mut Circles, _message: ()) {}

fn view(state: &Circles) -> Element<'_, ()> {
    let _ = state;
    iced::widget::Canvas::new(CirclesCanvas).width(iced::Length::Fill).height(iced::Length::Fill).into()
}

struct CirclesCanvas;

impl<Message> iced::widget::canvas::Program<Message> for CirclesCanvas {
    type State = ();
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        {
            let frame = &mut frame;
            let _ = &frame;
            frame.fill(&iced::widget::canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size()), iced::Color::from_rgb8(0, 0, 128));
            frame.fill(&iced::widget::canvas::Path::circle(iced::Point::new((320) as f32, (240) as f32), (120) as f32), iced::Color::from_rgb8(0, 255, 255));
            frame.stroke(&iced::widget::canvas::Path::circle(iced::Point::new((320) as f32, (240) as f32), (180) as f32), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(255, 255, 255)).with_width((2) as f32));
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", "a sketch"), position: iced::Point::new((20) as f32, (24) as f32), color: iced::Color::from_rgb8(255, 255, 255), ..Default::default() });
        }
        vec![frame.into_geometry()]
    }
}

fn main() -> iced::Result {
    iced::application("Circles", update, view)
        .window_size(iced::Size::new(640.0, 480.0))
        .run()
}
