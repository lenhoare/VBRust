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
            let pw = bounds.size().width.max(1.0).round() as u32;
            let ph = bounds.size().height.max(1.0).round() as u32;
            let mut pix = vec![0u8; pw as usize * ph as usize * 4];
            let mut pix_dirty = false;
            let width = pw as i32;
            let height = ph as i32;
            let _ = (width, height);
            let __vbr_draw: Result<(), String> = (|| {
                { let __cx = ((320) as f32) as i32; let __cy = ((240) as f32) as i32; let __cr = ((self.radius) as f32) as i32; let __cc = iced::Color::from_rgb8(0, 255, 255); let __rr = __cr.max(0); let mut __dy = -__rr; while __dy <= __rr { let __w = ((__rr * __rr - __dy * __dy) as f32).sqrt() as i32; let mut __dx = -__w; while __dx <= __w { let __px = __cx + __dx; let __py = __cy + __dy; if __px >= 0 && __py >= 0 && (__px as u32) < pw && (__py as u32) < ph { let __i = ((__py as u32 * pw + __px as u32) * 4) as usize; pix[__i] = (__cc.r * 255.0) as u8; pix[__i + 1] = (__cc.g * 255.0) as u8; pix[__i + 2] = (__cc.b * 255.0) as u8; pix[__i + 3] = 255; pix_dirty = true; } __dx += 1; } __dy += 1; } }
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", format!("radius = {}", self.radius)), position: iced::Point::new((16) as f32, (20) as f32), color: iced::Color::from_rgb8(255, 255, 255), ..Default::default() });
                Ok(())
            })();
            if let Err(__e) = __vbr_draw {
                eprintln!("Error: {}", __e);
            }
            if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); }
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
