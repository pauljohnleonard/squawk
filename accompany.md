# Accompaniment

Notes on the desk under the lead instrument, as it works today, plus a few product directions that should stay true as the code moves.

DSP lives mainly in `crates/squawk-engine/src/support.rs`, with tabla / FM bass / phrase clock / reverb as sibling engine modules. The Handpan **desk** (strips, mute/solo, trim, pan, send, meters) is wired in `crates/squawk-app` (`ui.rs` + the Handpan branch of the audio callback in `main.rs`).

**Today:** Handpan is the lead, and the accompaniment desk below runs while Handpan is selected. That is the current center of gravity, not a hard rule that only a handpan can sit in the Main strip.

## Direction (not blocking current polish)

These do not demand a big rewrite now; they are constraints to keep in mind:

1. **Any playable instrument can be the lead.** Handpan is first; other strike / pad voices should be able to take Main without redesigning the desk idea.
2. **Insert effects** (per-strip or bus inserts beyond today’s send reverb) may come later — design room, not a near-term build.
3. **Support should stay easy to experiment in.** Washes, followers, drones, groove roles, and future beds should stay swappable voices on strips, not a frozen “one pad engine” shape.
4. **External controls.** MIDI and similar hardware should drive the same parameters the UI does; the computer keyboard is a convenience, not the long-term control surface.
5. **Embedded / Pi.** Much of the egui desk may go away on a screenless box, but **presets designed on the desktop** (settings / parameter snapshots) should still load and run. Keep the control schema and persistence host-friendly, not UI-only.

## Desk overview

With Handpan selected, the right-hand mixer is a desk of strips, not a single “Support level” column:

| Section | Strips |
| --- | --- |
| Main | Handpan |
| Support | Resonator, Waveshape, Tambura, Root drone, Followers, Bass (FM line), Tabla, Echo |
| Effects | Reverb |

Each strip (where it applies) has **Pan**, **Level**, a peak **meter**, **Mute** / **Solo**, and a reverb **Send**. Clicking a strip name opens its inspector (trim, voice knobs, groove amount, and so on).

### Trim vs Level

- **Trim** (inspector, before mixer): instrument gain into the strip. Default `1`. Range `0…4`. Use this to boost a quiet voice without parking the strip Level above unity.
- **Level** (strip fader): mix contribution. Default `1`. Range `0…2`.

For the three washes, the callback feeds Support as `wash_level = trim × Level`. That product is a **blend weight on the shared formant bus**, not a post-gain on three identical coloured copies. Turning one wash Level down changes how much of that voice is in the shared colour; it does not run three independent FX chains.

Handpan, Followers, Root drone, FM Bass, Tabla, and Echo also use trim × Level (or trim into part level) on their paths. The Support engine bus itself is held at **unity** (`set_level(1.0)`), so strip Levels share the same reference as the Handpan fader. The old `DEFAULT_SUPPORT_LEVEL` of `0.35` was a pre-desk “quiet bed” attenuator and is no longer applied as a master pad gain in the callback.

### Mute / Solo

Wash mute/solo bits (Resonator / Waveshape / Tambura) are separate from Support part bits (Root drone / Echo) and from Handpan / Bass / Tabla / Reverb solo flags.

- Soloing a wash isolates that wash via Support’s wash gain and ducks Root drone, Followers, and Echo.
- Wash solo is **not** treated as `foreign_solo`. Marking washes as foreign would zero the shared Root envelope and silence the soloed wash as well.
- Soloing Handpan, Bass, Tabla, Reverb, or a non-wash Support part sets `foreign_solo` so washes / followers duck appropriately.

### Reverb sends

Each strip that can send has its own Send control. Wash sends (Ding / Waveshape / Tambura) and the Followers send are independent; they are not linked to a single “Support send.”

## What Support hears

Every handpan strike arrives as a note and a strength. The note is snapped to the nearest of the ten pads in the current scale. The strength is the same amplitude the handpan was struck with.

Scale, ding, and tuning match the handpan. Changing them retunes washes, followers, and root. Bands and the bass glide to the new pitches.

The last 16 strikes are kept (time, degree, strength). Harder strikes count for more. A soft strike still counts a little.

A strike closer than 80 ms to the previous one is a flam: it updates which notes have been played (bands follow), but does not move the beat or reset the swell.

A gap of 2.5 seconds or more is a pause. The next strike starts the swell again from the top.

Strike place (center / between / shoulder) is not used by Support — only note and strength.

## Two clocks

Support keeps its own **pulse / breath** clock from strike gaps (median of recent accepted gaps, default assumption ~0.55 s before enough gaps are heard). That drives follower swell and related motion.

Groove lock + **PhraseClock** are separate: after tempo locks from playing, tabla theka and the FM bass line walk a shared phrase. Support pulse does not have to lock to the theka; musically they can sit on different feels today.

## Washes (Resonator / Waveshape / Tambura)

Three voices share one Root mass-spring + formant colour bus. Levels are blend weights into that bus (see Trim vs Level above).

- **Resonator** — noise-driven resonant band around the ding / root path. Under Handpan it clears on each strike and blooms after a short gap (~0.28 s, or ~35% of the Support beat) so it fills space instead of attacking with the lead. Register: Unison / Octave down / Fifth / Octave + fifth.
- **Waveshape** — waveshaped ding voice into the same coloured bus. Same strike space-gate and register options as Resonator.
- **Tambura** — plucked-string bank (jawari / pick / strum / cycle); string count and solo-string audition live in its inspector.

Shared formant controls (rate, depth, three formant Hz / Q) colour the combined wash body. Band motion knobs (Width, Wander, Drift, Motion) still shape the underlying resonant motion.

Inspectors also expose **Trim** for each wash. Strip Level / Mute / Solo / Send live on the desk row.

## Followers

Soft bands that track recent scale degrees under the washes.

- Strip **Level** and **Trim** scale the Followers bus (`(recent + second) × trim × Level`).
- **Follower 1** / **Follower 2** (`MixRecent` / `MixSecond`) must be above 0 or you hear nothing from that voice.
- **Breath** (`SupportTone`): `0` = clearer tonal drive; `1` = noise-driven (breathy).
- **Pulse** (`SupportPulse`): swell depth against Support’s own beat (default `0.55`). Until three gaps have been heard, pulse is ignored and the layer stays steady. The bass / root path takes a shallower share of that swell.
- **Fade** (`SupportFade`): how long tone takes to leave after the ~2 s presence hold (default `4` s, range `0.5…12`).
- Band motion (Width / Wander / Drift / Motion) lives here as well for the follower colour bands.

Aims:

- Follower colour bands track recent degrees (last eight strikes, newer and harder weighted more).
- Centers glide (~0.45 s time constant) with wander / drift / width LFOs as in the engine tables.

## Root drone

Low pedal on the key root (octave under the ding, or two octaves when one would sit above ~160 Hz). Narrow tonal drive.

- Fades in when you play, then **holds** until Mute or Level / presence clears it (it can keep sounding after pad presence has faded).
- Strip Level mixes it; **Trim** feeds Support’s root part level.
- Wash solo ducks it.

## Echo

Quiet decorative taps / delayed pad hits, driven from play memory + decorations, voiced as Bells or Handpan (Echo instrument choice).

- Strip Level is the taps / echo bus (`MixTaps`).
- **Trim** scales the echo part inside Support’s `echo_gain` path.
- Held back when washes are soloed or when Echo is muted; respects foreign solo.

## Groove lines (Bass + Tabla)

After Groove locks tempo from playing:

- **Tabla** strip — theka amount, groove pattern, trim × Level into the tabla voice; walks bols on PhraseClock steps.
- **Bass** strip — FM bass line amount / variation; shares the same phrase clock (root tends to land on sam after lock). Amount `0` turns the line off. Has its own reverb Send (separate from Root drone).

These are separate from Support’s Root drone. Groove click / sensitivity / phase pull live in the Groove window, not on the strip row.

## Reverb

Stereo wet bus. Each sending strip has its own Send; the Reverb strip has Level / Mute / Pan (and can be soloed as a desk channel). Decay and tone live in the Reverb inspector.

## Presence and starting / stopping

Washes and followers stay silent until the first strike (resonators already running — entrance is a ~0.5 s fade). A flam counts as playing.

For ~2 s after the last strike, pad presence holds. Then pulse depth falls (~1 s) and tone fades over **Fade**. Root drone uses its own presence path and can hold longer.

The next strike brings pad presence back; remembered Support beat remains.

## How a Handpan frame is mixed (today)

Rough order in the audio callback (Handpan instrument branch):

1. Advance groove, decorations (echo strikes), phrase clock → tabla + FM bass hits.
2. Read handpan sample × trim × (muted/solo-aware) Level; pan.
3. Read Support parts: resonator, waveshape, tambura, followers, root drone (wash blend and mute/solo already applied inside Support).
4. Scale root drone × strip Level; followers × trim × Level; FM bass and tabla × trim × Level; echo taps × Level.
5. Build reverb send bus from handpan + wash sends + followers + root + echo + tabla; mix wet × Reverb Level / mute / pan.
6. Sum dry stereo pans + wet; publish per-strip peak meters.

Structural note (rain-check): this mix is still inlined in the app callback. Before the next new desk channel (or if mixer wiring bugs keep stacking), extract a `DeskMix` / frame helper and table-drive strip descriptors so `DeskModule` / `MixChannel` / ParameterIds stay aligned. Until then, prefer polish on the current desk rather than a large rewrite. Flexible lead/support voices, future inserts, external control, and desktop→embedded presets all argue for that same tidy when growth hits — not for stopping polish now.
