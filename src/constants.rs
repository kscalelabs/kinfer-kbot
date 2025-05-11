pub const ACTUATOR_NAME_TO_ID: &[(&str, u32)] = &[
    ("dof_left_shoulder_pitch_03", 11),
    ("dof_left_shoulder_roll_03", 12),
    ("dof_left_shoulder_yaw_02", 13),
    ("dof_left_elbow_02", 14),
    ("dof_left_wrist_00", 15),
    ("dof_right_shoulder_pitch_03", 21),
    ("dof_right_shoulder_roll_03", 22),
    ("dof_right_shoulder_yaw_02", 23),
    ("dof_right_elbow_02", 24),
    ("dof_right_wrist_00", 25),
    ("dof_left_hip_pitch_04", 31),
    ("dof_left_hip_roll_03", 32),
    ("dof_left_hip_yaw_03", 33),
    ("dof_left_knee_04", 34),
    ("dof_left_ankle_02", 35),
    ("dof_right_hip_pitch_04", 41),
    ("dof_right_hip_roll_03", 42),
    ("dof_right_hip_yaw_03", 43),
    ("dof_right_knee_04", 44),
    ("dof_right_ankle_02", 45),
];

// Kp values based on actuator types or common settings
const KP_00: f32 = 20.0; // For robstride_00 actuators
const KP_02: f32 = 40.0; // For robstride_02 actuators
const KP_03: f32 = 100.0; // For some robstride_03 actuators
const KP_04: f32 = 150.0; // For robstride_04 actuators
const KP_LARGE: f32 = 200.0; // For other robstride_03 actuators

// Tau (soft torque limit) values based on actuator types
const TAU_00: f32 = 9.8; // For robstride_00 actuators
const TAU_02: f32 = 11.9; // For robstride_02 actuators
const TAU_03: f32 = 42.0; // For robstride_03 actuators
const TAU_04: f32 = 84.0; // For robstride_04 actuators

// Kd values are specified directly in the ACTUATOR_KP_KD array
// as they are highly specific to each joint configuration.

// This is a mapping from the actuator ID to the PID gains and torque limits.
// (Actuator ID, Kp, Kd, Tau_limit)
pub const ACTUATOR_KP_KD: [(usize, f32, f32, f32); 20] = [
    (11, KP_03, 8.284, TAU_03),     // left_shoulder_pitch_03
    (12, KP_03, 8.257, TAU_03),     // left_shoulder_roll_03
    (13, KP_02, 0.945, TAU_02),     // left_shoulder_yaw_02
    (14, KP_02, 1.266, TAU_02),     // left_elbow_02
    (15, KP_00, 0.295, TAU_00),     // left_wrist_00
    (21, KP_03, 8.284, TAU_03),     // right_shoulder_pitch_03
    (22, KP_03, 8.257, TAU_03),     // right_shoulder_roll_03
    (23, KP_02, 0.945, TAU_02),     // right_shoulder_yaw_02
    (24, KP_02, 1.266, TAU_02),     // right_elbow_02
    (25, KP_00, 0.295, TAU_00),     // right_wrist_00
    (31, KP_04, 24.722, TAU_04),    // left_hip_pitch_04
    (32, KP_LARGE, 26.387, TAU_03), // left_hip_roll_03
    (33, KP_03, 3.419, TAU_03),     // left_hip_yaw_03
    (34, KP_04, 8.654, TAU_04),     // left_knee_04
    (35, KP_02, 0.99, TAU_02),      // left_ankle_02
    (41, KP_04, 24.722, TAU_04),    // right_hip_pitch_04
    (42, KP_LARGE, 26.387, TAU_03), // right_hip_roll_03
    (43, KP_03, 3.419, TAU_03),     // right_hip_yaw_03
    (44, KP_04, 8.654, TAU_04),     // right_knee_04
    (45, KP_02, 0.99, TAU_02),      // right_ankle_02
];
