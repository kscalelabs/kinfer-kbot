use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

// Global command state
static COMMAND_X: AtomicU32 = AtomicU32::new(0);
static COMMAND_Y: AtomicU32 = AtomicU32::new(0);
static COMMAND_YAW_RATE: AtomicU32 = AtomicU32::new(0);
static COMMAND_YAW: AtomicU32 = AtomicU32::new(0);
static COMMAND_HEIGHT: AtomicU32 = AtomicU32::new(0);
static COMMAND_PITCH: AtomicU32 = AtomicU32::new(0);
static COMMAND_ROLL: AtomicU32 = AtomicU32::new(0);
static KEYFRAME_INDEX: AtomicU32 = AtomicU32::new(0);
static KEYBOARD_RUNNING: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn get_commands() -> [f32; 8] {
    [
        f32::from_bits(COMMAND_X.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_Y.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_YAW_RATE.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_YAW.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_HEIGHT.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_ROLL.load(Ordering::Relaxed)),
        f32::from_bits(COMMAND_PITCH.load(Ordering::Relaxed)),
        f32::from_bits(KEYFRAME_INDEX.load(Ordering::Relaxed)),
    ]
}

#[inline]
fn set_command(index: usize, value: f32) {
    let bits = value.to_bits();
    match index {
        0 => COMMAND_X.store(bits, Ordering::Relaxed),
        1 => COMMAND_Y.store(bits, Ordering::Relaxed),
        2 => COMMAND_YAW_RATE.store(bits, Ordering::Relaxed),
        3 => COMMAND_YAW.store(bits, Ordering::Relaxed),
        4 => COMMAND_HEIGHT.store(bits, Ordering::Relaxed),
        5 => COMMAND_ROLL.store(bits, Ordering::Relaxed),
        6 => COMMAND_PITCH.store(bits, Ordering::Relaxed),
        7 => KEYFRAME_INDEX.store(bits, Ordering::Relaxed),
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
            // Block until an event is available (no polling!)
            // This uses zero CPU when no keys are pressed
            match event::read() {
                Ok(Event::Key(KeyEvent { code, kind, .. })) => {
                    // Handle ESC as graceful shutdown signal
                    if matches!(code, KeyCode::Esc) && kind == KeyEventKind::Press {
                        println!("\nESC pressed - requesting graceful shutdown...");
                        SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
                        KEYBOARD_RUNNING.store(false, Ordering::Relaxed);
                        break;
                    }

                    // Handle key events immediately when they occur
                    match (kind, code) {
                        (KeyEventKind::Press, KeyCode::Char('w')) => set_command(0, 0.2),
                        (KeyEventKind::Press, KeyCode::Char('s')) => set_command(0, -0.2),
                        (KeyEventKind::Press, KeyCode::Char('a')) => set_command(1, 0.2),
                        (KeyEventKind::Press, KeyCode::Char('d')) => set_command(1, -0.2),
                        (KeyEventKind::Press, KeyCode::Char('q')) => {
                            let current_yaw = f32::from_bits(COMMAND_YAW.load(Ordering::Relaxed));
                            set_command(2, 0.1);
                            set_command(3, current_yaw + 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('e')) => {
                            let current_yaw = f32::from_bits(COMMAND_YAW.load(Ordering::Relaxed));
                            set_command(2, -0.1);
                            set_command(3, current_yaw - 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('r')) => {
                            let current_roll = f32::from_bits(COMMAND_ROLL.load(Ordering::Relaxed));
                            set_command(5, current_roll + 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('f')) => {
                            let current_roll = f32::from_bits(COMMAND_ROLL.load(Ordering::Relaxed));
                            set_command(5, current_roll - 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('t')) => {
                            let current_pitch =
                                f32::from_bits(COMMAND_PITCH.load(Ordering::Relaxed));
                            set_command(6, current_pitch + 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('g')) => {
                            let current_pitch =
                                f32::from_bits(COMMAND_PITCH.load(Ordering::Relaxed));
                            set_command(6, current_pitch - 0.1);
                        }
                        (KeyEventKind::Press, KeyCode::Char('6')) => {
                            set_command(7, 6.0);
                        }
                        (KeyEventKind::Press, KeyCode::Char('7')) => {
                            set_command(7, 7.0);
                        }
                        (KeyEventKind::Press, KeyCode::Char('8')) => {
                            set_command(7, 8.0);
                        }
                        (KeyEventKind::Press, KeyCode::Char('9')) => {
                            set_command(7, 9.0);
                        }
                        (KeyEventKind::Press, KeyCode::Char('2')) => {
                            COMMAND_X.store(0, Ordering::Relaxed);
                            COMMAND_Y.store(0, Ordering::Relaxed);
                            COMMAND_YAW.store(0, Ordering::Relaxed);
                            COMMAND_YAW_RATE.store(0, Ordering::Relaxed);
                            COMMAND_HEIGHT.store(0, Ordering::Relaxed);
                            COMMAND_PITCH.store(0, Ordering::Relaxed);
                            COMMAND_ROLL.store(0, Ordering::Relaxed);
                        }
                        (KeyEventKind::Release, KeyCode::Char('w' | 's')) => set_command(0, 0.0),
                        (KeyEventKind::Release, KeyCode::Char('a' | 'd')) => set_command(1, 0.0),
                        (KeyEventKind::Release, KeyCode::Char('q' | 'e')) => set_command(2, 0.0),
                        _ => {}
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    break;
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
