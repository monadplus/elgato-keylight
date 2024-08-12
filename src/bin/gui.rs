use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use eframe::egui::{self, Color32, Id, PopupCloseBehavior, Ui};
use elgato_keylight::{
    avahi::{find_elgato_devices, spawn_avahi_daemon, AvahiState, Device},
    get_status, set_status, Brightness, DeviceStatus, KeyLightStatus, PowerStatus, Temperature,
};
use log::{debug, error, info};
use tokio::runtime::Runtime;
use tray_icon::menu::{MenuEvent, MenuId, MenuItem};

/// Identifier for the popup error
const ERROR_POPUP_ID: &str = "error-popup";

const OPEN_MENU_ITEM_ID: &str = "open-menu-item";
const EXIT_MENU_ITEM_ID: &str = "exit-menu-item";

fn main() -> eframe::Result {
    // RUST_LOG=debug cargo run
    env_logger::init();

    let is_window_opened = Arc::new(AtomicBool::new(true));
    let stop_signal = Arc::new(AtomicBool::new(false));

    // Since egui uses winit under the hood and doesn't use gtk on Linux, and we need gtk for
    // the tray icon to show up, we need to spawn a thread
    // where we initialize gtk and create the tray_icon
    #[cfg(target_os = "linux")]
    {
        let is_window_opened = Arc::clone(&is_window_opened);
        let stop_signal = Arc::clone(&stop_signal);

        std::thread::spawn(move || {
            gtk::init().expect("Couldn't start gtk context");

            let open_menu_item = MenuItem::with_id(
                OPEN_MENU_ITEM_ID,
                "open",
                !is_window_opened.load(Ordering::Relaxed),
                None,
            );

            let tray_menu = tray_icon::menu::Menu::with_id_and_items(
                MenuId::new("main"),
                &[
                    &open_menu_item,
                    &MenuItem::with_id(EXIT_MENU_ITEM_ID, "exit", true, None),
                ],
            )
            .unwrap();

            let tray_icon_icon = load_icon(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/elgato_icon.png"
            )));

            let _tray_icon = tray_icon::TrayIconBuilder::new()
                .with_menu(Box::new(tray_menu))
                .with_icon(tray_icon_icon)
                .with_tooltip("Elgato Keylight Controller")
                .with_title("Elgato Keylight Controller")
                .build()
                .expect("Couldn't start tray icon");

            while gtk::main_iteration() {
                let main_window_opened = is_window_opened.load(Ordering::Acquire);
                open_menu_item.set_enabled(!main_window_opened);
                if !main_window_opened {
                    if let Ok(event) = MenuEvent::receiver().try_recv() {
                        debug!("Menu event: {:?}", event);
                        if event.id() == OPEN_MENU_ITEM_ID {
                            is_window_opened.store(true, Ordering::Relaxed);
                        }
                        if event.id() == EXIT_MENU_ITEM_ID {
                            stop_signal.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
        });
    }

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
        run_and_return: true,
        ..Default::default()
    };

    let mut app = MyApp {
        is_window_open: Arc::clone(&is_window_opened),
        stop_signal: Arc::clone(&stop_signal),
        runtime,
        avahi,
        devices,
        error: None,
        state: AppState::default(),
    };
    if let Some(device) = opt_device {
        app.select_device(None, device.clone());
    }

    // NOTE: a condvar will not work because you need to
    // wait after the `run_native`, but you won't be able to set the stop
    // because you are holding a lock here.
    while !stop_signal.load(Ordering::Acquire) {
        if is_window_opened.load(Ordering::Acquire) {
            let app = app.clone();
            eframe::run_native(
                "Elgato Key Light Controller",
                options.clone(),
                Box::new(|_cc| Ok(Box::new(app))),
            )
            .unwrap()
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct MyApp {
    /// Is the main window open
    is_window_open: Arc<AtomicBool>,
    /// Stop app
    stop_signal: Arc<AtomicBool>,
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

#[derive(Debug, Default, Clone)]
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
        ctx.input(|i| {
            if i.viewport().close_requested() {
                debug!("Close requested");
                self.is_window_open.store(false, Ordering::Release);
            }
        });

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            debug!("Menu event: {:?}", event);
            if event.id() == EXIT_MENU_ITEM_ID {
                self.stop_signal.store(true, Ordering::Release);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        egui_extras::install_image_loaders(ctx);
        let elgato_icon = egui::include_image!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/elgato_logo.png"
        ));
        let bulb_icon = egui::Image::new(egui::include_image!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/bulb_icon.png"
        )))
        .max_width(20.0)
        .rounding(5.0);

        if let Ok(rlock) = self.avahi.try_read() {
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

fn load_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
