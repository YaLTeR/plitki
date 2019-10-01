use std::{fs::File, path::PathBuf};

use plitki_core::map::Map;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "dump-svs", about = "Prints normalized SV values.")]
struct Opt {
    /// Path to a supported map file.
    path: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    let file = File::open(opt.path).unwrap();
    let qua = plitki_map_qua::from_reader(file).unwrap();
    let map: Map = qua.into();

    println!("initial\t{}", map.initial_scroll_speed_multiplier.as_f32());
    for sv in map.scroll_speed_changes {
        println!(
            "{}\t{}",
            sv.timestamp.into_milli_hundredths(),
            sv.multiplier.as_f32()
        );
    }
}
