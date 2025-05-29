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

// Tau (soft torque limit) values based on actuator types.
const TAU_00: f32 = 9.8; // For robstride_00 actuators
const TAU_02: f32 = 11.9; // For robstride_02 actuators
const TAU_03: f32 = 42.0; // For robstride_03 actuators
const TAU_04: f32 = 84.0; // For robstride_04 actuators

// This matches the values in metadata.json from the K-Scale API.
pub const ACTUATOR_KP_KD: [(usize, f32, f32, f32); 20] = [
    (11, 100.0, 8.284, TAU_03),  // left_shoulder_pitch_03
    (12, 100.0, 8.257, TAU_03),  // left_shoulder_roll_03
    (13, 40.0, 0.945, TAU_02),   // left_shoulder_yaw_02
    (14, 40.0, 1.266, TAU_02),   // left_elbow_02
    (15, 20.0, 0.295, TAU_00),   // left_wrist_00
    (21, 100.0, 8.284, TAU_03),  // right_shoulder_pitch_03
    (22, 100.0, 8.257, TAU_03),  // right_shoulder_roll_03
    (23, 40.0, 0.945, TAU_02),   // right_shoulder_yaw_02
    (24, 40.0, 1.266, TAU_02),   // right_elbow_02
    (25, 20.0, 0.295, TAU_00),   // right_wrist_00
    (31, 150.0, 24.722, TAU_04), // left_hip_pitch_04
    (32, 200.0, 26.387, TAU_03), // left_hip_roll_03
    (33, 100.0, 3.419, TAU_03),  // left_hip_yaw_03
    (34, 150.0, 8.654, TAU_04),  // left_knee_04
    (35, 40.0, 0.99, TAU_02),    // left_ankle_02
    (41, 150.0, 24.722, TAU_04), // right_hip_pitch_04
    (42, 200.0, 26.387, TAU_03), // right_hip_roll_03
    (43, 100.0, 3.419, TAU_03),  // right_hip_yaw_03
    (44, 150.0, 8.654, TAU_04),  // right_knee_04
    (45, 40.0, 0.99, TAU_02),    // right_ankle_02
];

pub const HOME_POSITION: [(usize, f32); 20] = [
    (21, 0.0),
    (22, (-10.0_f32).to_radians()),
    (23, 0.0),
    (24, 90.0_f32.to_radians()),
    (25, 0.0),
    (11, 0.0),
    (12, 10.0_f32.to_radians()),
    (13, 0.0),
    (14, (-90.0_f32).to_radians()),
    (15, 0.0),
    (41, (-20.0_f32).to_radians()),
    (42, (-0.0_f32).to_radians()),
    (43, 0.0),
    (44, (-50.0_f32).to_radians()),
    (45, 30.0_f32.to_radians()),
    (31, 20.0_f32.to_radians()),
    (32, 0.0_f32.to_radians()),
    (33, 0.0),
    (34, 50.0_f32.to_radians()),
    (35, (-30.0_f32).to_radians()),
];
