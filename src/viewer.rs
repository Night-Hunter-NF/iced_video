//! Video viewer
//! displays the video and the overlay
use iced::{
    advanced::{layout, overlay, renderer, widget::Tree, Clipboard, Layout, Shell, Widget},
    alignment::{Horizontal, Vertical},
    event, mouse,
    widget::{self, container, image, svg, text},
    Alignment,  Color, Element, Event, Length, Point, Rectangle, Size,
};
use playbin_core::AdvancedPlayer;

use crate::{
    helpers::{helper_functions::secs_to_hhmmss, svgs},
    overlay::Overlay,
};

/// viewer event enum
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum ControlEvent {
    Play,
    Pause,
    ToggleMute,
    Volume(f64),
    Seek(f64),
    Released,
}

/// a viewer fuction to make an over easyliy
pub fn video_view<'a, Message,Theme, Renderer, F, P>(
    player: &'a P,
    frame: Option<&'a image::Handle>,
    on_event: &'a F,
    seek_amount: &'a Option<u64>,
) -> iced::Element<'a, Message,Theme, Renderer>
where
    P: AdvancedPlayer,
    Message: std::clone::Clone + 'a,
    Renderer: iced::advanced::text::Renderer
        + iced::advanced::image::Renderer
        + iced::advanced::svg::Renderer
        + 'static,
    Theme: widget::button::StyleSheet
        + widget::text_input::StyleSheet
        + widget::text::StyleSheet
        + widget::slider::StyleSheet
        + widget::container::StyleSheet
        + widget::svg::StyleSheet + 'a,
    F: Fn(ControlEvent) -> Message + 'static + Clone,
    <Renderer as iced::advanced::image::Renderer>::Handle: From<image::Handle>,
{
    let i_width = 1280 as u16;
    let i_height = (i_width as f32 * 9.0 / 16.0) as u16;
    let width = Box::new(i_width);
    let height = Box::new(i_height);
    let player = Box::new(player);

    let image = if let Some(handle) = frame {
        iced::widget::image(handle.clone())
            .height(i_height)
            .width(i_width)
    } else {
        iced::widget::image(image::Handle::from_pixels(0, 0, vec![]))
    };
    let duration = player.get_duration().as_secs();
    let position = if let Some(seek) = seek_amount {
        seek.to_owned()
    } else {
        player.get_position().as_secs()
    };

    let play_pause = if player.is_playing() {
        widget::Button::new(svg(svgs::pause_svg()).height(28).width(28))
            // .style(theme::Button::Transparent)
            .on_press(on_event(ControlEvent::Pause).clone())
    } else {
        widget::Button::new(svg(svgs::play_svg()).height(28).width(28))
            .on_press(on_event(ControlEvent::Play).clone())
    };

    let duration_text = text(format!(
        "{} / {}",
        secs_to_hhmmss(position),
        secs_to_hhmmss(duration)
    ));

    let volume = player.get_volume();
    let volume_svg = if volume > 0.66 {
        svgs::high_volume_svg()
    } else if volume > 0.33 {
        svgs::medium_volume_svg()
    } else if volume > 0.0 {
        svgs::low_volume_svg()
    } else {
        svgs::muted_svg()
    };

    let volume_button = if !player.get_muted() {
        widget::Button::new(svg(volume_svg).height(28).width(28))
            .on_press(on_event(ControlEvent::ToggleMute).clone())
    } else {
        widget::Button::new(svg(svgs::muted_svg()).height(28).width(28))
            .on_press(on_event(ControlEvent::ToggleMute).clone())
    };

    let volume_slider = widget::Slider::new(0.0..=1.0, volume, |v| {
        on_event(ControlEvent::Volume(v).clone())
    })
    .step(0.05)
    .width(80);

    let seek_slider = widget::Slider::new(
        0.0..=duration.to_owned() as f64,
        position.to_owned() as f64,
        |v| on_event(ControlEvent::Seek(v).clone()),
    )
    .on_release(on_event(ControlEvent::Released).clone())
    .step(1.0);

    let overlay = container(widget::column![
        seek_slider,
        widget::row![play_pause, duration_text, volume_button, volume_slider]
            .width(Length::Fill)
            .height(60)
            .align_items(Alignment::Center)
    ])
    .align_y(Vertical::Bottom)
    .align_x(Horizontal::Left)
    .width(*width)
    .height(*height);

    let content = Overlay::new(container(image).width(*width).height(*height), overlay);
    container(content).into()
}
