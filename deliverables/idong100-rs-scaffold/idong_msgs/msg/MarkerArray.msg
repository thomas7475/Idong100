# Up to 10-15 tracked markers per 20 ms frame (50 fps stream).
std_msgs/Header header
uint32 seq
Marker[] markers
uint8 link_quality      # 0 green / 1 yellow / 2 red (vendor thresholds)
uint8 mode              # record +0x3B8 mode flag
bool dropped            # frame-counter gap detected
