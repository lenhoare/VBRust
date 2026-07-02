#[derive(Debug, Clone)]
struct Bar {
    pub x: i32,
    pub h: i32,
}

fn makebars(seed: i32) -> Vec<Bar> {
    let mut result: Vec<Bar> = Vec::new();
    for i in 0..=9 {
        let h: i32 = (i * seed + 7) % 15 * 10 + 20;
        result.push(Bar { x: i * 32 + 14, h: h });
    }
    result
}

use iced::widget::{button, column, text};
use iced::Element;

struct Chart {
    bars: Vec<Bar>,
    seed: i32,
}

impl Default for Chart {
    fn default() -> Self {
        Chart {
            bars: makebars(3),
            seed: 3,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Regenerate,
}

fn update(state: &mut Chart, message: Message) {
    match message {
        Message::Regenerate => {
            state.seed += 1;
            state.bars = makebars(state.seed);
        }
    }
}

fn view(state: &Chart) -> Element<'_, Message> {
    column![
        text("A chart drawn from a Vec of data points"),
        button("Regenerate").on_press(Message::Regenerate),
        iced::widget::Canvas::new(PlotCanvas { bars: state.bars.clone() }).width(iced::Length::Fixed(340.0)).height(iced::Length::Fixed(200.0)),
    ].spacing(10).padding(10).into()
}

struct PlotCanvas {
    bars: Vec<Bar>,
}

impl<Message> iced::widget::canvas::Program<Message> for PlotCanvas {
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
            frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new((10) as f32, (180) as f32), iced::Point::new((330) as f32, (180) as f32)), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(128, 128, 128)).with_width((1) as f32));
            for b in &self.bars {
                frame.fill(&iced::widget::canvas::Path::rectangle(iced::Point::new((b.x) as f32, (180 - b.h) as f32), iced::Size::new((18) as f32, (b.h) as f32)), iced::Color::from_rgb8(0, 0, 128));
            }
        }
        vec![frame.into_geometry()]
    }
}

fn main() -> iced::Result {
    iced::run("Bar Chart", update, view)
}
