use iced::{widget, Background, Color, Length};
use iced_video::{viewer::ControlEvent, BasicPlayer};

use crate::{state::State, theme, update::Message, Element};

pub fn image(state: &State) -> Element {
    let image = if let Some(handle) = state.player_handler.get_frame("main player") {
        iced::widget::image(handle.clone())
            .height(Length::Fill)
            .width(Length::Fill)
    } else {
        iced::widget::image(widget::image::Handle::from_rgba(0, 0, vec![]))
            .height(Length::Fill)
            .width(Length::Fill)
    };

    widget::container(
        widget::button(image)
            .on_press(
                if let Some(player) = state.player_handler.get_player("main player") {
                    if player.is_playing() {
                        Message::ControlEvent(ControlEvent::Pause)
                    } else {
                        Message::ControlEvent(ControlEvent::Play)
                    }
                } else {
                    Message::None(())
                },
            )
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}
