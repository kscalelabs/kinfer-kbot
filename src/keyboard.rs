use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

// Global command state - just the bare minimum
static COMMAND_X: AtomicU32 = AtomicU32::new(0);
static COMMAND_Y: AtomicU32 = AtomicU32::new(0);
static COMMAND_YAW: AtomicU32 = AtomicU32::new(0);

pub fn get_commands() -> [f32; 3] {
    [
        f32::from_bits(COMMAND_X.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_Y.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_YAW.load(Ordering::Relaxed)),
    ]
}

fn set_command(index: usize, value: f32) {
    let bits = value.to_bits();
    match index {
        0 => COMMAND_X.store(bits, Ordering::Relaxed),
        1 => COMMAND_Y.store(bits, Ordering::Relaxed),
        2 => COMMAND_YAW.store(bits, Ordering::Relaxed),
        _ => {}
    }
}

pub async fn start_keyboard_listener() -> Result<(), Box<dyn std::error::Error>> {
    println!("Keyboard controls: W/S=X, A/D=Y, Q/E=Yaw, Space=Reset");
    println!("Commands stop when keys are released");

    // Completely separate OS thread
    std::thread::spawn(move || {
        let _ = enable_raw_mode();

        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                if let Ok(Event::Key(KeyEvent { code, kind, .. })) = event::read() {
                    match kind {
                        KeyEventKind::Press => match code {
                            KeyCode::Char('w') => set_command(0, 0.5),
                            KeyCode::Char('s') => set_command(0, -0.5),
                            KeyCode::Char('a') => set_command(1, 0.5),
                            KeyCode::Char('d') => set_command(1, -0.5),
                            KeyCode::Char('q') => set_command(2, 0.5),
                            KeyCode::Char('e') => set_command(2, -0.5),
                            KeyCode::Char(' ') => {
                                set_command(0, 0.0);
                                set_command(1, 0.0);
                                set_command(2, 0.0);
                            }
                            _ => {}
                        },
                        KeyEventKind::Release => {
                            // Zero out commands on key release
                            match code {
                                KeyCode::Char('w') | KeyCode::Char('s') => set_command(0, 0.0),
                                KeyCode::Char('a') | KeyCode::Char('d') => set_command(1, 0.0),
                                KeyCode::Char('q') | KeyCode::Char('e') => set_command(2, 0.0),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Long sleep to minimize CPU usage and interference
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    Ok(())
}

pub fn cleanup_keyboard() {
    let _ = disable_raw_mode();
}
