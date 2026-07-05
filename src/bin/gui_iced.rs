use std::time::Duration;

use iced::widget::{button, column, container, pick_list, row, rule, slider, text};
use iced::{time, Element, Fill, Size, Subscription, Task, Theme};

use elgato_keylight::{
    avahi::{find_elgato_devices, Device},
    get_status, set_status, Brightness, DeviceStatus, KeyLightStatus, PowerStatus, Temperature,
};

const APP_TITLE: &str = "Elgato Key Light Controller (iced)";

fn main() -> iced::Result {
    #[cfg(not(target_os = "linux"))]
    panic!("Only Linux is supported");

    env_logger::init();

    iced::application(KeyLightApp::new, update, view)
        .title(APP_TITLE)
        .theme(Theme::TokyoNight)
        .window_size(Size::new(440.0, 360.0))
        .centered()
        .resizable(true)
        .subscription(subscription)
        .run()
}

#[derive(Debug, Clone)]
struct KeyLightApp {
    devices: Vec<Device>,
    selected_device: Option<Device>,
    controls: Controls,
    is_discovering: bool,
    reload_after_discovery: bool,
    is_applying: bool,
    error: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Default)]
enum Controls {
    #[default]
    NoDevice,
    Loading,
    Ready {
        power_status: PowerStatus,
        brightness: u8,
        temperature: u16,
    },
}

#[derive(Debug, Clone)]
enum Message {
    RefreshDevices(bool),
    DevicesDiscovered(Result<Vec<Device>, String>),
    DeviceSelected(Device),
    StatusLoaded(Result<(Device, KeyLightStatus), String>),
    TogglePower,
    BrightnessChanged(u8),
    BrightnessReleased,
    TemperatureChanged(u16),
    TemperatureReleased,
    StatusApplied(Result<(Device, KeyLightStatus), String>),
    DismissError,
}

impl KeyLightApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                devices: Vec::new(),
                selected_device: None,
                controls: Controls::NoDevice,
                is_discovering: true,
                reload_after_discovery: true,
                is_applying: false,
                error: None,
                status: "Discovering Key Lights...".to_string(),
            },
            Task::perform(discover_devices(), Message::DevicesDiscovered),
        )
    }
}

fn update(app: &mut KeyLightApp, message: Message) -> Task<Message> {
    match message {
        Message::RefreshDevices(reload_selected) => {
            if app.is_discovering {
                return Task::none();
            }

            app.is_discovering = true;
            app.reload_after_discovery = reload_selected;
            app.status = "Discovering Key Lights...".to_string();
            Task::perform(discover_devices(), Message::DevicesDiscovered)
        }
        Message::DevicesDiscovered(result) => {
            app.is_discovering = false;

            let devices = match result {
                Ok(devices) => devices,
                Err(err) => {
                    app.error = Some(err);
                    app.status = "Discovery failed".to_string();
                    return Task::none();
                }
            };

            app.devices = devices;

            if app.devices.is_empty() {
                app.selected_device = None;
                app.controls = Controls::NoDevice;
                app.status = "No Key Lights found".to_string();
                return Task::none();
            }

            let selected = app
                .selected_device
                .as_ref()
                .and_then(|selected| app.devices.iter().find(|device| *device == selected))
                .cloned()
                .or_else(|| app.devices.first().cloned());

            let should_reload = app.reload_after_discovery
                || app.selected_device.as_ref() != selected.as_ref()
                || matches!(app.controls, Controls::NoDevice | Controls::Loading);

            app.reload_after_discovery = false;
            app.selected_device = selected.clone();

            if let Some(device) = selected {
                app.status = format!("Selected {}", device.name);

                if should_reload {
                    app.controls = Controls::Loading;
                    return Task::perform(load_status(device), Message::StatusLoaded);
                }
            }

            Task::none()
        }
        Message::DeviceSelected(device) => {
            app.selected_device = Some(device.clone());
            app.controls = Controls::Loading;
            app.status = format!("Loading {}...", device.name);
            Task::perform(load_status(device), Message::StatusLoaded)
        }
        Message::StatusLoaded(result) => {
            match result {
                Ok((device, status)) => {
                    if app.selected_device.as_ref() == Some(&device) {
                        app.controls = Controls::Ready {
                            power_status: status.power,
                            brightness: status.brightness.0,
                            temperature: status.temperature.0,
                        };
                        app.status = format!("Loaded {}", device.name);
                    }
                }
                Err(err) => {
                    app.error = Some(err);
                    app.controls = Controls::NoDevice;
                    app.status = "Failed to load device status".to_string();
                }
            }

            Task::none()
        }
        Message::TogglePower => {
            if let Controls::Ready { power_status, .. } = &mut app.controls {
                *power_status = match power_status {
                    PowerStatus::Off => PowerStatus::On,
                    PowerStatus::On => PowerStatus::Off,
                };
            }

            apply_current_status(app)
        }
        Message::BrightnessChanged(value) => {
            if let Controls::Ready { brightness, .. } = &mut app.controls {
                *brightness = value;
            }

            Task::none()
        }
        Message::BrightnessReleased => apply_current_status(app),
        Message::TemperatureChanged(value) => {
            if let Controls::Ready { temperature, .. } = &mut app.controls {
                *temperature = value;
            }

            Task::none()
        }
        Message::TemperatureReleased => apply_current_status(app),
        Message::StatusApplied(result) => {
            app.is_applying = false;

            match result {
                Ok((device, status)) => {
                    if app.selected_device.as_ref() == Some(&device) {
                        app.controls = Controls::Ready {
                            power_status: status.power,
                            brightness: status.brightness.0,
                            temperature: status.temperature.0,
                        };
                        app.status = format!("Updated {}", device.name);
                    }
                }
                Err(err) => {
                    app.error = Some(err);
                    app.status = "Failed to update device".to_string();
                }
            }

            Task::none()
        }
        Message::DismissError => {
            app.error = None;
            Task::none()
        }
    }
}

fn subscription(app: &KeyLightApp) -> Subscription<Message> {
    if app.is_discovering {
        Subscription::none()
    } else {
        time::every(Duration::from_secs(10)).map(|_| Message::RefreshDevices(false))
    }
}

fn view(app: &KeyLightApp) -> Element<'_, Message> {
    let refresh = if app.is_discovering {
        button("Discovering...")
    } else {
        button("Refresh").on_press(Message::RefreshDevices(true))
    };

    let selector = pick_list(
        app.devices.clone(),
        app.selected_device.clone(),
        Message::DeviceSelected,
    )
    .placeholder("No device selected")
    .width(Fill);

    let mut content = column![
        column![
            text("Elgato Key Light").size(28),
            text("iced-rs GUI variant").size(14),
        ]
        .spacing(4),
        rule::horizontal(1),
        row![selector, refresh].spacing(10),
        text(&app.status).size(14),
    ]
    .padding(24)
    .spacing(16)
    .max_width(560);

    if let Some(error) = &app.error {
        content = content.push(
            container(
                column![
                    text("Error").size(16),
                    text(error),
                    button("Dismiss").on_press(Message::DismissError),
                ]
                .spacing(8),
            )
            .padding(12)
            .width(Fill)
            .style(iced::widget::container::rounded_box),
        );
    }

    content = content.push(match app.controls {
        Controls::NoDevice => container(text("Select or discover a Key Light to begin."))
            .padding(16)
            .width(Fill)
            .style(iced::widget::container::rounded_box)
            .into(),
        Controls::Loading => container(text("Loading device status..."))
            .padding(16)
            .width(Fill)
            .style(iced::widget::container::rounded_box)
            .into(),
        Controls::Ready {
            power_status,
            brightness,
            temperature,
        } => controls_view(power_status, brightness, temperature, app.is_applying),
    });

    container(content)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .into()
}

fn controls_view(
    power_status: PowerStatus,
    brightness: u8,
    temperature: u16,
    is_applying: bool,
) -> Element<'static, Message> {
    let power_label = match power_status {
        PowerStatus::On => "On",
        PowerStatus::Off => "Off",
    };

    let power_action = match (is_applying, power_status) {
        (true, _) => button("Applying..."),
        (false, PowerStatus::On) => button("Turn off").on_press(Message::TogglePower),
        (false, PowerStatus::Off) => button("Turn on").on_press(Message::TogglePower),
    };

    container(
        column![
            row![text("Power:"), text(power_label), power_action].spacing(10),
            column![
                text(format!("Brightness: {brightness}%")),
                slider(3..=100, brightness, Message::BrightnessChanged)
                    .on_release(Message::BrightnessReleased),
            ]
            .spacing(8),
            column![
                text(format!("Temperature: {temperature}K")),
                slider(143..=344, temperature, Message::TemperatureChanged)
                    .on_release(Message::TemperatureReleased),
            ]
            .spacing(8),
        ]
        .spacing(16),
    )
    .padding(16)
    .width(Fill)
    .style(iced::widget::container::rounded_box)
    .into()
}

fn apply_current_status(app: &mut KeyLightApp) -> Task<Message> {
    if app.is_applying {
        return Task::none();
    }

    let Some(device) = app.selected_device.clone() else {
        return Task::none();
    };

    let Controls::Ready {
        power_status,
        brightness,
        temperature,
    } = app.controls.clone()
    else {
        return Task::none();
    };

    app.is_applying = true;
    app.status = format!("Updating {}...", device.name);

    Task::perform(
        apply_status(device, power_status, brightness, temperature),
        Message::StatusApplied,
    )
}

async fn discover_devices() -> Result<Vec<Device>, String> {
    find_elgato_devices().await.map_err(|err| err.to_string())
}

async fn load_status(device: Device) -> Result<(Device, KeyLightStatus), String> {
    let status = get_status(device.url.clone())
        .await
        .map_err(|err| err.to_string())?;

    let light = status
        .lights
        .first()
        .cloned()
        .ok_or_else(|| "No light found".to_string())?;

    Ok((device, light))
}

async fn apply_status(
    device: Device,
    power_status: PowerStatus,
    brightness: u8,
    temperature: u16,
) -> Result<(Device, KeyLightStatus), String> {
    let new_status = KeyLightStatus {
        power: power_status,
        brightness: Brightness::new(brightness)?,
        temperature: Temperature::new(temperature)?,
    };

    let payload = DeviceStatus {
        number_of_lights: 1,
        lights: vec![new_status.clone()],
    };

    set_status(device.url.clone(), payload)
        .await
        .map_err(|err| err.to_string())?;

    Ok((device, new_status))
}
