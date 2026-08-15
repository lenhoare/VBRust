// The Mandelbrot set, one pixel at a time.

use iced::Element;

struct Mandelbrot {
    maxiter: i32,
}

impl Default for Mandelbrot {
    fn default() -> Self {
        let maxiter = 80;
        Mandelbrot {
            maxiter,
        }
    }
}

fn update(_state: &mut Mandelbrot, _message: ()) {}

fn view(state: &Mandelbrot) -> Element<'_, ()> {
    iced::widget::Canvas::new(MandelbrotCanvas { maxiter: state.maxiter }).width(iced::Length::Fill).height(iced::Length::Fill).into()
}

struct MandelbrotCanvas {
    maxiter: i32,
}

impl<Message> iced::widget::canvas::Program<Message> for MandelbrotCanvas {
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
                        let cre: f64 = -2.2 + (x as f64) * 3.0 / (width as f64);
                        let cim: f64 = -1.2 + (y as f64) * 2.4 / (height as f64);
                        let mut zr: f64 = 0.0;
                        let mut zi: f64 = 0.0;
                        let mut n: i32 = 0;
                        while n < self.maxiter && zr * zr + zi * zi < 4.0 {
                            let tmp: f64 = zr * zr - zi * zi + cre;
                            zi = 2.0 * zr * zi + cim;
                            zr = tmp;
                            n = n + 1;
                        }
                        if n < self.maxiter {
                            let t: i32 = (((n * 255) as f64) / (self.maxiter as f64)) as i32;
                            { let __px = (x) as i32; let __py = (y) as i32; if __px >= 0 && __py >= 0 && (__px as u32) < pw && (__py as u32) < ph { let __i = ((__py as u32 * pw + __px as u32) * 4) as usize; let __c = iced::Color::from_rgb8((t) as u8, ((t as f64) / (2 as f64)) as u8, (255 - t) as u8); pix[__i] = (__c.r * 255.0) as u8; pix[__i + 1] = (__c.g * 255.0) as u8; pix[__i + 2] = (__c.b * 255.0) as u8; pix[__i + 3] = 255; pix_dirty = true; } }
                        }
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
    iced::application("Mandelbrot", update, view)
        .window_size(iced::Size::new(640.0, 480.0))
        .run()
}
