fn drawgrid(frame: &mut iced::widget::canvas::Frame) -> Result<(), String> {
    for x in (0..=300).step_by(30) {
        frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new((x) as f32, (0) as f32), iced::Point::new((x) as f32, (220) as f32)), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(128, 128, 128)).with_width((1) as f32));
    }
    for y in (0..=220).step_by(30) {
        frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new((0) as f32, (y) as f32), iced::Point::new((300) as f32, (y) as f32)), iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(128, 128, 128)).with_width((1) as f32));
    }
    Ok(())
}

use iced::widget::{column, slider, text};
use iced::Element;

struct Sketch {
    radius: i32,
}

impl Default for Sketch {
    fn default() -> Self {
        let radius = 40;
        Sketch {
            radius,
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
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.radius = value;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
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
            let pw = bounds.size().width.max(1.0).round() as u32;
            let ph = bounds.size().height.max(1.0).round() as u32;
            let mut pix = vec![0u8; pw as usize * ph as usize * 4];
            let mut pix_dirty = false;
            let width = pw as i32;
            let height = ph as i32;
            let _ = (width, height);
            let __vbr_draw: Result<(), String> = (|| {
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            drawgrid(frame)?;
                { let __cx = ((150) as f32) as i32; let __cy = ((110) as f32) as i32; let __cr = ((self.radius) as f32) as i32; let __cc = iced::Color::from_rgb8(0, 0, 128); let __rr = __cr.max(0); let mut __dy = -__rr; while __dy <= __rr { let __w = ((__rr * __rr - __dy * __dy) as f32).sqrt() as i32; let mut __dx = -__w; while __dx <= __w { let __px = __cx + __dx; let __py = __cy + __dy; if __px >= 0 && __py >= 0 && (__px as u32) < pw && (__py as u32) < ph { let __i = ((__py as u32 * pw + __px as u32) * 4) as usize; pix[__i] = (__cc.r * 255.0) as u8; pix[__i + 1] = (__cc.g * 255.0) as u8; pix[__i + 2] = (__cc.b * 255.0) as u8; pix[__i + 3] = 255; pix_dirty = true; } __dx += 1; } __dy += 1; } }
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            { let __c = iced::Point::new((150) as f32, (110) as f32); let __r = (self.radius) as f32; let __s = iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(255, 255, 255)).with_width((2) as f32); let mut __i = 0i32; while __i < 64 { let __a0 = (__i as f32) * std::f32::consts::TAU / 64.0; let __a1 = ((__i + 1) as f32) * std::f32::consts::TAU / 64.0; frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new(__c.x + __r * __a0.cos(), __c.y + __r * __a0.sin()), iced::Point::new(__c.x + __r * __a1.cos(), __c.y + __r * __a1.sin())), __s); __i += 1; } }
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", format!("radius = {}", self.radius)), position: iced::Point::new((10) as f32, (16) as f32), color: iced::Color::from_rgb8(0, 0, 0), ..Default::default() });
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
    iced::run("Canvas", update, view)
}
