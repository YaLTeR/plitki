![Build Status](https://img.shields.io/github/actions/workflow/status/YaLTeR/plitki/ci.yml?branch=master&style=flat-square)
<!-- [![Coverage (Codecov)](https://img.shields.io/codecov/c/github/YaLTeR/plitki?logo=codecov&style=flat-square)](https://codecov.io/gh/YaLTeR/plitki) -->

Plitki is an experimental vertical scrolling rhythm game engine. I wrote it to get a better understanding of the underlying systems and to test a few design ideas I had.

### `plitki-core`

This is the core engine logic. It is `no_std` and has minimal dependencies. It supports the basic set of functionality one might expect from a VSRG engine:
- variable lane count
- regular objects
- long notes
- scroll velocities
- timing lines
- global and local offset
- very basic hit handling and judgement for gameplay

The main unusual design decision was to use only integer storage and math. All object, timing point and SV timestamps and positions are stored and operated on as integers, which works surprisingly well and without any precision issues. The values are stored in fixed-point format (e.g. a 1× scroll velocity is stored as 1000, so the value 10 for example means a 0.01× scroll velocity). Bitness and acceptable value ranges are carefully chosen so that no integer overflow can occur during a typical computation pipeline.

The second design decision was to have timing points not affect the scroll velocities. The plitki implementation showed that it is possible to losslessly convert between the more usual "timing points affect SVs" format and the "timing points do not affect SVs" format. After heavy randomized testing, the code [went on to power](https://github.com/Quaver/Quaver.API/pull/80) the "timing points do not affect SVs" format in the [Quaver] VSRG.

Type safety and newtypes are used to good extent to prevent mistakes such as using map timestamps as positions (ignoring SV), using game timestamps as map timestamps (ignoring global and local offset) or using positions as screen positions (ignoring the player's scroll speed).

`plitki-core` is extensively tested with both manual and randomized tests using `proptest`. The tests check for logic errors as well as absence of undocumented panics with arbitrary input.

### `plitki-map-qua`

This crate implements reading and writing of the `.qua` map format (used by the [Quaver] VSRG) and conversion to and from `plitki-core`'s `Map` type. Conversion correctness and losslessness is thoroughly tested, however panic safety currently isn't. It's likely that trying to convert a specifically constructed `.qua` to a `Map` will cause panics.

### `plitki-ui-wayland`

A simple UI for playing `.qua` maps using low-level Wayland bindings ([`smithay-client-toolkit`](https://lib.rs/crates/smithay-client-toolkit)) and [`glium`](https://lib.rs/crates/glium). My interest in this somewhat dropped due to lack of audio timing interfaces in [`rodio`](https://lib.rs/crates/rodio) at the time. Currently somewhat bitrotted and doesn't always show a window from the first try for some reason.

There are a few interesting things `plitki-ui-wayland` does.

- It renders only on Wayland frame callbacks, which means not straining the GPU unnecessarily. I later [implemented](https://github.com/Quaver/MonoGame/pull/3/commits/71fa189880b1fda8a1a1e18029da62fbad81d5ce) this logic in [Quaver] where it also works very well.

- It renders the playfield in a separate thread so as to not block input processing. The game state is stored in a [`triple_buffer`](https://lib.rs/crates/triple_buffer) so the input thread can update it even while the rendering is in progress, and the rendering thread always gets the latest available game state to draw.

- It predicts the presentation time for the next frame (a high-precision timestamp for when the monitor will display the next frame) and renders the frame for that presentation time. This way, even at low FPS there's no visual delay.

### `plitki-gtk`

This crate contains GTK 4 widgets for drawing a VSRG playfield using `plitki-core`. It comes with a demo app that can open and show `.qua` maps.

![Screenshot of the demo app.](plitki-gtk/screenshot.png)

### `plitki-gnome`

A test application using widgets from `plitki-gtk`.

Building `plitki-gnome` requires [Blueprint].

[Quaver]: https://quavergame.com/
[Blueprint]: https://gitlab.gnome.org/jwestman/blueprint-compiler