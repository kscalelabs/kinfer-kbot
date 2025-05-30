use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

// Global command state
static COMMAND_X: AtomicU32 = AtomicU32::new(0);
static COMMAND_Y: AtomicU32 = AtomicU32::new(0);
static COMMAND_YAW: AtomicU32 = AtomicU32::new(0);
static KEYBOARD_RUNNING: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn get_commands() -> [f32; 3] {
    [
        f32::from_bits(COMMAND_X.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_Y.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_YAW.load(Ordering::Relaxed)),
    ]
}

#[inline]
fn set_command(index: usize, value: f32) {
    let bits = value.to_bits();
    match index {
        0 => COMMAND_X.store(bits, Ordering::Relaxed),
        1 => COMMAND_Y.store(bits, Ordering::Relaxed),
        2 => COMMAND_YAW.store(bits, Ordering::Relaxed),
        _ => {}
    }
}

pub async fn prepare_keyboard_listener() -> Result<(), Box<dyn std::error::Error>> {
    println!("Keyboard controls will be available after startup:");
    println!("  W/S: X velocity (forward/backward)");
    println!("  A/D: Y velocity (left/right)");
    println!("  Q/E: Yaw rate (turn left/right)");
    println!("  Space: Reset all commands");
    println!("  ESC: Exit program gracefully");
    Ok(())
}

pub fn start_keyboard_listener_now() {
    KEYBOARD_RUNNING.store(true, Ordering::Relaxed);

    std::thread::spawn(move || {
        if let Err(e) = enable_raw_mode() {
            eprintln!("Failed to enable raw mode: {}", e);
            return;
        }

        while KEYBOARD_RUNNING.load(Ordering::Relaxed) {
            match event::poll(Duration::from_millis(20)) {
                Ok(true) => {
                    if let Ok(Event::Key(KeyEvent { code, kind, .. })) = event::read() {
                        // Handle ESC as graceful shutdown signal
                        if matches!(code, KeyCode::Esc) && kind == KeyEventKind::Press {
                            println!("\nESC pressed - requesting graceful shutdown...");
                            SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
                            KEYBOARD_RUNNING.store(false, Ordering::Relaxed);
                            break;
                        }

                        // Streamlined key handling
                        match (kind, code) {
                            (KeyEventKind::Press, KeyCode::Char('w')) => set_command(0, 0.5),
                            (KeyEventKind::Press, KeyCode::Char('s')) => set_command(0, -0.5),
                            (KeyEventKind::Press, KeyCode::Char('a')) => set_command(1, 0.5),
                            (KeyEventKind::Press, KeyCode::Char('d')) => set_command(1, -0.5),
                            (KeyEventKind::Press, KeyCode::Char('q')) => set_command(2, 0.5),
                            (KeyEventKind::Press, KeyCode::Char('e')) => set_command(2, -0.5),
                            (KeyEventKind::Press, KeyCode::Char(' ')) => {
                                // Batch reset for efficiency
                                COMMAND_X.store(0, Ordering::Relaxed);
                                COMMAND_Y.store(0, Ordering::Relaxed);
                                COMMAND_YAW.store(0, Ordering::Relaxed);
                            }
                            (KeyEventKind::Release, KeyCode::Char('w' | 's')) => {
                                set_command(0, 0.0)
                            }
                            (KeyEventKind::Release, KeyCode::Char('a' | 'd')) => {
                                set_command(1, 0.0)
                            }
                            (KeyEventKind::Release, KeyCode::Char('q' | 'e')) => {
                                set_command(2, 0.0)
                            }
                            _ => {}
                        }
                    }
                }
                Ok(false) => {
                    // Sleep for a bit longer when no events
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }

        let _ = disable_raw_mode();
    });
}

pub fn is_keyboard_running() -> bool {
    KEYBOARD_RUNNING.load(Ordering::Relaxed)
}

pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

pub fn cleanup_keyboard() {
    KEYBOARD_RUNNING.store(false, Ordering::Relaxed);
    let _ = disable_raw_mode();
}
