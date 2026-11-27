#[path = "../game/build_icon.rs"]
mod build_icon;

fn main() {
    build_icon::link_icon("openbf-launcher");
}
