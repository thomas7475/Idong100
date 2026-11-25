cmake_minimum_required(VERSION 3.8)
project(idong_msgs)

find_package(ament_cmake REQUIRED)
find_package(rosidl_default_generators REQUIRED)
# Rust generation: rosidl_generator_rs must be in the workspace (see
# docs in the draft, section 2.2); it registers itself as an extension.
find_package(std_msgs REQUIRED)
find_package(sensor_msgs REQUIRED)
find_package(diagnostic_msgs REQUIRED)

rosidl_generate_interfaces(${PROJECT_NAME}
  "msg/MarkerArray.msg"
  "msg/HandleEvent.msg"
  "srv/DeviceInfo.srv"
  DEPENDENCIES std_msgs sensor_msgs
)

ament_package()
