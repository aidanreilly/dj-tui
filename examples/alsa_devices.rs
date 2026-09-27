//! Print the playback devices the ALSA backend would offer, for checking the list by eye.
fn main() {
    for device in backend::alsa_devices() {
        let kind = if device.hardware { "card" } else { "plugin" };
        println!("{:<24} {kind:<7} {}", device.name, device.description);
    }
}
