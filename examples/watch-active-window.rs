use std::{thread, time::Duration};

fn main() {
    let mut last = None;
    loop {
        let current = active_window::active_window();

        if current != last {
            last = current;
            if let Some(window) = &last {
                println!("{:?}", window);
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}
