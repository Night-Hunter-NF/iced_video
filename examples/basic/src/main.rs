use iced::{
    executor,
    widget::{self, container, button, text},
    Application, Command, Element,
};
use iced_video::{
    viewer::{video_view, ControlEvent},
    AdvancedPlayer, BasicPlayer, PlayerBuilder, PlayerHandler, PlayerMessage,
};

fn main() {
    // uncomment to see debug messages from gstreamer
    // std::env::set_var("GST_DEBUG", "3");
    App::run(Default::default()).unwrap();
}

#[derive(Clone, Debug)]
enum Message {
    Video(PlayerMessage),
    ControlEvent(ControlEvent),
    ToggleLoop(String),
}

struct App {
    player_handler: PlayerHandler,
    seek: Option<u64>,
    id: String,
}

impl Application for App {
    type Executor = executor::Default;

    type Message = Message;

    type Theme = iced::Theme;

    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let mut player_handler = PlayerHandler::default();
        let url =
            "http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4";
        player_handler.start_player(PlayerBuilder::new(url).set_auto_start(true).set_uri(url));

        (
            App {
                player_handler,
                seek: None,
                id: url.to_string(),
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        self.player_handler.subscriptions().map(Message::Video)
    }

    fn title(&self) -> String {
        String::from("Video Player")
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Video(event) => {
                self.player_handler.handle_event(event);
            }
            Message::ControlEvent(event) => {
                if let Some(player) = self.player_handler.get_player_mut(&self.id) {
                    match event {
                        ControlEvent::Play => player.play(),
                        ControlEvent::Pause => player.pause(),
                        ControlEvent::ToggleMute => {
                            if player.get_muted() {
                                player.set_muted(false)
                            } else {
                                player.set_muted(true)
                            }
                        }
                        ControlEvent::Volume(volume) => {
                            // player.set_volume(volume)
                        }
                        ControlEvent::Seek(p) => {
                            self.seek = Some(p as u64);
                        }
                        ControlEvent::Released => {
                            player
                                .seek(std::time::Duration::from_secs(self.seek.unwrap()))
                                .unwrap_or_else(|err| println!("Error seeking: {:?}", err));
                            self.seek = None;
                        }
                    }
                }
            }
            Message::ToggleLoop(id) => {
                if let Some(player) = self.player_handler.get_player_mut(&id) {
                    if player.get_looping() {
                        player.set_looping(false)
                    } else {
                        player.set_looping(true)
                    }
                }
            },
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<Message> {
        let player: Element<Message> = if let Some(player) =
            self.player_handler.get_player(&self.id)
        {
            let frame = self.player_handler.get_frame(&self.id);
            // if let Some(handle) = frame {
            //     let i_width = 1280 as u16;
            //     let i_height = (i_width as f32 * 9.0 / 16.0) as u16;
            //     iced::widget::image(handle.clone())
            //         .height(i_height)
            //         .width(i_width)
            //         .into()
            // } else {
            //     iced::widget::image(iced::widget::image::Handle::from_pixels(0, 0, vec![])).into()
            // }
            widget::column![widget::row![text(player.get_looping()) ,button("Loop").on_press(Message::ToggleLoop(self.id.clone()))],
            video_view(player, frame, &Message::ControlEvent, &self.seek)].into()
        } else {
            widget::Text::new("No player").size(30).into()
        };

        container(player).center_x().center_y().into()
    }
}
