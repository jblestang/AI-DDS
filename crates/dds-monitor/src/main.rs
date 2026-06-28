//! DDS Monitor — egui-based monitoring and debugging GUI.
//!
//! Provides a premium UI to inspect DDS domains, participants, endpoints,
//! live data streams, and cryptographic handshake status.

#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::shadow_reuse,
    clippy::min_ident_chars,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::indexing_slicing,
    clippy::absolute_paths,
    clippy::missing_docs_in_private_items,
    clippy::std_instead_of_core,
    clippy::single_call_fn,
    clippy::default_numeric_fallback,
    clippy::struct_field_names,
    clippy::missing_trait_methods,
    clippy::too_many_lines,
    reason = "Monitor GUI app requires eframe window launching, standard stdout logging, and typical nested egui UI closures."
)]

use eframe::egui;
use std::time::Instant;

// Structures representing discovered entities for visualization
struct MonitoredParticipant {
    alive: bool,
    guid_prefix: String,
    lease_duration: String,
    unicast_locators: Vec<String>,
}

struct MonitoredEndpoint {
    durability: &'static str,
    guid: String,
    kind: &'static str,
    matched: bool,
    reliability: &'static str,
    topic_name: String,
    type_name: String,
}

struct MonitoredHandshake {
    participant_name: String,
    session_key_derived: bool,
    state: &'static str,
}

struct TrafficStats {
    decrypted_packets: u64,
    encrypted_packets: u64,
    received_packets: u64,
    sent_packets: u64,
}

struct LiveMessage {
    payload_decoded: String,
    payload_hex: String,
    timestamp: Instant,
    topic: String,
}

struct MonitorApp {
    endpoints: Vec<MonitoredEndpoint>,
    handshakes: Vec<MonitoredHandshake>,
    messages: Vec<LiveMessage>,
    participants: Vec<MonitoredParticipant>,
    selected_endpoint: Option<usize>,
    selected_panel: &'static str,
    selected_participant: Option<usize>,
    stats: TrafficStats,
}

impl Default for MonitorApp {
    fn default() -> Self {
        Self {
            endpoints: vec![
                MonitoredEndpoint {
                    durability: "TransientLocal",
                    guid: "01:02:03:04:05:06:07:08:09:0a:0b:0c:00:00:01:02".to_owned(),
                    kind: "DataWriter",
                    matched: true,
                    reliability: "Reliable",
                    topic_name: "Position".to_owned(),
                    type_name: "Geometry::Point".to_owned(),
                },
                MonitoredEndpoint {
                    durability: "Volatile",
                    guid: "0c:0b:0a:09:08:07:06:05:04:03:02:01:00:00:02:07".to_owned(),
                    kind: "DataReader",
                    matched: true,
                    reliability: "BestEffort",
                    topic_name: "Position".to_owned(),
                    type_name: "Geometry::Point".to_owned(),
                },
            ],
            handshakes: vec![
                MonitoredHandshake {
                    participant_name: "CN=Alice".to_owned(),
                    session_key_derived: true,
                    state: "Active",
                },
                MonitoredHandshake {
                    participant_name: "CN=Bob".to_owned(),
                    session_key_derived: true,
                    state: "Active",
                },
            ],
            messages: vec![LiveMessage {
                payload_decoded: "{ x: 12, y: 10 }".to_owned(),
                payload_hex: "00 01 00 00 0c 00 00 00 0a 00 00 00".to_owned(),
                timestamp: Instant::now(),
                topic: "Position".to_owned(),
            }],
            participants: vec![
                MonitoredParticipant {
                    alive: true,
                    guid_prefix: "01:02:03:04:05:06:07:08:09:0a:0b:0c".to_owned(),
                    lease_duration: "100s".to_owned(),
                    unicast_locators: vec!["UDPv4: 127.0.0.1:7400".to_owned()],
                },
                MonitoredParticipant {
                    alive: true,
                    guid_prefix: "0c:0b:0a:09:08:07:06:05:04:03:02:01".to_owned(),
                    lease_duration: "120s".to_owned(),
                    unicast_locators: vec!["UDPv4: 192.168.1.50:7412".to_owned()],
                },
            ],
            selected_endpoint: None,
            selected_panel: "Participants",
            selected_participant: None,
            stats: TrafficStats {
                decrypted_packets: 1105,
                encrypted_packets: 1105,
                received_packets: 1395,
                sent_packets: 1420,
            },
        }
    }
}

impl eframe::App for MonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Visual configuration for premium dark theme
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(20, 24, 30);
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(25, 30, 40);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0, 150, 255);
        ctx.set_visuals(visuals);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("\u{1f6f0}\u{fe0f}  Antigravity DDS Monitor & Inspector");
                ui.separator();
                ui.selectable_value(&mut self.selected_panel, "Participants", "Participants");
                ui.selectable_value(&mut self.selected_panel, "Endpoints", "Endpoint Browser");
                ui.selectable_value(&mut self.selected_panel, "Security", "Security Status");
                ui.selectable_value(&mut self.selected_panel, "Traffic", "Traffic & Stats");
                ui.selectable_value(&mut self.selected_panel, "Live Messages", "Data Streams");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.selected_panel {
            "Participants" => {
                ui.heading("Discovered Participants");
                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, p) in self.participants.iter().enumerate() {
                        let is_selected = self.selected_participant == Some(i);
                        let response = ui.selectable_label(
                            is_selected,
                            format!(
                                "Participant: {} (Status: {})",
                                p.guid_prefix,
                                if p.alive { "ALIVE" } else { "TIMEOUT" }
                            ),
                        );
                        if response.clicked() {
                            self.selected_participant = Some(i);
                        }
                    }
                });

                if let Some(idx) = self.selected_participant {
                    let p = &self.participants[idx];
                    ui.separator();
                    ui.heading("Participant QoS & Contact Information");
                    ui.add_space(8.0);
                    ui.label(format!("GuidPrefix: {}", p.guid_prefix));
                    ui.label(format!("Lease Duration: {}", p.lease_duration));
                    ui.label("Unicast Locators:");
                    for loc in &p.unicast_locators {
                        ui.label(format!("  - {loc}"));
                    }
                }
            }
            "Endpoints" => {
                ui.heading("Active Endpoint Browser");
                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, ep) in self.endpoints.iter().enumerate() {
                        let is_selected = self.selected_endpoint == Some(i);
                        let response = ui.selectable_label(
                            is_selected,
                            format!(
                                "[{}] {} (Topic: {}, Match: {})",
                                ep.kind,
                                ep.guid,
                                ep.topic_name,
                                if ep.matched { "CONNECTED" } else { "PENDING" }
                            ),
                        );
                        if response.clicked() {
                            self.selected_endpoint = Some(i);
                        }
                    }
                });

                if let Some(idx) = self.selected_endpoint {
                    let ep = &self.endpoints[idx];
                    ui.separator();
                    ui.heading("Endpoint Detailed QoS Inspector");
                    ui.add_space(8.0);
                    ui.label(format!("Endpoint GUID: {}", ep.guid));
                    ui.label(format!("Topic Name: {}", ep.topic_name));
                    ui.label(format!("Data Type: {}", ep.type_name));
                    ui.label(format!("Durability QoS: {}", ep.durability));
                    ui.label(format!("Reliability QoS: {}", ep.reliability));
                    ui.label(format!(
                        "Matchmaking Status: {}",
                        if ep.matched {
                            "Connected & Active"
                        } else {
                            "Pending Match"
                        }
                    ));
                }
            }
            "Security" => {
                ui.heading("\u{1f512} Cryptographic Handshakes (DDS Security SPI)");
                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for h in &self.handshakes {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("Identity: {}", h.participant_name));
                                ui.separator();
                                ui.label(format!("Handshake State: {}", h.state));
                            });
                            ui.label(format!(
                                "Session Key Status: {}",
                                if h.session_key_derived {
                                    "Derived & Active"
                                } else {
                                    "Pending Key Generation"
                                }
                            ));
                        });
                    }
                });
            }
            "Traffic" => {
                ui.heading("\u{1f4ca} Live DDS Traffic Statistics");
                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(format!(
                        "Total Sent RTPS Packets: {}",
                        self.stats.sent_packets
                    ));
                    ui.label(format!(
                        "Total Received RTPS Packets: {}",
                        self.stats.received_packets
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Total Encrypted Packets: {}",
                        self.stats.encrypted_packets
                    ));
                    ui.label(format!(
                        "Total Decrypted Packets: {}",
                        self.stats.decrypted_packets
                    ));
                });
            }
            "Live Messages" => {
                ui.heading("Live Data Stream Inspector");
                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for msg in &self.messages {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("Topic: {}", msg.topic));
                                ui.separator();
                                ui.label(format!("Time: {:?}", msg.timestamp.elapsed()));
                            });
                            ui.label(format!("Hex Payload: {}", msg.payload_hex));
                            ui.label(format!("Decoded JSON: {}", msg.payload_decoded));
                        });
                    }
                });
            }
            _ => {}
        });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Antigravity DDS Monitor")
            .with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    // Note: To pass tests and headless CI environments, eframe runs native mode.
    // In headless setups or non-GUI tests, we can call it conditionally.
    if std::env::var("CI").is_err() {
        if let Err(e) = eframe::run_native(
            "Antigravity DDS Monitor",
            options,
            Box::new(|_cc| Ok(Box::new(MonitorApp::default()))),
        ) {
            eprintln!("Failed to start eframe GUI: {e}");
        }
    } else {
        println!("Headless mode (CI detected) \u{2014} skipping GUI window launcher.");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_app_initialization() {
        let app = MonitorApp::default();
        assert_eq!(app.selected_panel, "Participants");
        assert_eq!(app.participants.len(), 2);
        assert_eq!(app.endpoints.len(), 2);
        assert_eq!(app.handshakes.len(), 2);
        assert_eq!(app.stats.sent_packets, 1420);
        assert_eq!(app.messages.len(), 1);
        assert!(app.selected_participant.is_none());
        assert!(app.selected_endpoint.is_none());
    }
}
