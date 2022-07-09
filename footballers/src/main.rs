use footballers::components::FootballersApp;
use log::info;

fn main() {
    // TODO: make log level dynamic, for e.g. modifiable via a query parameter
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
    info!("Starting the app");
    yew::start_app::<FootballersApp>();
}
