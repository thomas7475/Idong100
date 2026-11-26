"""Launch the i-Dong node with lifecycle-style bring-up (P4.2).

Starts the node, calls ~/transition configure+activate, and passes the
parameter file with sane defaults (see draft table 5).
"""
from launch import LaunchDescription
from launch_ros.actions import Node


def generate_launch_description():
    return LaunchDescription([
        Node(
            package="idong_ros2",
            executable="idong_node",
            name="idong",
            output="screen",
            parameters=["params.yaml"],
        ),
    ])
