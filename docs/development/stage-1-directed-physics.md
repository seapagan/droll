# Stage 1 directed-physics evidence

## Status and non-claims

This is the Phase 3 d6 owner-review checkpoint. The directed d6 implementation
uses a predetermined target, a dynamic Avian rigid body, project-owned geometry
and collider vertices, late corrective torque through `Forces`, measured
stability gates, and a bounded physical recovery impulse. It never selects a
semantic result, writes a final transform, switches the die to kinematic, or
treats sleeping as proof of correctness.

The owner visual verdict is **pending**. Automated tests, numeric metrics, the
Linux window launch, screenshots or recordings, and the implementation agent's
judgement do not constitute that verdict. d20, multi-die, keep/drop, and final
native-platform evidence are not implemented and Stage 1 has no GO verdict.

## Candidate and configuration

- Starting baseline: `d3454b0cc3c969e87a7d46f139554d290c68ee8c`.
- Geometry checkpoint: `3a02f8a`.
- Owner-review candidate: the checkpoint commit containing this evidence; its
  exact SHA is reported with the owner-review handoff because a commit cannot
  contain its own SHA.
- Rust 1.97.1; MSRV 1.95.0; Bevy 0.19.1; Avian3D 0.7.0 with `3d`, `f32`, and
  `parry-f32` only; Avian parallel remains disabled.
- Review controller: `review-candidate-b`, proportional gain 7.0, damping 2.4,
  0.55 s ramp, near-tray height 0.9, guidance entry speeds 0.9 linear and 2.4
  angular, 0.10 rad target tolerance, 0.10/0.16 rest speeds, 0.60 s stable
  window, 3.5 s stall, 0.55 s recovery interval, 1.8 linear and 0.7 angular
  recovery impulses, three recoveries, and 18 s product timeout.

These values are a d6 review candidate, not final cross-shape or cross-platform
tuning. A behavior-affecting change invalidates this evidence.

## Corpus and measurement method

The deterministic corpus contains every target face 1 through 6 with
`high-tumble`, `side-spin`, and `awkward-low-energy` starts. Stable identifiers
are `d6-<face>-<start>`. Recovery support uses
`recovery-bad-orientation` and `recovery-edge`.

The headless harness uses `MinimalPlugins + PhysicsPlugins` with production
geometry, colliders, lifecycle, force systems, and a 60 Hz manually advanced
clock. The independent test watchdog is 1,400 updates; it is not a tuning
value. Each sample records contact/guidance/terminal transitions, speeds,
target error, recovery count, and maximum guidance torque.

Unguided baselines naturally stopped with zero terminal motion but retained the
face produced by the throw. High-tumble and side-spin starts peaked at
6.0493–7.0550 linear and 9.6531–10.7233 angular speed. Awkward starts peaked at
1.7848 linear and 2.3853 angular speed. Across guided candidates, entry occurred
only after contact at 0.3419–0.6894 linear and 0.4452–0.8042 angular speed,
separating the energetic throw from the low-energy guidance region.

## Bounded candidate sweep

| Candidate | P / D / ramp | d6 result | Completion range | Recovery | Disposition |
|---|---:|---:|---:|---:|---|
| A | 4.0 / 1.4 / 0.35 s | 18/18 correct | 1.500–15.641 s | 3 total | Rejected: slow tail and avoidable recovery |
| B | 7.0 / 2.4 / 0.55 s | 18/18 correct | 1.500–6.203 s | 0 | Owner-review candidate |
| C | 10.0 / 3.2 / 0.80 s | 18/18 correct | 1.484–5.469 s | 0 | Rejected for review: higher gain without a decisive systemic benefit |

Candidate B leaves 2.9 times its measured worst d6 completion time inside the
18 s timeout. Its guidance-entry threshold is well below the energetic maxima.
The remaining decision—whether its motion is subtle and convincing—is visual
and belongs to the owner.

## Systemic results

- Headless review candidate B: 18/18 d6 target/start cases revealed the correct
  face; zero wrong-face reveals, failures, timeouts, or recoveries.
- Completion median is 3.430 s; 17/18 cases completed by 5.234 s and the
  maximum is 6.203 s.
- The release-window corpus also revealed the correct face in all 18 cases with
  no failure or timeout. Unlike the fixed-step headless run,
  `d6-4-side-spin` used one recovery and completed in 10.234 s; this is a
  specific owner-review case, not evidence-equivalent behavior.
- `recovery-bad-orientation` completed correctly in 7.438 s after one bounded
  recovery; `recovery-edge` settled naturally in 1.234 s without recovery.
- Tests prove energetic motion cannot enter guidance, wrong faces cannot
  reveal, rest candidates return to bouncing after renewed physical motion,
  the recovery budget terminates, and one guidance step does not snap to the
  target.
- The release window reported Vulkan on NVIDIA GeForce RTX 4070 Ti SUPER,
  driver 595.84, and reported exact scenario/case/target and terminal summaries
  for the complete d6 and recovery corpora.

Per-case rows are in `stage-1-directed-physics-metrics.csv` in deterministic
face/start order. Commands used for the sweep and recovery evidence:

```console
cargo test -p droll-gui --test directed_physics --locked
cargo test -p droll-gui --test directed_physics \
  test_d6_measurement_matrix_is_finite_and_never_reveals_wrong_face \
  --locked -- --nocapture
```

## Platform and gate matrix

| Evidence | Linux x86_64 | macOS arm64 | macOS Intel | Windows MSVC |
|---|---|---|---|---|
| Native/systemic | Local pass | Pending later native gate | Pending later native gate | Pending later native gate |
| Cross-target | N/A | N/A | N/A | cargo-xwin 0.23.1 check/Clippy/build pass from Linux |
| Real-window d6 | Prepared; owner verdict pending | Pending | Pending | Pending |

Linux host prerequisites were Rust 1.97.1, Clang/clang-cl 19.1.1, llvm-tools,
and the `x86_64-pc-windows-msvc` target. Cross-building is supplementary and is
not native Windows or visual evidence.

## Known limitations and checkpoint verdict

- Visual quality, label readability in motion, late-guidance subtlety, and
  recovery naturalness require the owner's live review.
- The recovery edge case naturally reached its target without needing the
  recovery impulse; the bad-orientation case is the deliberate recovery proof.
- `d6-4-side-spin` needs particular visual review because the real-window run
  used one recovery where the headless run used none.
- d20, collisions, keep/drop, resource characterization, and the remaining
  native hosts are intentionally pending later phases.

Checkpoint verdict: **PENDING OWNER D6 PASS/FAIL**. Do not begin d20 until the
owner records an explicit pass. A fail is a STOP for review, not permission to
retune or redesign silently.
