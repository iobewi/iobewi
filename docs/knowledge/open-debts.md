# Open architectural and implementation debt

## DEBT-001 — NOR erase abstraction duplication

Portable `nor::erase_range` / `NorSlot`, ESP partition helpers and Agent
`EspArtifactStorage` contain related erase/range logic with different region/error
models. S20 deliberately did not factor this because doing so requires a new portable
abstraction and requalification of the long-validated Agent OTA path.

## DEBT-002 — recursion_limit remains 512

The Agent build historically requires a raised recursion limit. S20 measured that 128
fails while 192 passes, but kept 512 to avoid unrelated churn. This should be reduced or
the underlying type expansion diagnosed in a dedicated change.

## DEBT-003 — complete serial boot-guard sequence not captured

S20 proved the functional effect of the RTC boot guard over HTTP and proved healthy
counter clearing on serial. The complete serial `2/3 -> 3/3 -> SUPPRESSED` sequence was
not captured. This is an evidence gap, not a known functional failure.
