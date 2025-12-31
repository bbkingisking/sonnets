mod config;

use crate::config::Config;

fn main() {
    // Step 1: load config
    let config = Config::load();
}
