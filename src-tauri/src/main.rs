// Empêche l'ouverture d'une console noire sous Windows en version publiée.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sectool_app::run();
}
