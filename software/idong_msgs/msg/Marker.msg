# One tracked retro-reflector from the i-Dong sensor.
# Mirrors slot fields of the vendor xEye3Ddata record (xEyeClass.dll).
std_msgs/Header header   # stamp = host receive time; frame_id = sensor
uint32 seq              # device frame counter (continuity: delta <= 1)
uint8 slot              # 0..9 observed (chip spec: up to 15)
float32 x               # mm, device frame
float32 y               # mm
float32 z               # mm
int32 quality           # slot+0x24; >= 50 means tracked (vendor gate)
# No `age` field: turn 46 proved record +0x18 is z, not an age gate
# (the vendor GetADataFromUSB loop selects the smallest z); the core
# Marker struct and this definition mirror each other field-for-field.
