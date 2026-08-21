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
replacement hypothesis was H1 symmetry-conditioned launch families: only the
initial proper body-local solid symmetry may depend on the target, and all
post-spawn motion and settling must be unassisted. The bounded H1 implementation
at `623986844597b96f306c87c5dfa83f1c7689c67e` achieved 216/216 correct
unassisted faces but **failed the then-ratified complete-trajectory paired
physical-equivalence gate**. That mandatory STOP remains part of the experiment
history. The preserved pair was equivalent before contact and diverged only
after contact.

The owner subsequently corrected the H1 contract: paired pre-contact physical
equivalence remains blocking, while numerical divergence at and after contact
is diagnostic unless it causes an actual H1 defect. The corrected test boundary
at `d185919` passes all 216 paired cases without changing the prior tolerances,
and the independent all-face corpus remains 216/216 with zero recovery or
intervention. H1 has not received an owner visual verdict. d20, multi-die, and
later phases remain prohibited.

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

## H1 symmetry-conditioned d6 experiment

### Pure symmetry premise

The project-owned d6 implementation enumerates all 24 proper cube rotations.
Focused tests prove determinant +1, cube-vertex preservation, face-normal
bijection, opposite-face preservation, every ordered target/base mapping,
collider mass/center/inertia equivalence, and exclusion of render-only pips from
collider mass properties. A non-commuting quaternion probe verifies the live
Bevy/glam multiplication convention. Every nominal and orientation-nuisance
case constructs `Q_variant = P * Q_base` before right-composing the local
symmetry as `Q_target = Q_variant * S`.

### Bounded launch-family search

The search grid contained exactly 64 target-independent combinations: 16 in
each of four energetic regions. It stopped after considering five combinations
because one passing family had been found in every region. No search state,
velocity, nuisance, or family selection used a target face.

| Result | Search ID | Runtime family | Natural face | Reason |
|---|---|---|---:|---|
| Rejected | `search-00-high-tumble` | — | 5 nominal | +1% angular nuisance changed face 5 to 4 |
| Accepted | `search-01-high-tumble` | `h1-family-a` | 6 | 9/9 nominal/nuisance states retained face 6 |
| Accepted | `search-16-side-spin` | `h1-family-b` | 3 | 9/9 retained face 3 |
| Accepted | `search-32-overhead-tumble` | `h1-family-c` | 1 | 9/9 retained face 1 |
| Accepted | `search-48-diagonal-spin` | `h1-family-d` | 3 | 9/9 retained face 3 |

The preregistered nuisance IDs were used unchanged: nominal, world X position
plus/minus 0.01 m, world `(1,1,0)` orientation plus/minus 0.5 degrees, linear
velocity times 1.01/0.99, and angular velocity times 1.01/0.99. Accepted-family
first tray contact was 0.633-0.817 s; natural completion was 1.817-2.917 s;
peak linear speed was 5.7583-7.5031; and peak angular speed was 8.8834-12.0201.
All accepted states had meaningful airborne/tumbling motion and tray contact.

### H1 lifecycle and all-face corpus

Mode `symmetry-launch` uses the isolated lifecycle
`Spawned -> FreeThrow -> Bouncing -> RestCandidate -> Revealed/Failed`.
Its plugin has no force/torque writer and no guidance, recovery, retry,
post-spawn transform write, or kinematic path. Target-independent position,
linear velocity, angular velocity, and nuisance state are applied unchanged;
only the selected initial proper body-local symmetry varies by target. Unknown
mode values exit explicitly rather than falling back to Candidate D.

All four families completed all six targets across nominal plus eight nuisance
states: **216/216 correct faces**, zero timeout, zero wrong face, zero recovery,
and exactly zero recorded post-spawn target-aware force, torque, work, or
intervention. Maximum completion was 1.969 s for family A, 2.234 s for family B,
3.016 s for family C, and 2.406 s for family D. Group summaries remain in the
CSV; raw per-step traces remain untracked build evidence.

### Initial complete-trajectory paired-equivalence STOP

The paired test runs a target-independent geometric control and canonicalizes
the target orientation as:

```text
Q_canonical(t) = normalize(Q_target(t) * inverse(S))
```

The same-build tolerances were 0.01 m world position, 0.02 m/s linear velocity,
0.02 rad/s angular velocity, 0.01 rad sign-invariant canonical orientation,
and one fixed update for completion classification. These are deliberately much
larger than pure `f32` geometry error while remaining small relative to the
one-metre die and 5.76-7.50 / 8.88-12.02 energetic speed ranges.

The first paired nominal comparison, `h1-family-a`, target 1 versus its
target-independent control, diverged after contact. Linear velocity first
exceeded tolerance at trace step 47. Across the paired trace, maximum deltas
were:

- world position: 0.01532233 m;
- linear velocity: 0.22150882 m/s;
- angular velocity: 0.61894333 rad/s;
- canonical orientation: 0.02007314 rad; and
- completion: 107 versus 106 trace samples, one fixed update apart.

Contact counts remained paired through the compared trace and both runs reached
the correct face, but the physical-field deltas materially exceeded every
then-blocking complete-trace numeric tolerance. The original test stopped
before checking later target pairs. Under the contract ratified at that time,
this was a mandatory **STOP / PAIRED-TRAJECTORY DIVERGENCE** before Linux visual
review. The implementation and negative evidence were preserved. The later
contract correction does not erase, rename, or reinterpret that original STOP.

### Corrected pre-contact gate rerun

Commit `d185919` splits each paired trace at the first physical tray contact in
either run. Position, linear velocity, angular velocity, and sign-invariant
canonical orientation remain blocking before that boundary with the unchanged
0.01 m, 0.02 m/s, 0.02 rad/s, and 0.01 rad tolerances. The existing pure
composition-order regression remains blocking, and an additional regression
proves that target-dependent initial linear velocity fails the pre-contact
gate.

All 216/216 paired cases passed the corrected pre-contact gate. Across them,
first contact occurred at trace steps 37-48. Maximum pre-contact deltas were:

- world position: 0.000000363 m;
- linear velocity: 0.0 m/s;
- angular velocity: 0.000275784 rad/s; and
- canonical orientation: 0.00119604 rad.

Post-contact measurements remain present for every pair. Of 216 comparisons,
180 crossed at least one former complete-trace numeric/lifecycle/contact
threshold and 36 did not. First recorded diagnostic divergence was at trace
steps 51-122, always after the paired first-contact boundary. Bounded maxima
across the post-contact paired traces were 0.126389 m position, 1.693292 m/s
linear velocity, 4.861898 rad/s angular velocity, 0.325243 rad canonical
orientation, and 0.328125 s completion-time difference. Tray contact-count
transitions, completion differences, and both final physical faces remain in
the per-pair test diagnostics.

The previously preserved `h1-family-a`, nominal target-1 comparison remains
equivalent through first contact at step 46 and first crosses a former strict
threshold at step 51. Its maximum post-contact deltas remain 0.01532233 m,
0.22150882 m/s, 0.61894333 rad/s, and 0.02007314 rad, with a 0.015625 s
completion difference. The control finishes on its natural face 6 and the
mapped run finishes on requested face 1.

The separately rerun nominal+nuisance outcome corpus remains **216/216 correct
faces**, with zero timeout, wrong face, recovery, retry, or post-spawn
target-aware force, torque, work, transform correction, or other intervention.
Completion was 1.719-3.016 s; family maxima remained 1.969 s (A), 2.234 s (B),
3.016 s (C), and 2.406 s (D). Peak corpus linear/angular speeds were
7.5031/11.4011. The family search was not rerun or expanded to replace any
family.

Corrected automated H1 verdict: **PASS / OWNER LINUX VISUAL VERDICT PENDING**.
This automated result and any agent observation are not the required owner
visual verdict. No d20 or cross-platform H1 validation is authorized before
that later explicit owner decision.

## Historical Candidate D review commands

```console
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d --case d6-1-high-tumble
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d --case d6-2-awkward-low-energy
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d --case d6-4-side-spin
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d --case d6-5-awkward-low-energy
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario d6-faces --mode candidate-d --case d6-6-side-spin
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery --mode candidate-d
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery --mode candidate-d --case recovery-bad-orientation
cargo run -p droll-gui --release --example directed_physics_spike -- --scenario recovery --mode candidate-d --case recovery-edge
```

## Platform and checkpoint verdict

The native Linux release runs used Vulkan on NVIDIA GeForce RTX 4070 Ti SUPER,
driver 595.84. macOS arm64, macOS Intel, Windows native/manual evidence, d20,
multi-die, keep/drop, and final Stage 1 GO/STOP remain later gates. Linux xwin
0.23.1 remains supplementary compile/check/Clippy evidence only.

Candidate D checkpoint verdict: **FAIL / OWNER-REJECTED**. H1 replacement
initial checkpoint verdict: **STOP / PAIRED-TRAJECTORY DIVERGENCE BEFORE VISUAL
REVIEW**. The corrected pre-contact gate and 216/216 corpus pass, but the H1
owner Linux visual verdict remains pending. Candidate B/D and the original H1
STOP remain historical evidence; neither automation nor agent observation
constitutes visual approval or authority to begin d20.
