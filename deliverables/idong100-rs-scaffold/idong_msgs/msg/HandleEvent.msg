# RF handle (wand / airtouch wristband) event over the 2.4 GHz link.
std_msgs/Header header
uint8 event             # 0 FOUND, 1 NOT_FOUND, 2 TIMEOUT, 3 FOUND_OVER,
                        # 4 BUTTON_DOWN, 5 BUTTON_UP
uint8 handle_id
uint8 button            # bitmask when event is BUTTON_*
