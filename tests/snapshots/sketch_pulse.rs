// An animated Sketch: Every ticks an Event, Draw paints from State.

use iced::Element;

struct Pulse {
    radius: i32,
    growing: bool,
}

impl Default for Pulse {
    fn default() -> Self {
        let radius = 20;
        let growing = true;
        Pulse {
            radius,
            growing,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
}

fn update(state: &mut Pulse, message: Message) {
    match message {
        Message::Tick => {
            {
                let __vbr_event: Result<(), String> = (|| {
                    if state.growing {
                        state.radius = state.radius + 2;
                        if state.radius >= 160 {
                            state.growing = false;
                        }
                    } else {
                        state.radius = state.radius - 2;
                        if state.radius <= 20 {
                            state.growing = true;
                        }
                    }
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
    }
}

fn subscription(_state: &Pulse) -> iced::Subscription<Message> {
    iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
}

fn view(state: &Pulse) -> Element<'_, Message> {
    iced::widget::Canvas::new(PulseCanvas { radius: state.radius }).width(iced::Length::Fill).height(iced::Length::Fill).into()
}

struct PulseCanvas {
    radius: i32,
}

impl<Message> iced::widget::canvas::Program<Message> for PulseCanvas {
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
            frame.fill(&iced::widget::canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size()), iced::Color::from_rgb8(0, 0, 0));
            frame.fill(&iced::widget::canvas::Path::circle(iced::Point::new((320) as f32, (240) as f32), (self.radius) as f32), iced::Color::from_rgb8(0, 255, 255));
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", format!("radius = {}", self.radius)), position: iced::Point::new((16) as f32, (20) as f32), color: iced::Color::from_rgb8(255, 255, 255), ..Default::default() });
        }
        vec![frame.into_geometry()]
    }
}

fn main() -> iced::Result {
    iced::application("Pulse", update, view)
        .window_size(iced::Size::new(640.0, 480.0))
        .subscription(subscription)
        .run()
}
