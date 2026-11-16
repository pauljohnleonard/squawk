# Hardware notes

Notes for a battery or USB powered box that plays squawk with no screen. The prototype board is a Raspberry Pi 5 with 1GB of RAM.

The engine sends 48 kHz, 32-bit samples. A Pi 4 and a Pi 5 produce the same bits. What you hear is decided by the amplifier and the speakers.

## Why the Pi 5

On an M1 Pro, one full handpan (8 pads, 8 modes each, the cavity, and the sympathetic links) uses 1.6% of one core to stay in real time. Silent pads cost the same as ringing ones, because every pad is stepped on every sample. Scaled from single-core Geekbench, that handpan is about 5–10% of one Pi 5 core and about 15% of one Pi 4 core. Treat the Pi figures as a factor-of-two estimate until the benchmark below is run on the board.

A small rig is about three of those handpans: two physical models of this weight, the smaller 10-mode voice, and a delay, chorus, and plate reverb. That fits in well under half of one Pi 5 core, with room for another model. The same rig crowds a Pi 4.

The Pi 5 1GB is the board to buy. This program is a few megabytes, so the larger RAM options are unused, and in late 2026 those options cost much more. Official US list prices then were about $45 for a Pi 5 1GB, $35 for a Pi 4 1GB, and about $100 for a Pi 4 4GB.

## Cores

The Pi 4 has 4 Cortex-A72 cores at 1.8 GHz. The Pi 5 has 4 Cortex-A76 cores at 2.4 GHz. All four cores on a board are the same kind. There are no efficiency cores.

The audio callback in `squawk-app` runs on one thread and calls one instrument for each buffer. Extra cores do not make that thread faster. They can run other work beside it.

Separate instruments can use separate cores. Each one renders its buffer on its own core, and the audio thread adds the buffers together. The pads inside one handpan stay on one core: every sample, the sympathetic links read positions from the other pads, and splitting that loop across cores costs more than the arithmetic.

An effect on the mixed output runs after that add. It can share the audio thread, which on a Pi 5 still has most of the core free. Pipelining a heavy effect onto another core adds one buffer of latency.

Leave one core for the operating system. On the Pi 5 the remaining three are spare until several physical models, or a convolution reverb, are actually in the box.

Time the current callback on the Pi with:

```text
cargo run --release -p squawk-engine --example handpan_cpu
```

The printed fraction is CPU time divided by one second of audio.

## Sound

Digital audio leaves the 40-pin header as I2S: bit clock, left/right clock, and data, plus 5 V and ground. The same pins exist on the Pi 4 and the Pi 5. That stream goes straight into a class-D amplifier with an I2S input, and the amplifier drives the speakers. A stereo board built from two MAX98357A channels delivers about 3 W into each 4 Ω speaker from 5 V. At lap volume it averages well under a watt, so the Pi is the larger load on the battery. Two small full-range drivers in the shell are the right speakers for those few watts.

The app copies its one sample to every channel the sound card asks for, so both speakers play the handpan. A stereo effect later uses the same amplifier.

The Pi 5 has no headphone jack. The Pi 4 jack is a filtered pulse train on the computer's ground, and the hiss is audible under a long ring. Either board sounds like the instrument once the I2S amplifier is the output. Open that device at 48 kHz so nothing resamples the stream.

## The rest of the box

No screen. LEDs, or a small text display on the header, are enough to show which instrument or scale is active. Their CPU cost is negligible next to one handpan.

Power is a USB power bank. A Pi 5 playing this synth is about 5–8 W, and the amplifier averages under 1 W, so the box draws about 6 W. Five hours is 30 Wh at the board. A bank labeled 10,000 mAh (10 Ah) stores that at the cell voltage, about 3.7 V, which is 37 Wh, and delivers about 30 Wh after it boosts to 5 V. That is the five-hour pack. A battery that is 10 Ah at 5 V would be 50 Wh, larger than this box needs. A 20,000 mAh bank covers the same five hours if the draw sits at the 8 W end, and it lasts longer at the likely draw. The bank needs to hold 5 V at 3 A while it does this. The Pi 5's 5 A supply warning can be ignored: this box has no USB devices hanging off it.

The Pi does not charge a battery. Its USB-C port is power in. A power bank already contains the charger: you charge the bank from a USB plug, then the bank powers the Pi. An internal pack needs its own charger board between the cells and the Pi.

## Pads

The first playing surface is eight 20 mm piezo discs, one under each pad, read by an Arduino already on hand. The Pi has no analog inputs. The Arduino sends each hit over its USB serial link as which pad, how hard, and where on the pad. Squawk does not read that link yet. The Arduino can draw its power from a USB port on the Pi.

A fingertip on a bendy pad is one hump of about 5–20 ms, a few newtons when quiet and a few tens of newtons when hard. The synth keeps the peak of that hump, and where it landed, and plays its own 1.5 ms rise and 16 ms fall.

The discs have to be mounted so a hit stays on one pad:

- Each pad is supported around its rim on rubber or foam, with the center free to dish and a gap to the next pad. The shell can be one piece, and it must not be a stiff bridge between discs.
- The pad is a light, bendy piece of wood or plastic, so a finger dishes the center. The disc is glued under that center, with light foam behind it and a resistor across its leads. A thick slab barely bends, so the disc stays quiet. A lively pad keeps making voltage for 10–15 ms and the tail looks like a second hit.
- Lifting the finger makes a short spike of the opposite polarity. A diode, or the Arduino ignoring that swing, drops it.
- The strength sent to the Pi is the highest sample during the hump.

A disc glued under a pad can reach about 10 V. An unmounted disc can exceed 25 V, because a finger can bend it much farther than the pad will allow. The voltage follows that bend. Each input wants a voltage divider and a clamp down to the Arduino's pin voltage. An Uno has six analog inputs, so eight pads wants a board with eight, such as a Nano or a Mega.

## Prototype parts

Approximate US prices in late 2026. The Pi 5 figure is the official list price. The other lines are typical shop prices and move around.

| Part | Role | Estimate |
| --- | --- | --- |
| Raspberry Pi 5, 1GB | Computer | $45 |
| 32GB microSD card | Boot and the program | $10 |
| 10,000 mAh USB-C power bank, 5 V at 3 A | About 5 hours, with its own charger | $25 |
| USB-C cable | Bank to the Pi. Often included with the bank | $8 |
| Stereo MAX98357A I2S amplifier | Digital input, about 3 W per speaker | $10 |
| Two small 4 Ω full-range speakers | The sound of the box | $12 |
| LEDs, or a small I2C text display | Which instrument or scale is active | $5 |
| Jumper wires | Header to the amplifier and the display | $5 |
| Eight 20 mm piezo discs | One under each pad | already owned |
| Arduino with eight analog inputs | Read the discs, send the hit | already owned |
| Divider, clamp, and a resistor per disc | Protect the Arduino pins | a few dollars |
| Rubber or foam mounts | Keep a hit on one pad | a few dollars |
| **Total** | Shopping list, pads excluded | **about $120** |
