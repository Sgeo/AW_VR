# AW_VR
Experience Active Worlds in Virtual Reality on Oculus Rift.

Only works in AW versions prior to 6.0, and only works in OpenGL mode.

Tested on Active Worlds 4.2 standalone mode.

## How to

1. Download from https://github.com/Sgeo/AW_VR/releases
1. Ensure Active Worlds is using OpenGL mode
1. While Active Worlds is running, run aw_vr_injector.exe

## Controls

1. All motion is relevant to your avatar, which only moves with AW movement and not with your head. Press the menu button to recenter, which will put your view into alignment with the avatar.
1. Make sure the AW window is active when using the controllers.
1. Controls subject to change.
1. Left stick = forward/back. Push all the way forward/back for Ctrl-run.
1. Right stick = turn left/right.

## Limitations

1. Does NOT work on Active Worlds 6.x (the current version). The injector, as written, will not locate it, and I have no way to test its functionality even with the injector fixed.
1. At this point in time, OpenGL mode ONLY. AW, by default, is on DirectX, and DirectX model caches do not work properly in OpenGL mode.
1. Some worlds may have a simpler sky, which will cause the world to not render properly at this time. e.g. as of a few years ago, AWGZ had the simpler sky, AW has a sky compatible with this code.
1. Fast moving objects and rotating objects may not show up with a sensible depth.
1. Fast animations may look weird and uncomfortable.
1. No guarantees about suitable framerate. I personally do not get VR-sick. If you are prone to VR-sickness, be careful.