//! Prints what `pw-raw` finds in image files.
//!
//! `cargo run -p pw-raw --example inspect --release -- <file> [<file> ...]`

use std::path::Path;
use std::time::Instant;

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let t = Instant::now();
        match pw_raw::inspect(path) {
            Ok(info) => {
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                println!("{arg}  ({ms:.2} ms)");
                println!("  format      {:?}", info.format);
                println!("  camera      {:?} {:?}", info.make, info.model);
                println!("  lens        {:?}", info.lens);
                println!("  taken       {:?}", info.date_time_original);
                let exp = info.exposure_time.map(|e| e.to_string());
                println!(
                    "  exposure    {exp:?}  f/{:?}  ISO {:?}  {:?} mm  bias {:?}",
                    info.f_number, info.iso, info.focal_length, info.exposure_bias
                );
                println!("  orientation {}", info.orientation);
                println!("  image size  {:?} x {:?}", info.width, info.height);
                for p in &info.previews {
                    println!(
                        "  preview     {:>5} x {:<5} {:>9} bytes @ {:<9} {}",
                        p.width, p.height, p.length, p.offset, p.source
                    );
                }
            }
            Err(e) => println!("{arg}: {e}"),
        }
    }
}
