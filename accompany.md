# Accompaniment

Notes on the layer that plays under the handpan, as it works today. The code is `crates/squawk-engine/src/support.rs`. The audio callback adds it to the handpan only while the handpan is the selected instrument.

The handpan is still the voice. This layer is a pad under it. It has three parts: three resonant bands, one soft bass tone, and an occasional quiet tap.

## What it hears

Every handpan strike arrives as a note and a strength. The note is snapped to the nearest of the ten pads in the current scale. The strength is the same amplitude the handpan was struck with.

The current scale, ding, and tuning (Equal or Just) are the same ones the handpan is using. Changing them retunes this layer. The bands and the bass glide to the new pitches. A tap that is already aimed snaps.

The last 16 strikes are kept: when they happened, which scale degree they were, and how hard. Harder strikes count for more. A very soft strike still counts a little.

A strike closer than 80 ms to the previous one is a flam. It updates which notes have been played, and the bands follow that. It does not move the beat, and it does not reset the swell.

A gap of 2.5 seconds or more is a pause. The next strike starts the swell again from the top. The long gap is not treated as a beat.

Strike position (center, between, shoulder) is not used. Only the note and the strength come across.

## The beat

Before any playing, the layer assumes a beat of 0.55 seconds, about 109 bpm. The swell stays still until three real gaps have been heard.

Each accepted gap is the time between strikes, ignoring flams and pauses. The beat is the median of the last six of those gaps, so one late note does not throw it. After the first gap, each new median only pulls the beat 20% of the way. A tempo change takes several notes to settle. The beat is kept between 0.25 and 1.6 seconds.

The swell phase goes back to zero on every accepted strike, so the loud part of the cycle sits on the strike. If a strike does not come, the phase keeps running and wraps on its own. That free-running wrap still counts as a beat for the bass vote below. After a rest, the swell is no longer heard. See Starting and stopping.

## The controls

While the handpan is selected, the right-hand column under the peak meter is the accompaniment. The positions are saved with the other settings, and the next launch starts from them. Reset to defaults writes the factory values back, including these sliders and the "Rows set where the strike lands" checkbox.

Support level scales the whole layer: bands, bass, and taps. It runs from 0 to 1 and starts at 0.35. At 0 the output is silent. The layer keeps listening and keeps its resonators moving, so turning the level back up does not start from nothing.

Support pulse is how deep the swell is. It runs from 0 to 1 and starts at 0.55. At 0 the bands hold a steady level. At 1 they dip to about a third of their peak halfway through the beat. The default 0.55 is a moderate breath, down to about two thirds at the midpoint. Until three gaps have been heard, the pulse knob is ignored and the layer stays steady.

The bass takes only a third of that swell, and it sits on a floor, so the low note stays present while the bands breathe.

Width runs from 0 to 1 and starts at 1. The bands are noise through a resonator, and this slider is how much of that noise they keep. At 1 each band uses the widths in the table below, which is the sound the layer started with. The ding band then opens from a damping of 0.055 to 0.17, about 15 Hz to 50 Hz around D3, and that open setting is the wash. At 0 all three bands sit near a damping of 0.01, a narrow tone with a little breath. A lower Width is the more tonal band. The bass and the taps are already narrow tones. They do not follow this slider.

Wander is the random walk of each band's center, in cents. It runs from 0 to 40 and starts at 14. A new target is picked every 0.9 to 2.1 seconds and approached over about 1.8 seconds. At 0 the center stays on the aim. The note it belongs to does not change.

Drift is the slow slide of the filter, in cents, on the same oscillator as the width. It runs from 0 to 60 and starts at 26, which is the ding band. The two color bands keep their present share of that amount, 16 and 18 against 26, so they stay a little quieter in pitch than the ding band. At 0 the center does not slide with the filter.

Motion multiplies how fast the width opens and closes. It runs from 0.25 to 3 and starts at 1, the rates in the table. The small jitter that keeps the cycle off a clock stays in proportion.

Fade is how long the tone takes to leave after the 2 second hold. It runs from 0.5 to 12 seconds and starts at 4. The swell still falls over about a second, ahead of the tone.

## Starting and stopping

The layer is silent until the first strike. The resonators are already running, so the entrance is a fade of about half a second, not a click. A flam counts as playing.

For about 2 seconds after the last strike, the pad stays as it was. That is the same gap that stops the taps, and it is inside the 2.5 second pause that is not a beat. A breath between notes does not drop the accompaniment.

After that hold, two things happen. The swell depth falls over about a second, so the pulse stops while the tone is still there. The tone then fades over the Fade time, 4 seconds unless that slider has been moved. Bands, bass, and any remaining tap all go. This also takes away the slow wobble between two bands that are sitting on nearly the same note.

The next strike brings the layer back over the same half second. The remembered beat is still there, and that strike puts the swell back at the top of its cycle.

## The three bands

These are the pad. Each one is a mass-spring, the same kind of resonator as a handpan mode. It is driven by noise, and the sound taken from it is the velocity. That is a band of pitches around a center, not a sine.

The center each band is aiming at is always a note of the current scale.

- The first band aims at the ding, always.
- The second aims at the scale degree you have been playing most, over the last eight strikes. Newer strikes count more than older ones, and harder strikes count more than softer ones.
- The third aims at the next most played degree in that same window. If you have only been playing one degree, it sits with the second band.

When the aim changes, the center glides. The time constant is 0.45 seconds, so it is about halfway in half a second and settled after a bit more than a second.

Around that aim, two slow motions keep the band from sitting still. Wander and Drift, above, set how far they go. Motion sets how fast the opening and closing runs.

The width opens and closes. A very slow oscillator moves the damping between a narrower and a wider setting. Width and center are on the same oscillator, a quarter cycle apart, so the band slides as it opens. Each band has its own rate, and that rate is nudged by a few percent every second or two so the cycle is not a clock. The table is the Width slider at 1. Turning Width down pulls every band toward the same narrow damping, so the wash becomes a tone.

The three bands are tuned differently. The ding band is the widest and the loudest. The other two are narrower, so the color notes are easier to hear as pitches.

| Band | Aims at | How often the filter cycles | Width (damping) | Slow center drift | Mix |
| --- | --- | --- | --- | --- | --- |
| Ding | the ding | about every 33 seconds | 0.055 to 0.17 | ±26 cents | 1.00 |
| Color | the recent degree | about every 22 seconds | 0.040 to 0.09 | ±16 cents | 0.72 |
| Second color | the next degree | about every 15 seconds | 0.045 to 0.10 | ±18 cents | 0.50 |

Damping here is the resonator's zeta. The audible width is about `2 × zeta × frequency`. Around a D3 ding (about 147 Hz) the ding band is roughly 15 Hz wide when it is shut and about 50 Hz wide when it is open. Higher notes are wider in hertz by the same proportion. The three oscillators start at different points in their cycle, so the bands do not open together.

Before the first strike the bands are aimed at the ding, because that is the only degree with any weight. They are not heard until a strike opens the fade.

## The bass

One more mass-spring, an octave under a scale degree. If one octave down would sit above 160 Hz, it goes down two octaves instead. It is driven by a quiet sine at its own pitch, with a low damping (zeta 0.006), so it stays a tone. It glides when its note changes, with the same 0.45 second time constant as the bands.

The degree comes from a longer memory than the bands: the last 16 strikes, weighted by strength, with old and new strikes counting the same. The winner has to stay the winner for four beats in a row before the bass moves. A beat for this vote is an accepted strike, or a swell that wraps because no strike came.

One new note does not move the bass. A stretch of that note does, a few beats after it has become the note you are actually playing. Until then the bass stays on the ding, one octave down.

## The taps

A tap is a single short ring on a scale note. It is the same kind of resonator, struck once and left to decay (zeta 0.005, no continuing drive). It does not glide. It is there to fill a hole, and it stays quiet when the playing is already full.

Taps start only after a beat exists. The main one is halfway through the beat. If the beat is longer than 0.7 seconds, a softer one can also land three quarters of the way through.

A tap is allowed when the last strike was long enough ago to be a gap (more than 30% of a beat) and recent enough to still be part of the playing (under 2 seconds). It is held back when two or more strikes have landed in the last 0.85 of a beat, or when four or more strikes have landed in the last 0.6 seconds. A second tap will not start while the previous one is still in its first third of a second.

The note is chosen from the same recent window as the color bands. It prefers the second most played degree, as long as that is not the note just struck. Otherwise it uses the most played degree, with the same condition. If both of those are the note just struck, it steps three pads up the scale.

## How the mix is built

Each sample is:

1. The three bands, summed with the mix weights above, then multiplied by the swell.
2. The bass, multiplied by its shallower swell.
3. The tap, if one is ringing.

That sum is multiplied by Support level and by the presence fade, then limited to the range −1 to 1. The bands are the body of the sound. The bass is quieter. The tap is a short accent and then it is gone.

Until the first strike, presence is zero. The first three gaps turn the swell on. The bass waits for agreement before it leaves the ding. After a rest of about 2 seconds the swell stops, and the Fade control takes the tone away.
