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
            let pw = bounds.size().width.max(1.0).round() as u32;
            let ph = bounds.size().height.max(1.0).round() as u32;
            let mut pix = vec![0u8; pw as usize * ph as usize * 4];
            let mut pix_dirty = false;
            let width = pw as i32;
            let height = ph as i32;
            let _ = (width, height);
            let __vbr_draw: Result<(), String> = (|| {
                { let __cx = ((320) as f32) as i32; let __cy = ((240) as f32) as i32; let __cr = ((120) as f32) as i32; let __cc = iced::Color::from_rgb8(0, 255, 255); let __rr = __cr.max(0); let mut __dy = -__rr; while __dy <= __rr { let __w = ((__rr * __rr - __dy * __dy) as f32).sqrt() as i32; let mut __dx = -__w; while __dx <= __w { let __px = __cx + __dx; let __py = __cy + __dy; if __px >= 0 && __py >= 0 && (__px as u32) < pw && (__py as u32) < ph { let __i = ((__py as u32 * pw + __px as u32) * 4) as usize; pix[__i] = (__cc.r * 255.0) as u8; pix[__i + 1] = (__cc.g * 255.0) as u8; pix[__i + 2] = (__cc.b * 255.0) as u8; pix[__i + 3] = 255; pix_dirty = true; } __dx += 1; } __dy += 1; } }
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            { let __c = iced::Point::new((320) as f32, (240) as f32); let __r = (180) as f32; let __s = iced::widget::canvas::Stroke::default().with_color(iced::Color::from_rgb8(255, 255, 255)).with_width((2) as f32); let mut __i = 0i32; while __i < 64 { let __a0 = (__i as f32) * std::f32::consts::TAU / 64.0; let __a1 = ((__i + 1) as f32) * std::f32::consts::TAU / 64.0; frame.stroke(&iced::widget::canvas::Path::line(iced::Point::new(__c.x + __r * __a0.cos(), __c.y + __r * __a0.sin()), iced::Point::new(__c.x + __r * __a1.cos(), __c.y + __r * __a1.sin())), __s); __i += 1; } }
                if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix.clone()); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); pix.fill(0); pix_dirty = false; }
            frame.fill_text(iced::widget::canvas::Text { content: format!("{}", "a sketch"), position: iced::Point::new((20) as f32, (24) as f32), color: iced::Color::from_rgb8(255, 255, 255), ..Default::default() });
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
    iced::application("Circles", update, view)
        .window_size(iced::Size::new(640.0, 480.0))
        .run()
}
