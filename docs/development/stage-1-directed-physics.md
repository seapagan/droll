# Stage 1 directed-physics evidence

## Status and owner verdicts

Phase 3 remains stopped at the d6 visual checkpoint. The owner reviewed
`review-candidate-b` at `b8fa6173147099a8bc4f531aac62376f4c65be68` and
recorded **FAIL / rejected** on 2026-08-21. Its 18/18 systemic result remains
historical evidence, but the real-window motion showed an effectively settled
die being rotated toward a predetermined face strongly enough to become
`RenewedMotion`/`Bouncing`. The render mesh also culled its exterior faces
because its triangle winding opposed its stored outward normals.

The owner also reviewed `review-candidate-d` at
`5faf74f5202eb0ed32e47c3a37080afe6683c247` and recorded **FAIL / rejected** on
2026-08-21. Candidate D's 18/18 systemic result does not override the repeated
visibly targeted recovery-hop behavior: 15 of 18 ordinary d6 cases required
recovery, with 18 recoveries in total and three cases requiring two. Recovery
was therefore the normal outcome-selection mechanism, not an exceptional
pathology path.

The late-guided-settling/recovery architecture is rejected. The approved
replacement hypothesis is H1 symmetry-conditioned launch families: only the
initial proper body-local solid symmetry may depend on the target, and all
post-spawn motion and settling must be unassisted. H1 remains unproven. d20,
multi-die, and later phases remain prohibited until the replacement d6 gate
passes its automated, platform, and owner-review requirements.

## Candidate provenance and fixed configuration

- Stage 1 baseline: `d3454b0cc3c969e87a7d46f139554d290c68ee8c`.
- Rejected Candidate B: `b8fa6173147099a8bc4f531aac62376f4c65be68`.
- Winding remediation: `fabc603`.
- Candidate D behavior/configuration:
  `5faf74f5202eb0ed32e47c3a37080afe6683c247`.
- Rust 1.97.1; MSRV 1.95.0; Bevy 0.19.1; Avian3D 0.7.0 with `3d`, `f32`, and
  `parry-f32` only. Avian parallel remains disabled.

Candidate D keeps dynamic Avian rigid bodies and uses these measured bounds:

- low-energy decision: tray contact, height at most 0.9, linear speed at most
  0.25, and angular speed at most 0.35;
- guidance capture: target is already the current upward face and target error
  is at most 0.019635 rad (1.125 degrees);
- guidance: acceleration-domain P/D values 4.0/2.5, 0.35 s ramp, and a hard
  0.05 rad/s2 angular-acceleration limit converted to torque using Avian's
  computed world-space inertia tensor;
- disturbance: contact loss, 0.65 linear speed, or 1.0 angular speed;
- completion: 0.10 rad orientation tolerance, 0.10/0.16 rest speeds, and an
  uninterrupted 0.60 s stable window;
- recovery: measured-state entry, one impulse per recovery, 2.6 m/s desired
  upward speed, target-error rotation over the derived ballistic flight time,
  6.0 rad/s angular-speed limit, 0.8 rad/s yaw, 1.2 s contact timeout, three
  attempts, and the existing 18 s product timeout.

The generated d6 now winds every rendered triangle into the same outward
hemisphere as its declared face normal. Mesh-level tests inspect all 12 render
triangles for non-degeneracy and outward winding; culling remains enabled.

## Capture-region derivation

The d6's minimum face-center-to-adjacent-face support boundary is pi/4. The
bounded capture sweep tested pi/80 (0.009817 rad), pi/40 (0.019635 rad), and
pi/20 (0.039270 rad), all far inside that 45-degree face-changing boundary.
The first native release run showed `d6-5-awkward-low-energy` naturally reach
the correct upward face at 0.010590 rad. That was just outside pi/80 and caused
an unnecessary recovery. Pi/40 captured it without recovery while limiting
ordinary guidance to 1.125 degrees, so pi/40 is Candidate D's selected bound.

The selected headless corpus entered guidance at 0.0-0.002003 rad, 0.0757-
0.2350 linear speed, 0.0173-0.2745 angular speed, and at most 0.028483 units of
kinetic energy. Candidate D therefore has measured margin for fixed-step/native
timing variation without approaching a face boundary.

## Mass/inertia-aware guidance evidence

The Avian 0.7 body reports mass 1.0 and principal inertia 0.166667 for the
project-owned unit d6 collider. Candidate D computes desired angular
acceleration, removes yaw from damping, clamps acceleration, converts it with
the rotated `ComputedAngularInertia` tensor, and applies the resulting torque
through `Forces`. It does not multiply raw orientation error into unconstrained
torque.

Across the selected headless corpus:

- maximum applied guidance angular acceleration was 0.013762 rad/s2 against
  the 0.05 limit;
- maximum applied torque was 0.002294 N m against the inertia-derived 0.008334
  N m ceiling;
- maximum guided linear/angular speeds were 0.1651/0.1959, below the
  0.65/1.0 disturbance thresholds; and
- no headless guidance interval produced `RenewedMotion`.

The native release corpus had measured rest-candidate invalidations in
`d6-5-awkward-low-energy` and `d6-6-side-spin`; both remained inside the target
face capture region and returned to rest without recovery. Candidate D never
uses ordinary guidance for a substantially wrong face. If target-up or the
capture bound is lost, force application is suppressed and the lifecycle
selects bouncing or physical recovery.

## Recovery design and evidence

`DirectedDie::require_recovery` was removed. Recovery scenarios now supply only
physical initial state; the same production observation path selects recovery
for normal and recovery corpora. `recovery-bad-orientation` requires one
recovery, while `recovery-edge` settles naturally with zero, which would fail
if scenario identity controlled production recovery.

When motion becomes low-energy outside capture, recovery starts immediately
from that observation. A mass-scaled upward impulse and an inertia-scaled
angular impulse reintroduce real kinetic energy. The angular target is a
bounded velocity over the 2v/g ballistic flight estimate; orientation is never
written. Recovery ends on measured leave-and-return tray contact, with a finite
timeout fallback. Each recovery applies exactly one impulse.

The selected headless d6 corpus used 18 recoveries: three cases used none,
twelve used one, and three used two. Maximum recovery impulses were 2.633
linear and 0.998 angular; recovery-entry energy was at most 0.024098. The
recovery rotation sweep rejected 0.8x (19 total, maximum three) and 1.2x
(19 total, maximum two) in favor of the geometry/flight-time-derived 1.0x
(18 total, maximum two). The selected 2.6 m/s upward speed implies about
0.344 m of ideal ballistic rise, smaller than the 2.8 and 3.0 m/s candidates.

## Contact classification

Tray floors and walls carry a dedicated `TraySurface` marker. A die has tray
contact only when `CollidingEntities` contains a marked entity. An unmarked
collider no longer enables first contact, guidance, rest, or recovery decisions;
the focused regression adds the marker live and proves the distinction. This
keeps future die-on-die contact from masquerading as independent tray contact
without implementing multi-die behavior.

## Baseline and bounded candidate results

Unguided runs retained natural final faces rather than their targets: the three
start families ended on faces 6, 3, and 5. Energetic peaks were 6.0493-7.0550
linear and 9.6531-10.7233 angular for high-tumble/side-spin, versus guidance
entry below 0.2350/0.2745 for Candidate D. The motion states remain separable.

| Configuration | Capture / accel / recovery up | d6 result | Recovery total/max | Max completion | Disposition |
|---|---:|---:|---:|---:|---|
| Candidate B | unrestricted / raw P-D / 1.8 impulse | 18/18 | 0/0 headless | 6.203 s | **Owner rejected**: late face-changing guidance |
| Narrow | pi/80 / 0.05 / 2.6 | 18/18 | 18/2 | 3.828 s | Rejected after native timing caused an unnecessary hop |
| Candidate D | pi/40 / 0.05 / 2.6 | 18/18 | 18/2 | 3.828 s | **Owner rejected**: repeated targeted recovery hops |
| Broad/high | pi/20 / 0.20 / 3.0 | 18/18 | 18/2 | 4.188 s | Rejected: larger correction/energy without systemic benefit |

Candidate D headless results are 18/18 correct, with no wrong-face reveal,
failure, timeout, or recovery-budget exhaustion. Completion was 1.609-3.828 s.
The final Linux release-window corpus also completed 18/18 correctly; its
longest case was `d6-1-high-tumble` at 4.078 s. Release recovery support
completed `recovery-bad-orientation` correctly after one recovery in 1.609 s
and `recovery-edge` naturally in 1.250 s.

Per-case fixed-step rows, including the preserved rejected Candidate B rows,
are in `stage-1-directed-physics-metrics.csv` in deterministic face/start
order.

## Candidate D visual failure cases

- Two recoveries: `d6-1-high-tumble`, `d6-2-awkward-low-energy`, and
  `d6-4-side-spin`.
- Correct-face nudge without recovery: `d6-5-awkward-low-energy`.
- Rest-candidate re-entry: `d6-6-side-spin`.
- Deliberate production-selected recovery: `recovery-bad-orientation`.
- Natural no-recovery control: `recovery-edge`.

The repeated recovery hops were visibly targeted and failed the owner review.
Their systemic correctness cannot waive that result. These case IDs remain as
reproducers and negative evidence for the rejected architecture.

## Review commands

```console
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --case d6-1-high-tumble
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --case d6-2-awkward-low-energy
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --case d6-4-side-spin
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --case d6-5-awkward-low-energy
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --case d6-6-side-spin
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery --case recovery-bad-orientation
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery --case recovery-edge
```

## Platform and checkpoint verdict

The native Linux release runs used Vulkan on NVIDIA GeForce RTX 4070 Ti SUPER,
driver 595.84. macOS arm64, macOS Intel, Windows native/manual evidence, d20,
multi-die, keep/drop, and final Stage 1 GO/STOP remain later gates. Linux xwin
0.23.1 remains supplementary compile/check/Clippy evidence only.

Checkpoint verdict: **FAIL / OWNER-REJECTED FOR CANDIDATE D**. The
late-guided-settling/recovery architecture is rejected; H1 is the approved
replacement hypothesis. Candidate B/D measurements remain historical negative
evidence and do not constitute H1 evidence or approval.
