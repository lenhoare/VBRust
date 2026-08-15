// Per-pixel drawing — Set Pixel writes the raster buffer.

use iced::Element;

struct Gradient;

impl Default for Gradient {
    fn default() -> Self { Gradient }
}

fn update(_state: &mut Gradient, _message: ()) {}

fn view(state: &Gradient) -> Element<'_, ()> {
    let _ = state;
    iced::widget::Canvas::new(GradientCanvas).width(iced::Length::Fill).height(iced::Length::Fill).into()
}

struct GradientCanvas;

impl<Message> iced::widget::canvas::Program<Message> for GradientCanvas {
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
                for y in 0..=height - 1 {
                    for x in 0..=width - 1 {
                        { let __px = (x) as i32; let __py = (y) as i32; if __px >= 0 && __py >= 0 && (__px as u32) < pw && (__py as u32) < ph { let __i = ((__py as u32 * pw + __px as u32) * 4) as usize; let __c = iced::Color::from_rgb8((((x * 255) as f64) / (width as f64)) as u8, (((y * 255) as f64) / (height as f64)) as u8, (80) as u8); pix[__i] = (__c.r * 255.0) as u8; pix[__i + 1] = (__c.g * 255.0) as u8; pix[__i + 2] = (__c.b * 255.0) as u8; pix[__i + 3] = 255; pix_dirty = true; } }
                    }
                }
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
    iced::application("Pixels", update, view)
        .window_size(iced::Size::new(400.0, 240.0))
        .run()
}
