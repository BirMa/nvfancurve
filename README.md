# nvfancurve

## what?

Very simple rust app that periodically checks the current (nvidia) GPU temps and adjusts the fan speed percentage accordingly (if it deems the temperature delta justifies adjusting the fan speed that is).
It does so based on a (currently hard coded) fan curve.  
It allows adjusting the fan speed without switching to a "rootful" X.  
The X11 part is inspired by https://github.com/foucault/nvfancontrol/, check it out.

## how?

Uses xhost to push `root` to the acl of the X server of the current DISPLAY, then uses `nvidia-settings` to update the fon speed percentage.
The dance with root is needed because allowing non-root to update fan speed is a critical security issue for nvidia. Otherwise you'd need to run X as root.

## prerequisites

Be in a running X session.  
Have `nvidia-settings` & `sudo`.
Only tested with my MSI 3080 Ti, which has 3 fans, two of which appear to share one fan header.
Not tested with anything else.

## build & run

`./start.sh`

Use `RUST_LOG` env variable to adjust log level if you're curious what it does and when. Defaults to `info` level, which is very quiet.
