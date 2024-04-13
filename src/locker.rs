use chrono::{DateTime, Local};
use cosmic::{
    font::FONT_BOLD,
    iced::{
        self, alignment,
        event::{
            self,
            wayland::{Event as WaylandEvent, OutputEvent, SessionLockEvent},
        },
        subscription,
        wayland::session_lock::{destroy_lock_surface, get_lock_surface, lock, unlock},
        Length, Subscription,
    },
    iced_widget::text,
    widget::Widget,
};
use std::{collections::HashMap, error::Error, process, time::Duration};

use cosmic::{
    app::{message, Command, Core, Settings},
    executor::{self, multi::Executor},
    iced_runtime::core::window::Id as SurfaceId,
    style, widget, Element,
};

use wayland_client::{protocol::wl_output::WlOutput, Proxy};

pub fn main() -> Result<(), Box<dyn Error>> {
    let flags = Flags {};
    let settings = Settings::default().no_main_window(true);
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Debug)]
enum State {
    Locking,
    Locked,
    Unlocking,
    Unlocked,
}

pub struct App {
    core: Core,
    flags: Flags,
    now: DateTime<Local>,
    surface_ids: HashMap<WlOutput, SurfaceId>,
    state: State,
}

#[derive(Clone)]
pub struct Flags {}

#[derive(Clone, Debug)]
pub enum Message {
    None,
    OutputEvent(OutputEvent, WlOutput),
    SessionLockEvent(SessionLockEvent),
    Unlock,
    Tick,
}

impl cosmic::Application for App {
    type Executor = executor::Default;

    type Flags = Flags;

    type Message = Message;

    const APP_ID: &'static str = "zero-lock";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(
        mut core: Core,
        flags: Self::Flags,
    ) -> (Self, cosmic::iced::Command<message::Message<Self::Message>>) {
        core.window.show_window_menu = false;
        core.window.show_headerbar = false;
        core.window.sharp_corners = true;
        core.window.show_maximize = false;
        core.window.show_minimize = false;
        core.window.use_template = false;

        let now = Local::now();
        let app = App {
            core,
            flags,
            now,
            state: State::Unlocked,
            surface_ids: HashMap::new(),
        };

        (app, lock())
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<message::Message<Self::Message>> {
        match message {
            Message::OutputEvent(output_event, output) => match output_event {
                OutputEvent::Created(output_info_opt) => {
                    log::info!("output {}: created", output.id());

                    let surface_id = SurfaceId::unique();
                    match self.surface_ids.insert(output.clone(), surface_id) {
                        Some(old_surface_id) => {
                            //TODO: remove old surface?
                            log::warn!(
                                "output {}: already had surface ID {:?}",
                                output.id(),
                                old_surface_id
                            );
                        }
                        None => {}
                    }
                    Command::none()
                }
                OutputEvent::Removed => {
                    log::info!("output {}: removed", output.id());
                    match self.surface_ids.remove(&output) {
                        Some(surface_id) => {
                            if matches!(self.state, State::Locked) {
                                return destroy_lock_surface(surface_id);
                            }
                        }
                        None => {
                            log::warn!("output {}: no surface found", output.id());
                        }
                    }
                    Command::none()
                }
                OutputEvent::InfoUpdate(_output_info) => {
                    log::info!("output {}: info update", output.id());
                    Command::none()
                }
            },
            Message::SessionLockEvent(session_lock_event) => match session_lock_event {
                SessionLockEvent::Focused(_, surface_id) => {
                    log::info!("focus surface {:?}", surface_id);
                    Command::none()
                }
                SessionLockEvent::Locked => {
                    log::info!("session locked");
                    self.state = State::Locked;
                    let mut commands = Vec::with_capacity(self.surface_ids.len());
                    for (output, surface_id) in self.surface_ids.iter() {
                        commands.push(get_lock_surface(*surface_id, output.clone()));
                    }
                    return Command::batch(commands);
                }
                SessionLockEvent::Unlocked => {
                    log::info!("session unlocked");
                    self.state = State::Unlocked;
                    process::exit(0)
                }
                SessionLockEvent::Finished => todo!(),
                SessionLockEvent::NotSupported => todo!(),
                SessionLockEvent::Unfocused(_, _) => todo!(),
                //TODO: handle finished signal
            },
            Message::None => todo!(),
            Message::Unlock => unlock(),
            Message::Tick => {
                self.now = Local::now();
                Command::none()
            }
        }
    }

    fn view(&self) -> cosmic::prelude::Element<Self::Message> {
        unimplemented!()
    }

    fn view_window(&self, surface_id: SurfaceId) -> Element<Self::Message> {
        let date_time_column = {
            let mut column = widget::column::with_capacity::<Message>(1).padding(10);

            //TODO: localized format
            let date = self.now.format("%b %e %-I:%M %p");
            column = column.push(
                widget::text::text(format!("{}", date))
                    .style(style::Text::Default)
                    .size(18)
                    .font(FONT_BOLD),
            );

            column
        };

        let centered = cosmic::widget::container(date_time_column)
            .width(iced::Length::Fill)
            .height(iced::Length::Shrink)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Top);

        Element::from(centered)
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let mut subscriptions = Vec::with_capacity(7);

        subscriptions.push(event::listen_with(|event, _| match event {
            iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                wayland_event,
            )) => match wayland_event {
                WaylandEvent::Output(output_event, output) => {
                    Some(Message::OutputEvent(output_event, output))
                }
                WaylandEvent::SessionLock(evt) => Some(Message::SessionLockEvent(evt)),
                _ => None,
            },
            _ => None,
        }));

        // Unlocks automatically for testing purpose
        subscriptions.push(time_subscription(10).map(|_| Message::Unlock));
        subscriptions.push(time_subscription(60).map(|_| Message::Tick));

        Subscription::batch(subscriptions)
    }
}

fn time_subscription(secs: u64) -> Subscription<()> {
    subscription::unfold("time-sub", (), move |()| async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        ((), ())
    })
}
