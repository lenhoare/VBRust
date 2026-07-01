fn draw_grid(frame: &mut iced::widget::canvas::Frame) {
    for x in (0..=300).step_by(30) {
        frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new((x) as f32, (0) as f32), iced::Point::new((x) as f32, (220) as f32)), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(128, 128, 128)).with_width((1) as f32));
    }
    for y in (0..=220).step_by(30) {
        frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new((0) as f32, (y) as f32), iced::Point::new((300) as f32, (y) as f32)), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(128, 128, 128)).with_width((1) as f32));
    }
}

use iced::widget::{column, slider, text};
use iced::Element;

struct Sketch {
    radius: i32,
}

impl Default for Sketch {
    fn default() -> Self {
        Sketch {
            radius: 40,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Resize(i32),
}

fn update(state: &mut Sketch, message: Message) {
    match message {
        Message::Resize(value) => {
            state.radius = value;
        }
    }
}

fn view(state: &Sketch) -> Element<'_, Message> {
    column![
        text("Drag the slider to resize the circle"),
        slider(10..=120, state.radius, Message::Resize),
        iced::widget::Canvas::new(FaceCanvas { radius: state.radius }).width(iced::Length::Fixed(300.0)).height(iced::Length::Fixed(220.0)),
    ].spacing(10).padding(10).into()
}

struct FaceCanvas {
    radius: i32,
}

impl<Message> iced::widget::canvas::Program<Message> for FaceCanvas {
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
            draw_grid(frame);
            frame.fill(&iced::widget::canvas::Path::circle(iced::Point::new((150) as f32, (110) as f32), (self.radius) as f32), iced::Color::from_rgb8(0, 0, 128));
            frame.stroke(&iced::widget::canvas::Path::circle(iced::Point::new((150) as f32, (110) as f32), (self.radius) as f32), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(255, 255, 255)).with_width((2) as f32));
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", format!("{}{}", "radius = ", self.radius)), position: iced::Point::new((10) as f32, (16) as f32), color: iced::Color::from_rgb8(0, 0, 0), ..Default::default() });
        }
        vec![frame.into_geometry()]
    }
}

fn main() -> iced::Result {
    iced::run("Canvas", update, view)
}
