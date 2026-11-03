# PC reference facts and limits

[O] UNAUDITED static comparison, 2026-11-03. The PS2 remains the gameplay reference.
These are original descriptions of facts, not reconstructed source excerpts. No
new original-game run or gameplay acceptance is claimed.

The projects jt-marte_OpenSWBF2, OpenSWBF2_OpenSWBF2 and OpenSWBF2_swbf2decomp
provide names for PC Battlefront II executable addresses. Their unlicensed
reconstructions may help identify a PS2 function, but their code and close
paraphrases must not enter this MIT project. A modernized function can contain
new behaviour under an old executable address. Agreement among related forks
is not independent confirmation of retail behaviour.

The useful chain is a named PC behaviour, an evidenced PS2 counterpart, a check
against the original game, then an assessment of our Rust implementation.
Function pairs are behaviour correspondences; they do not imply identical
layouts, constants or instructions across platforms. BF2-native sprint and
Force systems must not acquire invented BF1 equivalents.

Two examples from this pass clarify existing analysis:

- CameraTrackSetting::SetupCameraTrackMatrix, PC 0x00659090 in
  jt-marte_OpenSWBF2 and OpenSWBF2_swbf2decomp, corresponds to BF2 PS2
  FUN_001cef68. Its obstruction cache has five entries and updates one entry
  per camera update (BF2 listing lines 78578-78715). BF1's counterpart
  FUN_001b58f0 casts all five during the same update (BF1 listing lines
  59488-59626). The five-ray description in re-aim-assist.md section 10.6
  therefore needs that temporal qualification. Our swept-sphere policy is an
  explicit deviation recorded in camera-occlusion.md, not a newly found bug.
- OrdnanceTowCable::Update, PC 0x006081c0 in those same projects, helps name
  BF2 FUN_00323178 and BF1 FUN_002814f8. Both PS2 updaters replace the packet's
  base damage with the target's maximum health before dispatch (BF2 listing
  line 315990; BF1 line 206359). BF2 then has additional source-seat and
  friendly-fire handling. The existing towing note's description of BF1's
  ordinary descriptor needs this qualification. A complete towing orbit and
  damage comparison in the original game is still outstanding.

Existing camera and towing measurements remain in re-aim-assist.md,
re-tow-cable.md and re-vehicle-motion.md. Static source agreement cannot replace
their pending original-game checks.
