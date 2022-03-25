mod vec2;
mod pos2;

#[cfg(target_arch = "wasm32")]
pub use vec2::Vec2;

#[cfg(target_arch = "wasm32")]
pub use web_sys;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

#[cfg(target_arch = "wasm32")]
fn native_pixels_per_point() -> f32 {
    let pixels_per_point = web_sys::window().unwrap().device_pixel_ratio() as f32;
    if pixels_per_point > 0.0 && pixels_per_point.is_finite() {
        pixels_per_point
    } else {
        1.0
    }
}

#[cfg(target_arch = "wasm32")]
fn screen_size_in_native_points() -> Vec2 {
    let window = web_sys::window().unwrap();
    vec2::vec2(
        window.inner_width().ok().unwrap().as_f64().unwrap() as f32,
        window.inner_height().ok().unwrap().as_f64().unwrap() as f32,
    )
}

#[cfg(target_arch = "wasm32")]
pub fn resize_canvas_to_screen_size(window: &winit::window::Window) -> bool {
    let canvas = window.canvas();

    let screen_size_points = screen_size_in_native_points();
    let pixels_per_point = native_pixels_per_point();

    let canvas_size_pixels = pixels_per_point * screen_size_points;
    let canvas_size_points = canvas_size_pixels / pixels_per_point;


    fn round_to_even(v: f32) -> f32 {
        (v / 2.0).round() * 2.0
    }

    canvas
        .style()
        .set_property(
            "width",
            &format!("{}px", round_to_even(canvas_size_points.x)),
        )
        .ok();
    canvas
        .style()
        .set_property(
            "height",
            &format!("{}px", round_to_even(canvas_size_points.y)),
        )
        .ok();

    let width = round_to_even(canvas_size_pixels.x) as u32;
    let height = round_to_even(canvas_size_pixels.y) as u32;

    if canvas.width() != width || canvas.height() != height {
        canvas.set_width(width);
        canvas.set_height(height);
        return true
    }

    false
}

