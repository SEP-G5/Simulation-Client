mod app;
mod hash;
mod rest;
mod transaction;

// ========================================================================== //

fn main() {
    let app = match app::App::new("Simulation Client") {
        Ok(a) => a,
        Err(_) => panic!("Failed to create application"),
    };
    app.run();
}
