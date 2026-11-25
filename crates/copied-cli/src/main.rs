use copied_core::TARGETS;

fn main() {
    for target in TARGETS {
        println!("{}\t{}", target.id, target.label);
    }
}
