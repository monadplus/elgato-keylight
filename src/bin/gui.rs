use std::sync::{Arc, RwLock};

use eframe::egui::{self, Color32, Id, PopupCloseBehavior, Ui};
use elgato_keylight::{
    avahi::{find_elgato_devices, spawn_avahi_daemon, AvahiState, Device},
    get_status, set_status, Brightness, DeviceStatus, KeyLightStatus, PowerStatus, Temperature,
};
use log::{error, info};
use tokio::runtime::Runtime;

/// Identifier for the popup error
const ERROR_POPUP_ID: &str = "error-popup-id";

fn main() -> eframe::Result {
    // RUST_LOG=debug cargo run
    env_logger::init();

    let runtime = Arc::new(Runtime::new().expect("Unable to create runtime"));

    let devices = get_available_devices(&runtime).unwrap_or_else(|err| {
        error!("Failed to get available devices: {err}");
        vec![]
    });
    let opt_device = devices.first().cloned();

    let avahi = Arc::new(RwLock::new(AvahiState {
        devices: devices.clone(),
    }));

    let _ = spawn_avahi_daemon(Arc::clone(&avahi));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 240.0])
            .with_close_button(true)
            .with_resizable(true),
        ..Default::default()
    };

    let mut app = MyApp {
        runtime,
        avahi,
        devices,
        error: None,
        state: AppState::default(),
    };

    if let Some(device) = opt_device {
        app.select_device(None, device.clone());
    }

    eframe::run_native(
        "Elgato Key Light Controller",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

#[derive(Debug)]
struct MyApp {
    /// `tokio` runtime to execute asynchronous task
    runtime: Arc<Runtime>,
    /// Asynchronous avahi state of devices
    avahi: Arc<RwLock<AvahiState>>,
    /// Current list of available devices
    devices: Vec<Device>,
    /// Error messageCLI & device discover
    error: Option<String>,
    /// Application state
    state: AppState,
}

#[derive(Debug, Default)]
enum AppState {
    #[default]
    NotSelected,
    Selected {
        /// Current selected device
        device: Device,
        power_status: PowerStatus,
        brightness: Brightness,
        temperature: Temperature,
    },
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);
        let elgato_icon = egui::include_image!("../../assets/elgato_logo.png");
        let bulb_icon = egui::Image::new(egui::include_image!("../../assets/bulb_icon.png"))
            .max_width(20.0)
            .rounding(5.0);

        {
            let rlock = self.avahi.read().expect("read lock");
            self.devices = rlock.devices.clone();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.horizontal(|ui| {
                ui.heading("Elgato Key Light Controller");
                ui.add(egui::Image::new(elgato_icon))
            });
            let response = response.inner;

            egui::popup_below_widget(
                ui,
                Id::new(ERROR_POPUP_ID),
                &response,
                PopupCloseBehavior::CloseOnClick,
                |ui| {
                    ui.set_min_width(300.0);
                    ui.heading("Error");
                    ui.separator();
                    ui.label(self.error.clone().unwrap_or_else(|| "No error".to_string()));
                },
            );

            ui.separator();
            ui.add_space(10.0);

            let mut device_selected = if let AppState::Selected { device, .. } = &self.state {
                device.name.clone()
            } else {
                "No device found".to_string()
            };
            let response = egui::ComboBox::from_label("")
                .selected_text(device_selected.clone())
                .show_ui(ui, |ui| {
                    self.devices
                        .iter()
                        .map(|device| {
                            ui.selectable_value(
                                &mut device_selected,
                                device.name.clone(),
                                device.name.clone(),
                            )
                        })
                        .reduce(|acc, e| acc.union(e))
                });
            let response = response.inner.flatten().unwrap_or(response.response);
            if response.changed() {
                if let Some(device) = self.devices.iter().find(|d| d.name == device_selected) {
                    info!("Device `{}` selected", device.name);
                    self.select_device(Some(ui), device.clone());
                }
            }

            ui.add_space(20.0);

            match &self.state {
                AppState::NotSelected => {}
                AppState::Selected {
                    power_status,
                    brightness,
                    temperature,
                    ..
                } => {
                    let power_status = (*power_status).into();
                    let mut brightness = brightness.0;
                    let mut temperature = temperature.0;

                    if power_status {
                        let r = ui.add(egui::Button::image(bulb_icon).fill(Color32::YELLOW));
                        if r.clicked() {
                            self.set_power(ui, PowerStatus::Off)
                        }
                    } else {
                        let r = ui.add(egui::Button::image(bulb_icon).fill(Color32::GRAY));
                        if r.clicked() {
                            self.set_power(ui, PowerStatus::On)
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label("Temperature:");
                        let response = ui.add(
                            egui::Slider::new(&mut temperature, 143..=344)
                                .suffix("K")
                                .clamp_to_range(true)
                                .trailing_fill(true),
                        );
                        if response.drag_stopped() {
                            self.set_temperature(ui, temperature)
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Brightness:");
                        ui.add_space(15.0);
                        let response = ui.add(
                            egui::Slider::new(&mut brightness, 3..=100)
                                .suffix("%")
                                .clamp_to_range(true)
                                .trailing_fill(true),
                        );
                        if response.drag_stopped() {
                            self.set_brightness(ui, brightness)
                        }
                    });
                }
            }
        });
    }
}

impl MyApp {
    fn error_popup<E: std::fmt::Display>(&mut self, ui: &Ui, err: E) {
        self.error = Some(format!("{err}"));
        ui.memory_mut(|mem| mem.toggle_popup(Id::new(ERROR_POPUP_ID)));
    }

    pub fn select_device(&mut self, ui: Option<&Ui>, new_device: Device) {
        if let AppState::Selected { ref device, .. } = self.state {
            if *device == new_device {
                info!("Same device selected");
                return;
            }
        }

        match self.runtime.block_on(get_status(new_device.url.clone())) {
            Err(err) => {
                error!("Get status failed: {err}");
                if let Some(ui) = ui {
                    self.error_popup(ui, err);
                }
            }
            Ok(status) => {
                let Some(light) = status.lights.first() else {
                    error!("No light found");
                    return;
                };

                self.state = AppState::Selected {
                    device: new_device,
                    power_status: light.power,
                    brightness: light.brightness,
                    temperature: light.temperature,
                };
            }
        }
    }

    fn set_status(&mut self, ui: &Ui, new_status: KeyLightStatus) {
        if let AppState::Selected {
            device,
            power_status,
            brightness,
            temperature,
            ..
        } = &mut self.state
        {
            let payload = DeviceStatus {
                number_of_lights: 1,
                lights: vec![new_status.clone()],
            };

            match self
                .runtime
                .block_on(set_status(device.url.clone(), payload))
            {
                Ok(_) => {
                    info!(
                        "Setting new status: power={}, brightness={}, temperature={}",
                        power_status, brightness.0, temperature.0
                    );
                    // Set new state
                    *power_status = new_status.power;
                    *brightness = new_status.brightness;
                    *temperature = new_status.temperature;
                }
                Err(err) => self.error_popup(ui, err),
            }
        }
    }

    pub fn set_power(&mut self, ui: &Ui, power: PowerStatus) {
        if let AppState::Selected {
            brightness,
            temperature,
            ..
        } = &self.state
        {
            let new_status = KeyLightStatus {
                power,
                brightness: *brightness,
                temperature: *temperature,
            };
            self.set_status(ui, new_status);
        }
    }

    pub fn set_temperature(&mut self, ui: &Ui, temperature: u16) {
        if let AppState::Selected {
            power_status,
            brightness,
            ..
        } = &self.state
        {
            let new_status = KeyLightStatus {
                power: *power_status,
                brightness: *brightness,
                temperature: Temperature::new(temperature).expect("Temperature range [143,344]"),
            };
            self.set_status(ui, new_status);
        }
    }

    pub fn set_brightness(&mut self, ui: &Ui, brightness: u8) {
        if let AppState::Selected {
            power_status,
            temperature,
            ..
        } = &self.state
        {
            let new_status = KeyLightStatus {
                power: *power_status,
                temperature: *temperature,
                brightness: Brightness::new(brightness).expect("Brightness range [0, 100]"),
            };
            self.set_status(ui, new_status);
        }
    }
}

fn get_available_devices(rt: &Runtime) -> anyhow::Result<Vec<Device>> {
    Ok(rt.block_on(find_elgato_devices())?)
}
