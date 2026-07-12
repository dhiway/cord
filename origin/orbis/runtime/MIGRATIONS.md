# Orbis runtime migration contract

Score and Honour are introduced only from an unambiguous predecessor state: storage version zero
and a completely absent pallet prefix. `ScoreHonourIntroductionPreflight` is the first unreleased
migration and validates both pallets before Score, Honour, or Bulletin writes anything. A dirty v0
prefix or a storage version newer than v1 aborts the runtime upgrade deliberately. Operators must
inspect and explicitly reconcile that state rather than allowing a partial introduction.

Fresh absent prefixes and already-introduced v1 prefixes are accepted. Try-runtime rehearsals cover
fresh introduction, current-state idempotence, and independent dirty Score and Honour failures with
byte-identical state and no later Bulletin progress.
