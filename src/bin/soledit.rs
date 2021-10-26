use std::env;

use amf::Amf3Value;
use egui::{DragValue, ScrollArea, TextEdit};
use sfml::{
    graphics::{Color, RenderTarget, RenderWindow},
    window::{Event, Style},
};

fn main() {
    let path = env::args_os().nth(1).expect("Need file path as argument");
    let mut sol = soledit::read_from_file(path.as_ref()).unwrap();
    let mut window = RenderWindow::new((640, 480), "SolEdit", Style::CLOSE, &Default::default());
    window.set_vertical_sync_enabled(true);
    let mut sf_egui = egui_sfml::SfEgui::new(&window);
    let mut filter_string = String::new();
    while window.is_open() {
        while let Some(event) = window.poll_event() {
            sf_egui.add_event(&event);
            if event == Event::Closed {
                window.close();
            }
        }
        sf_egui.do_frame(|ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(sol.root_name());
                ui.add(TextEdit::singleline(&mut filter_string).hint_text("Filter"));
                match &mut sol {
                    soledit::SolVariant::Amf0(sol) => {
                        ui_amf0(ui, &mut sol.root_object, &filter_string)
                    }
                    soledit::SolVariant::Amf3(sol) => {
                        ui_amf3(ui, &mut sol.root_object, &filter_string)
                    }
                }
            });
        });
        window.clear(Color::BLACK);
        sf_egui.draw(&mut window, None);
        window.display();
    }
    sol.write_to_file(path.as_ref()).unwrap();
}

fn ui_amf3(
    ui: &mut egui::Ui,
    root_object: &mut [soledit::Pair<soledit::Amf3Value>],
    filter_string: &str,
) {
    ScrollArea::vertical().show(ui, |ui| {
        for pair in root_object {
            if !pair.key.contains(filter_string) {
                continue;
            }
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut pair.key);
                match &mut pair.value {
                    Amf3Value::Undefined => todo!(),
                    Amf3Value::Null => ui.label("null"),
                    Amf3Value::Boolean(b) => ui.checkbox(b, ""),
                    Amf3Value::Integer(n) => ui.add(DragValue::new(n)),
                    Amf3Value::Double(n) => ui.add(DragValue::new(n)),
                    Amf3Value::String(s) => ui.text_edit_singleline(s),
                    Amf3Value::XmlDocument(_) => todo!(),
                    Amf3Value::Date { unix_time } => ui.label("<date>"),
                    Amf3Value::Array {
                        assoc_entries,
                        dense_entries,
                    } => ui.label("<array>"),
                    Amf3Value::Object {
                        class_name,
                        sealed_count,
                        entries,
                    } => ui.label("<object>"),
                    Amf3Value::Xml(_) => todo!(),
                    Amf3Value::ByteArray(_) => todo!(),
                    Amf3Value::IntVector { is_fixed, entries } => todo!(),
                    Amf3Value::UintVector { is_fixed, entries } => todo!(),
                    Amf3Value::DoubleVector { is_fixed, entries } => todo!(),
                    Amf3Value::ObjectVector {
                        class_name,
                        is_fixed,
                        entries,
                    } => todo!(),
                    Amf3Value::Dictionary { is_weak, entries } => todo!(),
                }
            });
        }
    });
}

fn ui_amf0(
    ui: &mut egui::Ui,
    root_object: &mut [soledit::Pair<soledit::Amf0Value>],
    filter_string: &str,
) {
    ScrollArea::vertical().show(ui, |ui| {
        for pair in root_object {
            if !pair.key.contains(filter_string) {
                continue;
            }
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut pair.key);
                match &mut pair.value {
                    soledit::Amf0Value::Num(n) => ui.add(DragValue::new(n)),
                    soledit::Amf0Value::Bool(b) => ui.checkbox(b, ""),
                    soledit::Amf0Value::String(s) => ui.text_edit_singleline(s),
                    soledit::Amf0Value::Object(_) => todo!(),
                }
            });
        }
    });
}
