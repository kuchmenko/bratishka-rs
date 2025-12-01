use iced::widget::{button, column, text, text_input};
use iced::{Element, Task};

fn main() -> iced::Result {
    iced::application("Bratishka", App::update, App::view).run_with(App::new)
}

#[derive(Default)]
struct App {
    url: String,
}

#[derive(Debug, Clone)]
enum Message {
    UrlChanged(String),
    Process,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UrlChanged(url) => self.url = url,
            Message::Process => {
                // TODO: integrate with bratishka-core
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text("Bratishka").size(24),
            text_input("Enter YouTube URL...", &self.url).on_input(Message::UrlChanged),
            button("Process").on_press(Message::Process),
        ]
        .padding(20)
        .spacing(10)
        .into()
    }
}
