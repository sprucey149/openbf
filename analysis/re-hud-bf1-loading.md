# RE: BF1 loading display

The retail loading screen tracks a rectangle on each image, then expands that rectangle into the next image.

## Evidence

- **Timing and map targets.** BF1 PS2 `FUN_002c8aa0` (B1:254839–254845, 255042–255069) reads the common loading config. Each location gives the image name and a rectangle in that image's pixels. The PS2 and PC copies specify 3.5 seconds of tracking and one second of zoom. Xbox specifies 2.5 and 0.75 seconds. The last image has a zero rectangle because it is the destination.
- **Tracking.** `FUN_002c69c0` (B1:253911–253968) moves the horizontal edges toward the target Y in the first half of the tracking period. The vertical edges move toward target X in the second half. Each update takes a fraction of the remaining distance. The corresponding loop runs while the leading edge moves more than 0.001 of the image dimension during an update. A pause follows each scan while the edges settle.
- **Zoom.** `FUN_002c69c0` (B1:253849–253906) expands the target rectangle linearly to the screen bounds. The fade advances from the previous opacity on every update, so the new image becomes opaque before the expansion finishes. `FUN_002c7780` (B1:254101–254140) draws the outgoing image crop inside the growing window, blends in the incoming image, and composites the tracking trails, scanline and noise. The overlays fade on the final zoom.
- **Dial.** `FUN_002c5b88` (B1:253367–253383) places two sizes of the same dial sprite at (0.75, 0.7). `FUN_002c6468` (B1:253731–253747) advances them in opposite directions. The shared config supplies the dial and LED textures. The loading label uses the BF1 large game font (B1:253178–253188).
- **Sound.** Bespin Platforms and Hoth Echo Base captures from the PS2 retail game give about 0.7–0.8 seconds of audible tracking in each axis. The measured tones are near 793 Hz then 925 Hz. The zoom cue is a one-shot and ends before the following sweep. The retail sound properties are in [the BF1 sound-bank notes](re-sound-banks.md).

## Port behavior

The common loading bake reads the selected copy's timing, texture names and rectangles. The game normalizes each rectangle against its actual image dimensions. The render and sound use one elapsed-time sample. The scan interpolates between 50 ms samples of the original recurrence, based on the PS2 update's 40 ms minimum and the measured refresh cadence (B1:253731–253743).

The port holds the final image until the scene is ready. The two-press skip can end the presentation early. Maps without a BF1 location use the galaxy view. The loading bus applies a common output gain measured against the PS2 capture. The direct SPU2 gain, exact PS2 blend/filter output and the separately named transition sound remain open.
