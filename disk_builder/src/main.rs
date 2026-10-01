use bootloader::BiosBoot;
use std::env;
use std::path::Path;

fn main() {
    let mut args = env::args().skip(1);
    let kernel_path = args.next().expect("Arg 1: kernel binary path required");
    let out_path = args.next().expect("Arg 2: output image path required");

    println!("Building BIOS boot image for kernel: {}", kernel_path);
    let mut bios = BiosBoot::new(Path::new(&kernel_path));
    bios.create_disk_image(Path::new(&out_path)).expect("Failed to create disk image");
    println!("Successfully created boot image at {}", out_path);
}
