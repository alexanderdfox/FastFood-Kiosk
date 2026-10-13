use std::process::Command;

fn main() {
    let bios_path = env!("BIOS_PATH");

    let status = Command::new("qemu-system-x86_64")
        .arg("-drive")
        .arg(format!("format=raw,file={bios_path}"))
        .arg("-serial")
        .arg("stdio")
        .status()
        .expect("failed to run qemu");

    std::process::exit(status.code().unwrap_or(1));
}
